use alloc::vec::Vec;

use core::mem;
use core::slice;

use crate::elf::ProgHeaderEntry;
use crate::memory::frame_allocator::LinkedListFrameAllocator;
use crate::memory::heap::HEAP_SIZE;
use crate::memory::heap::HEAP_START;
use crate::memory::mappings::ident_map_vga_buf;
use crate::memory::page_table::PhysPage4KiB;
use crate::memory::page_table::PML4;
use crate::memory::stack::KERN_STACK_TOP;
use crate::memory::stack::STACK_SIZE;

use crate::println;

const ELF_STAGING_AREA: usize = 0x0000_4000_0000_0000;

const USER_PROG_AREA: usize = 0x0000_2000_0000_0000;

pub struct ElfLoader {}

impl ElfLoader {
    pub fn load(
        file_data: Vec<u8>,
        frame_alloc: &mut LinkedListFrameAllocator,
        kernel_pml4: &mut PML4,
        heap_regions: &Vec<(&'static PhysPage4KiB, usize)>,
        stack_phys: *const PhysPage4KiB,
    ) -> (&'static mut PML4, u64) {
        let magic: u32 =
            u32::from_le_bytes(file_data[0..4].try_into().expect("Couldn't get offset [1]"));
        assert_eq!(magic, 0x464C457F, "magic was {:#x}", magic);

        let prog_headers = ElfLoader::get_prog_header_entries(&file_data);

        let user_pml4 = unsafe { PML4::new(Some(heap_regions)) };

        ElfLoader::map_elf_and_copy(
            &prog_headers,
            frame_alloc,
            kernel_pml4,
            user_pml4,
            heap_regions,
            &file_data,
        );

        // TODO: enable once they have reloc
        // ElfLoader::fix_relocatable_addrs(&prog_headers);

        ElfLoader::map_kernel_stack(user_pml4, stack_phys, heap_regions);
        println!("about to map kern heap");
        ElfLoader::map_heap(heap_regions, user_pml4);
        println!("done map kern heap");
        ident_map_vga_buf(user_pml4, Some(heap_regions));

        let entry: u64 = u64::from_le_bytes(
            file_data[0x18..0x20]
                .try_into()
                .expect("Couldn't get offset"),
        );
        let entry = entry + USER_PROG_AREA as u64;

        (user_pml4, entry)
    }

    fn get_prog_header_entries(data: &Vec<u8>) -> Vec<ProgHeaderEntry> {
        let ph_off = usize::from_le_bytes(
            data[0x20..0x28]
                .try_into()
                .expect("Couldn't get offset [0]"),
        );

        let ph_ent_size = u16::from_le_bytes(
            data[0x36..0x38]
                .try_into()
                .expect("Couldn't get offset [0]"),
        );

        assert_eq!(ph_ent_size as usize, mem::size_of::<ProgHeaderEntry>());

        let ph_ent_num = u16::from_le_bytes(
            data[0x38..0x3a]
                .try_into()
                .expect("Couldn't get offset [0]"),
        );

        let prog_headers = {
            let ptr = (&data[0] as *const _ as usize + ph_off) as *const ProgHeaderEntry;
            unsafe { slice::from_raw_parts(ptr, ph_ent_num as usize) }
        };

        let mut vec = Vec::new();
        for entry in prog_headers {
            vec.push(entry.clone());
        }
        vec
    }

    fn map_elf_and_copy(
        prog_headers: &Vec<ProgHeaderEntry>,
        frame_alloc: &mut LinkedListFrameAllocator,
        kernel_pml4: &mut PML4,
        user_pml4: &mut PML4,
        heap_regions: &Vec<(&'static PhysPage4KiB, usize)>,
        data: &Vec<u8>,
    ) {
        for entry in prog_headers {
            if entry.seg_type == 0x1 {
                let start_page = entry.v_addr & 0xfffffffffffff000; // align to 0x1000
                let end_page = (entry.v_addr + entry.mem_size as usize) & 0xfffffffffffff000; // align to 0x1000
                let pages = ((end_page - start_page) / 0x1000) + 1;

                for page in 0..pages {
                    let seg_offset = start_page;
                    let staging_virt_page = ELF_STAGING_AREA + seg_offset + page * 0x1000;
                    let user_virt_page = USER_PROG_AREA + seg_offset + page * 0x1000;
                    let (_kern_virt_page, phys_page) = frame_alloc
                        .allocate_and_map(kernel_pml4, staging_virt_page, heap_regions)
                        .unwrap();
                    unsafe {
                        user_pml4.map_frame_4k(
                            phys_page,
                            user_virt_page,
                            true,
                            true,
                            Some(heap_regions),
                        );
                    }
                }

                let start_addr = entry.v_addr + ELF_STAGING_AREA;
                let start_index = entry.offset;
                // TODO: optimize to be not byte by byte
                for i in 0..entry.file_size as usize {
                    let addr = (i + start_addr) as *mut u8;
                    unsafe {
                        *addr = data[i + start_index];
                    }
                }
            }
        }
    }

    pub fn map_kernel_stack(
        pml4: &mut PML4,
        stack_phys: *const PhysPage4KiB,
        heap_regions: &Vec<(&'static PhysPage4KiB, usize)>,
    ) {
        let kern_top_page = KERN_STACK_TOP & 0xfffffffffffff000;
        let kern_bot_page = (KERN_STACK_TOP - STACK_SIZE) & 0xfffffffffffff000;
        let stack_top = stack_phys as usize;

        for offset in (0..kern_top_page - kern_bot_page).step_by(0x1000) {
            let paddr = stack_top - offset;
            let vaddr = kern_top_page - offset - 0x1000;
            unsafe {
                pml4.map_frame_4k(paddr, vaddr, true, true, Some(heap_regions));
            }
        }
    }

    pub fn map_heap(heap_regions: &Vec<(&'static PhysPage4KiB, usize)>, pml4: &mut PML4) {
        let mut vpage = HEAP_START;
        for (start_page, num_pages) in heap_regions {
            let start_page = *start_page as *const PhysPage4KiB as usize;
            for page in 0..*num_pages {
                let phys_page = start_page + (page * 0x1000);
                unsafe {
                    pml4.map_frame_4k(phys_page, vpage, true, true, Some(heap_regions));
                }
                vpage += 0x1000;
            }
        }
        if vpage != HEAP_START + HEAP_SIZE {
            panic!("Regions did not cover entire heap");
        }
    }
}
