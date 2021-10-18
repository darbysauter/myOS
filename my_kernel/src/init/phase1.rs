use crate::init::phase2::phase2_init;
use crate::println;
use crate::memory::frame_allocator::LinkedListFrameAllocator;
use crate::memory::heap::{ init_heap_phase1, init_heap_phase2, translate_box, 
    translate_box_vec, fix_heap_after_remap };
use crate::memory::mappings::{ map_heap, map_elf_at_current_mapping, ident_map_vga_buf,
    map_elf_at_new_base, ELF_NEW_BASE, ELF_OLD_BASE, unmap_elf_at_original_mapping };
use crate::memory::stack::{ create_new_stack_and_map, KERN_STACK_TOP };
use crate::alloc::vec::Vec;
use crate::alloc::boxed::Box;
use crate::elf::{ get_loadable_prog_header_entries, ProgHeaderEntry, fix_relocatable_addrs };
use crate::memory::page_table::{ PML4, PhysPage4KiB };
use crate::bootloader_structs::BootInfo;

pub fn phase1_init(boot_info: &BootInfo) -> ! {
    let mut frame_allocator = unsafe {
        LinkedListFrameAllocator::init(boot_info)
    };

    let (heap_phys, num_pages_1) = init_heap_phase1(&mut frame_allocator);

    let mut heap_phys_regions = init_heap_phase2(&mut frame_allocator, num_pages_1);
    heap_phys_regions.push((heap_phys, num_pages_1));

    unsafe {
        let pml4 = PML4::new(None);
        // map in stack, ELF regions twice, pagetable recursively

        let _stack_phys = create_new_stack_and_map(&mut frame_allocator, pml4);

        map_heap(&heap_phys_regions, pml4);
        map_elf_at_current_mapping(boot_info, pml4);
        map_elf_at_new_base(boot_info, pml4);
        ident_map_vga_buf(pml4);

        // NEED TO TRANSLATE ALL HEAP ADDRESSES TO NEW HEAP LOCATION
        // We have the pagetable on heap

        let frame_alloc_boxed: Box<LinkedListFrameAllocator> = Box::new(frame_allocator);
        // this is invalid in the current context
        let frame_alloc_boxed = translate_box(&heap_phys_regions, frame_alloc_boxed);

        let pml4_boxed: Box<PML4> = Box::from_raw(pml4 as *mut PML4);
        // this is invalid in the current context
        let pml4_boxed = translate_box(&heap_phys_regions, pml4_boxed);

        let prog_header_entries = get_loadable_prog_header_entries(boot_info);
        let prog_header_entries_boxed = translate_box_vec(&heap_phys_regions, prog_header_entries);

        let clone = heap_phys_regions.clone();
        // this is invalid in the current context
        let heap_regions_boxed = translate_box_vec(&heap_phys_regions, clone);

        let pml4_phys = pml4 as *const PML4 as usize;

        let phase_2_offset = ELF_NEW_BASE - ELF_OLD_BASE;
        let phase_2_entry_point = (phase_2_transition as *const () as usize) + phase_2_offset;

        fix_relocatable_addrs(boot_info, ELF_NEW_BASE - ELF_OLD_BASE);

        asm!(
            "mov cr3, {}; mov rsp, {}; push 0; jmp {}",
            in(reg) pml4_phys,
            in(reg) KERN_STACK_TOP as usize,
            in(reg) phase_2_entry_point,
            in("rdi") pml4_boxed as *mut PML4 as usize,
            in("rsi") heap_regions_boxed as *mut Vec<(&PhysPage4KiB, usize)> as usize,
            in("rdx") frame_alloc_boxed as *mut LinkedListFrameAllocator as usize,
            in("rcx") prog_header_entries_boxed as *mut Vec<(&PhysPage4KiB, usize)> as usize
        );
    }
    loop {}
}


// Arguments are passed in order of:
// RDI, RSI, RDX, RCX, R8, R9
#[no_mangle]
pub extern "sysv64" fn phase_2_transition(pml4: &mut PML4, 
    heap_phys_regions: *mut Vec<(&PhysPage4KiB, usize)>,
    frame_alloc: *mut LinkedListFrameAllocator,
    prog_header_entries: *mut Vec<ProgHeaderEntry>) -> ! {
    println!("Entering Phase 2!");

    let frame_alloc = unsafe { Box::from_raw(frame_alloc) };
    let frame_alloc = Box::into_inner(frame_alloc);

    let heap_phys_regions = unsafe { Box::from_raw(heap_phys_regions) };
    let heap_phys_regions: Vec<(&PhysPage4KiB, usize)> = Box::into_inner(heap_phys_regions);

    let prog_header_entries = unsafe { Box::from_raw(prog_header_entries) };
    let prog_header_entries: Vec<ProgHeaderEntry> = Box::into_inner(prog_header_entries);

    unmap_elf_at_original_mapping(&prog_header_entries, pml4, &heap_phys_regions);

    fix_heap_after_remap(&heap_phys_regions);

    phase2_init(pml4, frame_alloc, heap_phys_regions)
}