use crate::memory::page_table::{ PML4, VirtPage4KiB, PhysPage4KiB };
use core::slice;
use core::convert::TryInto;
use core::mem;
use crate::bootloader_structs::BootInfo;
use crate::elf::ProgHeaderEntry;
use alloc::vec::Vec;
use crate::memory::heap::{ HEAP_START, HEAP_SIZE };

pub fn map_heap(heap_regions: &Vec<(&PhysPage4KiB, usize)>, pml4: &mut PML4) {
    let mut vpage = HEAP_START;
    for (start_page, num_pages) in heap_regions {
        let start_page = *start_page as *const PhysPage4KiB as usize;
        for page in 0..*num_pages {
            let phys_page = start_page + (page * 0x1000);
            unsafe {
                // println!("mapping phys: {:#x} to virt: {:#x}", phys_page, vpage);
                pml4.map_frame_4k(phys_page, vpage, true, false, None);
            }
            vpage += 0x1000;
        }
    }
    if vpage != HEAP_START + HEAP_SIZE {
        panic!("Regions did not cover entire heap");
    }
}

pub fn map_elf_at_current_mapping(boot_info: &BootInfo, pml4: &mut PML4) {
    let e = {
        let ptr = boot_info.elf_location as *const u8;
        unsafe { slice::from_raw_parts(ptr, boot_info.elf_size as usize) }
    };

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
    
    for entry in prog_headers {
        if entry.seg_type == 0x1 {
            let start_page = entry.v_addr & 0xfffffffffffff000; // align to 0x1000
            let end_page = 
                (entry.v_addr + entry.mem_size as usize) & 0xfffffffffffff000; // align to 0x1000
            let pages = ((end_page - start_page) / 0x1000) + 1;

            for page in 0..pages {
                let phys_page = start_page + page * 0x1000;
                let virt_page = phys_page;
                unsafe {
                    pml4.map_frame_4k(phys_page, virt_page, true, false, None);
                }
            }
        }
    }
}

pub fn unmap_elf_at_original_mapping(prog_header_entries: &Vec<ProgHeaderEntry>, 
    pml4: &mut PML4, heap_regions: &Vec<(&PhysPage4KiB, usize)>) {
    
    for entry in prog_header_entries {
        if entry.seg_type == 0x1 {
            let start_page = entry.v_addr & 0xfffffffffffff000; // align to 0x1000
            let end_page = 
                (entry.v_addr + entry.mem_size as usize) & 0xfffffffffffff000; // align to 0x1000
            let pages = ((end_page - start_page) / 0x1000) + 1;

            for page in 0..pages {
                let phys_page = start_page + page * 0x1000;
                unsafe {
                    let virt_page = &(*(phys_page as *const VirtPage4KiB));
                    pml4.unmap_frame_4k(virt_page, Some(heap_regions));
                }
            }
        }
    }
}

pub const ELF_NEW_BASE: usize = 0xFFFF_8000_0000_0000;
pub const ELF_OLD_BASE: usize = 0x200000; // maybe dont have this hardcoded

pub fn map_elf_at_new_base(boot_info: &BootInfo, pml4: &mut PML4) {
    let e = {
        let ptr = boot_info.elf_location as *const u8;
        unsafe { slice::from_raw_parts(ptr, boot_info.elf_size as usize) }
    };

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
    
    for entry in prog_headers {
        if entry.seg_type == 0x1 {
            let start_page = entry.v_addr & 0xfffffffffffff000; // align to 0x1000
            let end_page = 
                (entry.v_addr + entry.mem_size as usize) & 0xfffffffffffff000; // align to 0x1000
            let pages = ((end_page - start_page) / 0x1000) + 1;

            for page in 0..pages {
                let phys_page = start_page + page * 0x1000;
                let seg_offset = start_page - ELF_OLD_BASE;
                let virt_page = ELF_NEW_BASE + seg_offset + page * 0x1000;
                unsafe {
                    pml4.map_frame_4k(phys_page, virt_page, true, false, None);
                }
            }
        }
    }
}


pub fn ident_map_vga_buf(pml4: &mut PML4) {
    unsafe {
        let phys_page = 0xb8000;
        let virt_page = phys_page;
        pml4.map_frame_4k(phys_page, virt_page, true, false, None);
    }
}
