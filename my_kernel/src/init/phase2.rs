use crate::println;
use crate::memory::frame_allocator::LinkedListFrameAllocator;
use crate::memory::page_table::{ PML4, PhysPage4KiB };
use crate::alloc::vec::Vec;
use crate::interrupts::*;
use crate::gdt::*;
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

    // POSSIBLY OVER FREEING, NOT GOOD BECAUSE STUFF IN USE MIGHT BE ON THE FREE LISTS
    // LIKELY STUFF MIGHT BE FREED DURING THE REMAPPING 
    let mut gdt = Box::new(GlobalDescriptorTable::new());
    gdt.load();
    gdt.reload_segments();


    let mut idt = Box::new(IDT::new());
    idt.set_breakpoint_handler(bp_handler);
    idt.set_divide_error_handler(de_handler);
    idt.load(&heap_phys_regions);

    // unsafe {
    //     asm!("int3");
    // }

    // unsafe {
    //     asm!(   "mov edx, 0",
    //             "mov eax, 0x8003",
    //             "mov ecx, 0",
    //             "div ecx");
    // }

    println!("We didn't die! :)");
    loop {}
}

extern "x86-interrupt" fn bp_handler(sf: InterruptStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", sf);
    loop{}
}


extern "x86-interrupt" fn de_handler(sf: InterruptStackFrame)
{
    println!("EXCEPTION: DIVIDE\n{:#?}", sf);
    loop{}
}