use crate::alloc::boxed::Box;
use crate::alloc::vec::Vec;
use crate::apic::{check_apic, get_apic_base, ident_map_apic_page};
use crate::bootloader_structs::BootInfo;
use crate::elf::{fix_relocatable_addrs, get_loadable_prog_header_entries, ProgHeaderEntry};
use crate::init::phase2::phase2_init;
use crate::memory::frame_allocator::LinkedListFrameAllocator;
use crate::memory::heap::{
    fix_heap_after_remap, init_heap_phase1, init_heap_phase2, translate_box, translate_box_vec,
};
use crate::memory::mappings::{
    ident_map_vga_buf, map_elf_at_current_mapping, map_elf_at_new_base, map_heap,
    unmap_elf_at_original_mapping, ELF_NEW_BASE, ELF_OLD_BASE,
};
use crate::memory::page_table::{PhysPage4KiB, PML4};
use crate::memory::stack::{create_new_stack_and_map, KERN_STACK_TOP};
use crate::println;
use core::arch::asm;

pub fn phase1_init(boot_info: &BootInfo) -> ! {
    let mut frame_allocator = LinkedListFrameAllocator::init(boot_info);

    let (heap_phys, num_pages_1) = init_heap_phase1(&mut frame_allocator);
    let mut heap_phys_regions = init_heap_phase2(&mut frame_allocator, num_pages_1);
    heap_phys_regions.push((heap_phys, num_pages_1));

    unsafe {
        let pml4 = PML4::new(None);
        // map in stack, ELF regions twice, pagetable recursively

        let stack_phys = create_new_stack_and_map(&mut frame_allocator, pml4);

        map_heap(&heap_phys_regions, pml4);
        map_elf_at_current_mapping(boot_info, pml4);
        map_elf_at_new_base(boot_info, pml4);
        ident_map_vga_buf(pml4, None);

        if check_apic() {
            println!("APIC AVALIBLE");
        } else {
            panic!("APIC NOT AVALIBLE");
        }

        let apic_base = get_apic_base();
        println!("APIC base: {:#x}", apic_base);

        ident_map_apic_page(apic_base, pml4);

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
            in(reg) KERN_STACK_TOP,
            in(reg) phase_2_entry_point,
            in("rdi") pml4_boxed,
            in("rsi") heap_regions_boxed,
            in("rdx") frame_alloc_boxed,
            in("rcx") prog_header_entries_boxed,
            in("r8") stack_phys
        );
    }
    loop {}
}

/// # Safety
/// `heap_phys_regions`, `frame_alloc`, `prog_header_entries` and `stack_phys` must be valid pointers
/// Arguments are passed in order of:
/// RDI, RSI, RDX, RCX, R8, R9
#[no_mangle]
pub unsafe extern "sysv64" fn phase_2_transition(
    pml4: &mut PML4,
    heap_phys_regions: *mut Vec<(&'static PhysPage4KiB, usize)>,
    frame_alloc: *mut LinkedListFrameAllocator,
    prog_header_entries: *mut Vec<ProgHeaderEntry>,
    stack_phys: *const PhysPage4KiB,
) -> ! {
    println!("Entering Phase 2!");

    let frame_alloc = Box::from_raw(frame_alloc);
    let frame_alloc = Box::into_inner(frame_alloc);

    let heap_phys_regions = Box::from_raw(heap_phys_regions);
    let heap_phys_regions: Vec<(&'static PhysPage4KiB, usize)> = Box::into_inner(heap_phys_regions);

    let prog_header_entries = Box::from_raw(prog_header_entries);
    let prog_header_entries: Vec<ProgHeaderEntry> = Box::into_inner(prog_header_entries);

    unmap_elf_at_original_mapping(&prog_header_entries, pml4, &heap_phys_regions);

    fix_heap_after_remap(&heap_phys_regions);

    phase2_init(
        pml4,
        frame_alloc,
        heap_phys_regions,
        prog_header_entries,
        stack_phys,
    )
}
