use core::mem;
use core::marker::PhantomData;
use crate::println;
use crate::memory::heap::translate_ref_to_phys;
use alloc::vec::Vec;
use crate::memory::page_table::PhysPage4KiB;

// abort - trap gate that saves unrelated IP (basically NMI and such)
// fault - trap gate that saves the current IP
// trap - trap gate that saves the next IP
// interrupt - interrupt gate that saves the next IP

// It would be better if Intel engineers were not incompetent, 
// and they should have called the IDT entries "exception gate" and 
// "interrupt gate" or they should have classify exceptions as fault, 
// continuable and abort (or something like that). It is very confusing that 
// they call all exception descriptors "trap" gate, but also a kind of exception 
// is named "trap" too, which happens to act just like the interrupt gate from the 
// continuation point of view... It is a complete naming mess.


#[derive(Debug)]
#[repr(C)]
pub struct InterruptStackFramePtr {
    inner: InterruptStackFrame,
}

#[derive(Debug)]
#[repr(C)]
pub struct InterruptStackFrame {
    pub rip: u64,
    pub cs: u64,
    pub eflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct IDTEntryOptions {
    data: u16
}

impl IDTEntryOptions {
    #[inline]
    const fn default() -> Self {
        IDTEntryOptions{
            data: 0b1110_0000_0000, // Type for 64bit Interrupt Gate
        }
    }
    
    #[inline]
    pub fn set_present(&mut self, present: bool) -> &mut Self {
        if present {
            self.data |= 1 << 15;
        } else {
            self.data &= !(1 << 15);
        }
        self
    }

    // toggles between trap(disabled) and interrupt(enabled) type
    // this trap type is different than a trap exception
    // this type just means that when entered interrupts are
    // disabled if the type is interrupt or enabled if the type is trap
    #[inline]
    pub fn disable_interrupts(&mut self, disable: bool) -> &mut Self {
        if !disable {
            self.data |= 1 << 8;
        } else {
            self.data &= !(1 << 8);
        }
        self
    }

    #[inline]
    pub fn set_dpl(&mut self, level: u8) -> &mut Self {
        if level > 3 {
            panic!("Invalid priv level");
        }
        let range = 13..15;
        let bitmask = !(!0 << (16 - range.end) >>
                                    (16 - range.end) >>
                                    range.start << range.start);

        self.data = (self.data & bitmask) | ((level as u16) << range.start);
        self
    }

    #[inline]
    pub unsafe fn set_stack_index(&mut self, index: u16) -> &mut Self {
        let range = 0..3;
        let bitmask = !(!0 << (16 - range.end) >>
                                    (16 - range.end) >>
                                    range.start << range.start);

        self.data = (self.data & bitmask) | (index << range.start);
        self
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct IDTEntry<F> {
    addr_low: u16,
    gdt_selector: u16,
    options: IDTEntryOptions,
    addr_mid: u16,
    addr_high: u32,
    reserved: u32,
    phantom: PhantomData<F>,
}

impl<F> IDTEntry<F> {
    #[inline]
    pub const fn empty() -> Self {
        IDTEntry {
            addr_low: 0,
            gdt_selector: 0,
            options: IDTEntryOptions::default(),
            addr_mid: 0,
            addr_high: 0,
            reserved: 0,
            phantom: PhantomData,
        }
    }
}

#[repr(align(8))]
#[repr(C)]
struct InterruptDescriptorTable {
    divide_error: IDTEntry<HandlerFunc>,
    debug_excepton: IDTEntry<HandlerFunc>,
    nmi: IDTEntry<HandlerFunc>,
    breakpoint: IDTEntry<HandlerFunc>,
    overflow: IDTEntry<HandlerFunc>,
    bound_range: IDTEntry<HandlerFunc>,
    invalid_opcode: IDTEntry<HandlerFunc>,
    device_not_available: IDTEntry<HandlerFunc>,
    double_fault: IDTEntry<DivergingHandlerFuncWithErrCode>,
    coprocessor_segment_overrun: IDTEntry<HandlerFunc>,
    invalid_tss: IDTEntry<HandlerFuncWithErrCode>,
    segment_not_present: IDTEntry<HandlerFuncWithErrCode>,
    stack_fault: IDTEntry<HandlerFuncWithErrCode>,
    general_protection: IDTEntry<HandlerFuncWithErrCode>,
    page_fault: IDTEntry<PageFaultHandlerFunc>,
    x87_fpu_error: IDTEntry<HandlerFunc>,
    reserved: IDTEntry<HandlerFunc>,
    alignment_check: IDTEntry<HandlerFuncWithErrCode>,
    machine_check: IDTEntry<DivergingHandlerFunc>,
    simd_fpu: IDTEntry<HandlerFunc>,
    virtualization: IDTEntry<HandlerFunc>, // 21
    reserved_arr: [IDTEntry<HandlerFunc>; 11], // 32 or index 31
    extra: [IDTEntry<HandlerFunc>; 256-32],
}

// #[repr(align(8))]
#[repr(C, packed)]
struct InterruptDescriptorTablePtr {
    limit: u16,
    addr: u64,
}

#[repr(align(8))]
#[repr(C)]
pub struct IDT {
    ptr: InterruptDescriptorTablePtr,
    table: InterruptDescriptorTable,
}

pub type HandlerFunc = extern "x86-interrupt" fn(InterruptStackFramePtr);
pub type DivergingHandlerFunc = extern "x86-interrupt" fn(InterruptStackFramePtr) -> !;
pub type PageFaultHandlerFunc = extern "x86-interrupt" fn(_: InterruptStackFramePtr, error: u64);
pub type HandlerFuncWithErrCode = extern "x86-interrupt" fn(_: InterruptStackFramePtr, error: u64);
pub type DivergingHandlerFuncWithErrCode = extern "x86-interrupt" fn(_: InterruptStackFramePtr, error: u64) -> !;

impl IDT {
    pub fn new() -> Self {
        IDT {
            ptr: InterruptDescriptorTablePtr {
                limit: 0,
                addr: 0,
            },
            table: InterruptDescriptorTable {
                divide_error: IDTEntry::empty(),
                debug_excepton: IDTEntry::empty(),
                nmi: IDTEntry::empty(),
                breakpoint: IDTEntry::empty(),
                overflow: IDTEntry::empty(),
                bound_range: IDTEntry::empty(),
                invalid_opcode: IDTEntry::empty(),
                device_not_available: IDTEntry::empty(),
                double_fault: IDTEntry::empty(),
                coprocessor_segment_overrun: IDTEntry::empty(),
                invalid_tss: IDTEntry::empty(),
                segment_not_present: IDTEntry::empty(),
                stack_fault: IDTEntry::empty(),
                general_protection: IDTEntry::empty(),
                page_fault: IDTEntry::empty(),
                reserved: IDTEntry::empty(),
                x87_fpu_error: IDTEntry::empty(),
                alignment_check: IDTEntry::empty(),
                machine_check: IDTEntry::empty(),
                simd_fpu: IDTEntry::empty(),
                virtualization: IDTEntry::empty(),
                reserved_arr: [IDTEntry::empty(); 11],
                extra: [IDTEntry::empty(); 256-32],
            },
        }
    }

    pub fn load(&mut self, heap_regions: &Vec<(&PhysPage4KiB, usize)>) {
        // loop{}
        unsafe {
            let translated = translate_ref_to_phys(heap_regions, &mut self.table);
            self.ptr.addr = translated as *const _ as u64;
        }
        self.ptr.limit = (mem::size_of::<InterruptDescriptorTable>() - 1) as u16;

        let ptr = (&self.ptr) as *const _ as usize;
        if ptr % 0x8 != 0 {
            panic!("IDT pointer not aligned");
        }
        unsafe {
            asm!( "lidt [{}]", in(reg) ptr);
        }
    }

    pub fn set_breakpoint_handler(&mut self, handler: HandlerFunc) -> &mut IDTEntryOptions {
        let addr = handler as usize;
        println!("addr: {:#x}", addr);

        self.table.breakpoint.addr_low = addr as u16;
        self.table.breakpoint.addr_mid = (addr >> 16) as u16;
        self.table.breakpoint.addr_high = (addr >> 32) as u32;

        self.table.breakpoint.gdt_selector = 0x08;
        self.table.breakpoint.options.set_present(true);
        &mut self.table.breakpoint.options
    }
}