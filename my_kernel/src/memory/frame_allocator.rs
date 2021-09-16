use core::slice;
use core::convert::TryInto;
use core::mem;
use crate::bootloader_structs::{ BootInfo, E820MemoryRegion };
use crate::println;
use crate::memory::page_table::*;
use crate::elf::ProgHeaderEntry;
use alloc::vec::Vec;

const INITIAL_STACK_SIZE: usize = 0x2000;

#[derive(Debug)]
pub struct LinkedListFrameAllocator {
    pub frame_count: u64,
    pub next: usize,
}

impl LinkedListFrameAllocator {
    pub unsafe fn init(boot_info: &BootInfo) -> Self {
        let (frame_count, first_page) = init_frames(boot_info);
        LinkedListFrameAllocator {
            frame_count,
            next: first_page,
        }
    }

    // this only works when identity mapped because the next points to some 
    // physical addr
    pub fn allocate(&mut self) -> Option<&'static PhysPage4KiB> {
        if self.frame_count != 0 {
            self.frame_count -= 1;
            unsafe {
                let ptr = self.next as *mut usize;
                self.next = *ptr;
                return Some(&*(ptr as *const PhysPage4KiB));
            }
        }
        if self.next != 0xdeadbeef {
            panic!("Next was not set to the end singifier");
        }
        None
    }

    pub fn deallocate(&mut self, page: &PhysPage4KiB) {
        if self.frame_count == 0 {
            self.frame_count = 1;
            unsafe {
                let ptr = page as *const PhysPage4KiB as usize as *mut usize;
                *ptr = 0xdeadbeef;
                self.next = ptr as usize;
            }
        } else {
            self.frame_count += 1;
            unsafe {
                let ptr = page as *const PhysPage4KiB as usize as *mut usize;
                let ptr_next = self.next as *const PhysPage4KiB as usize;
                *ptr = ptr_next;
                self.next = ptr as usize;
            }
        }
    }

    // This needs to be used when memory is not identity mapped and fully mapped
    // This will need to map in the frame in order to get the 'next' pointer
    pub fn allocate_and_map(&mut self, pml4: &mut PML4, vaddr: &VirtPage4KiB, 
        heap_regions: &Vec<(&'static PhysPage4KiB, usize)>) -> Option<&'static VirtPage4KiB> {
        if self.frame_count != 0 {
            self.frame_count -= 1;
            unsafe {
                // phys addr
                let phys_page = self.next;
                let virt_page = vaddr as *const VirtPage4KiB as usize;
                // must map phys addr to some virt addr
                // then get the new next pointer from virt addr
                pml4.map_frame_4k(phys_page, virt_page, true, false, Some(heap_regions));

                let next = *(virt_page as *mut usize);

                self.next = next;
                return Some(&*(virt_page as *const VirtPage4KiB));
            }
        }
        if self.next != 0xdeadbeef {
            panic!("Next was not set to the end singifier");
        }
        None
    }

    pub fn deallocate_and_unmap(&mut self, pml4: &mut PML4, page: &VirtPage4KiB,
        heap_regions: &Vec<(&PhysPage4KiB, usize)>) {
        if self.frame_count == 0 {
            self.frame_count = 1;
            unsafe {
                let ptr = page as *const VirtPage4KiB as usize as *mut usize;
                *ptr = 0xdeadbeef;
                let phys_page = pml4.unmap_frame_4k(page, Some(heap_regions));
                self.next = phys_page as *const PhysPage4KiB as usize;
            }
        } else {
            self.frame_count += 1;
            unsafe {
                let ptr = page as *const VirtPage4KiB as usize as *mut usize;
                let ptr_next = self.next as *const VirtPage4KiB as usize;
                *ptr = ptr_next;
                let phys_page = pml4.unmap_frame_4k(page, Some(heap_regions));
                self.next = phys_page as *const PhysPage4KiB as usize;
            }
        }
    }
}

fn init_frames(boot_info: &BootInfo) -> (u64, usize) {
    println!("Initializing physical memory for frame allocator");
    let mm = {
        let ptr = boot_info.mem_map as *const E820MemoryRegion;
        unsafe { slice::from_raw_parts(ptr, boot_info.mem_map_entries as usize) }
    };

    let mut total_mem: u64 = 0;
    for region in mm {
        if region.region_type == 1 {
            total_mem += region.len;
            println!("Mem Region {:#x}:{:#x}", region.start_addr, region.len);
        }
    }
    let gib = total_mem / 0x40000000;
    println!("total mem: {} GiB", gib);

    get_elf_regions(boot_info, &mm)
}

fn get_elf_regions(boot_info: &BootInfo, mem_map: &[E820MemoryRegion]) -> (u64, usize) {
    let e = {
        let ptr = boot_info.elf_location as *const u8;
        unsafe { slice::from_raw_parts(ptr, boot_info.elf_size as usize) }
    };
    
    if e.get(0..4) != Some(b"\x7fELF") {
        panic!("Invalid ELF header");
    }

    let ph_off: usize =
        u64::from_le_bytes(
            e.get(0x20..0x28).expect("Couldn't get offset [0]")
            .try_into().expect("Couldn't get offset [1]"))
            .try_into().expect("Couldn't get offset [2]");

    let ph_ent_size: u16 =
        u16::from_le_bytes(
            e.get(0x36..0x38).expect("Couldn't get ent size [0]")
            .try_into().expect("Couldn't get ent size [1]"))
            .try_into().expect("Couldn't get ent size [2]");

    assert_eq!(ph_ent_size as usize, mem::size_of::<ProgHeaderEntry>());

    let ph_ent_num: u16 =
        u16::from_le_bytes(
            e.get(0x38..0x3a).expect("Couldn't get ent num [0]")
            .try_into().expect("Couldn't get ent num [1]"))
            .try_into().expect("Couldn't get ent num [2]");
    
    let prog_headers = {
        let ptr = (boot_info.elf_location + ph_off) as *const ProgHeaderEntry;
        unsafe { slice::from_raw_parts(ptr, ph_ent_num as usize) }
    };
    let mut seg_avoid: [(usize, usize); 0x10] = [(0x0, 0x0); 0x10];

    let mut ind = 0;
    let mut seg_lowest: usize = 0xffffffffffffffff;
    let mut seg_greatest: usize = 0;
    for entry in prog_headers {
        if entry.seg_type == 0x1 {
            if ind == seg_avoid.len() {
                panic!("Too many segments");
            }
            let start_page = entry.v_addr & 0xfffffffffffff000; // align to 0x1000
            let end_page = 
                (entry.v_addr + entry.mem_size as usize) & 0xfffffffffffff000; // align to 0x1000
            seg_avoid[ind] = (start_page, end_page);
            ind += 1;
            if start_page < seg_lowest {
                seg_lowest = start_page;
            }
            if end_page > seg_greatest {
                seg_greatest = end_page;
            }
        }
    }

    // get valid page ranges
    let mut usable_pages = 0;
    let mut unusable_pages = 0;
    let mut first_page: usize = 0;
    let mut last_good_page: u64 = 0;
    let mut first_page_set = false;

    let (pt_min, pt_max) = get_page_table_min_max();

    let stack_end_page = (boot_info.stack_location - 1) & 0xfffffffffffff000; // align to 0x1000
    let stack_start_page = stack_end_page - INITIAL_STACK_SIZE;

    for region in mem_map.iter() {
        if region.start_addr == 0x0 || region.region_type != 1 {
            continue;
        }
        let page_count = region.len / 0x1000;
        for i in 0..page_count {
            let page_addr = region.start_addr + (i * 0x1000);
            // check that nothing important is in this page
            // check stack, loaded elf segments, etc
            // since these are all page aligned this makes it easier

            // segments
            if page_addr >= seg_lowest as u64 && page_addr <= seg_greatest as u64 {
                let mut skip_page = false;
                for (start_page, end_page) in seg_avoid {
                    if start_page == 0 {
                        break
                    }
                    if page_addr >= start_page as u64 && page_addr <= end_page as u64 {
                        skip_page = true;
                        break;
                    }
                }
                if skip_page {
                    unusable_pages += 1;
                    continue;
                }
            } else if page_addr >= stack_end_page as u64 && page_addr <= stack_start_page as u64 {
                unusable_pages += 1;
                continue;
            } else if page_addr >= pt_min && page_addr <= pt_max {
                if check_page_table_overlap(page_addr) {
                    unusable_pages += 1;
                    continue;
                }
            }

            usable_pages += 1;

            if !first_page_set {
                first_page_set = true;
                first_page = page_addr as usize;
            }

            // point every frame to the next one so its a loop
            if last_good_page != 0 {
                unsafe {
                    let ptr = last_good_page as *mut usize;
                    *ptr = page_addr as usize;
                }
            }
            last_good_page = page_addr;
        }
    }
    unsafe {
        let ptr = last_good_page as *mut usize;
        *ptr = 0xdeadbeef;
    }

    println!("Phys Mem Initialized");
    println!("Usable pages: {:#x} Unusable pages: {:#x}", usable_pages, unusable_pages);
    (usable_pages, first_page)
}

fn get_page_table_min_max() -> (u64, u64) {
    let mut min: u64 = 0xffffffffffffffff;
    let mut max: u64 = 0x0;
    unsafe {
        let pml4 = current_page_table();

        // println!("PML4 at {:p}", pml4);
        let addr = pml4 as *const PML4 as u64 + 0x200;
        if addr < min {
            min = addr;
        }
        if addr > max {
            max = addr;
        }

        for pml4e in pml4.entries.iter() {
            if pml4e.present() {
                if let Some(pdpt) = pml4e.pdpt() {
                    // println!("PDPT at {:p}", pdpt);
                    let addr = pdpt as *const PDPT as u64 + 0x200;
                    if addr < min {
                        min = addr;
                    }
                    if addr > max {
                        max = addr;
                    }

                    for pdpte in pdpt.entries.iter() {
                        if pdpte.present() {
                            if let Some(pd) = pdpte.pd() {
                                // println!("PD at {:p}", pd);
                                let addr = pd as *const PD as u64 + 0x200;
                                if addr < min {
                                    min = addr;
                                }
                                if addr > max {
                                    max = addr;
                                }

                                for pde in pd.entries.iter() {
                                    if pde.present() {
                                        if !pde.big_page_enabled() {
                                            if let Some(pt) = pde.pt() {
                                                // println!("PT at {:p}", pt);
                                                let addr = pt as *const PT as u64 + 0x200;
                                                if addr < min {
                                                    min = addr;
                                                }
                                                if addr > max {
                                                    max = addr;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    (min & 0xfffffffffffff000, max & 0xfffffffffffff000)
}

fn check_page_table_overlap(page: u64) -> bool {
    unsafe {
        let pml4 = current_page_table();

        // println!("PML4 at {:p}", pml4);
        if page == pml4 as *const PML4 as u64 {
            return true
        }

        for pml4e in pml4.entries.iter() {
            if pml4e.present() {
                if let Some(pdpt) = pml4e.pdpt() {
                    // println!("PDPT at {:p}", pdpt);
                    if page == pdpt as *const PDPT as u64 {
                        return true
                    }

                    for pdpte in pdpt.entries.iter() {
                        if pdpte.present() {
                            if let Some(pd) = pdpte.pd() {
                                // println!("PD at {:p}", pd);
                                if page == pd as *const PD as u64 {
                                    return true
                                }

                                for pde in pd.entries.iter() {
                                    if pde.present() {
                                        if pde.big_page_enabled() {
                                        } else {
                                            if let Some(pt) = pde.pt() {
                                                // println!("PT at {:p}", pt);
                                                if page == pt as *const PT as u64 {
                                                    return true
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}
