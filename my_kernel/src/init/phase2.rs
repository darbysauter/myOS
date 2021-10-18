use crate::println;
use crate::memory::frame_allocator::LinkedListFrameAllocator;
use crate::memory::page_table::{ PML4, PhysPage4KiB };
use crate::alloc::vec::Vec;
use crate::interrupts::*;
use alloc::boxed::Box;
use crate::memory::heap::print_heap;

// At this point we have elf loadable segments, heap and stack all mapped into high memory
// The Page tables are on the heap.
// The old elf loadable regions that were in low memory are unmapped
// It is trivial now to map pages, allocate pages, and allocate memory on heap
pub fn phase2_init(
    _pml4: &mut PML4, 
    frame_alloc: LinkedListFrameAllocator, 
    heap_phys_regions: Vec<(&PhysPage4KiB, usize)> ) -> ! {

    println!("frame alloc has {:#x} free pages", frame_alloc.frame_count);

    print_heap();
    for i in 0..100_000 {
        let b = Box::new(1);
        let b1 = Box::new(1);
        let b2 = Box::new(1);
        let b3 = Box::new(1);
        let b4 = Box::new(1);
        let b5 = Box::new(1);
        let b6 = Box::new(1);
        let b7 = Box::new(1);
        let b8 = Box::new(1);
    }
    print_heap();
    let mut idt = Box::new(IDT::new());
    idt.set_breakpoint_handler(bp_handler);
    loop{}
    idt.load(&heap_phys_regions);

    unsafe {
        asm!("int3");
    }

    println!("We didn't die! :)");
    loop {}
}

extern "x86-interrupt" fn bp_handler(
    stack_frame: InterruptStackFramePtr)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
    loop{}
}