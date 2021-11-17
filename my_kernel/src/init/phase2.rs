use crate::println;
use crate::memory::frame_allocator::LinkedListFrameAllocator;
use crate::memory::page_table::{ PML4, PhysPage4KiB };
use crate::alloc::vec::Vec;
use crate::interrupts::*;
use crate::gdt::*;
use crate::memory::heap::{ print_heap, heap_sanity_check };

// At this point we have elf loadable segments, heap and stack all mapped into high memory
// The Page tables are on the heap.
// The old elf loadable regions that were in low memory are unmapped
// It is trivial now to map pages, allocate pages, and allocate memory on heap
pub fn phase2_init(
    _pml4: &mut PML4, 
    frame_alloc: LinkedListFrameAllocator, 
    _heap_phys_regions: Vec<(&PhysPage4KiB, usize)> ) -> ! {

    // memory diagnostics
    println!("frame alloc has {:#x} free pages", frame_alloc.frame_count);
    heap_sanity_check();
    print_heap();
    
    // create new gdt and load it
    let mut gdt = GDT::create_gdt_on_heap();
    GDT::setup_gdt(&mut gdt);

    // create new idt and load it
    let mut idt = IDT::create_idt_on_heap();
    IDT::setup_idt(&mut idt);

    // unsafe {
    //     asm!("int3");
    // }

    println!("We didn't die! :)");
    loop {}
}