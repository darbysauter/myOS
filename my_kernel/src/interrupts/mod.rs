pub mod interrupt_handlers;

use core::mem;
use core::fmt;
use core::marker::PhantomData;
use alloc::boxed::Box;
use crate::interrupts::interrupt_handlers::*;

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


// #[derive(Debug)]
// #[repr(C)]
// pub struct InterruptStackFramePtr {
//     inner: InterruptStackFrame,
// }

pub fn enable_hardware_interrupts() {
    unsafe {
        asm!("sti");
    }
}


impl fmt::Debug for InterruptStackFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "StackFrame[\n    RIP: {:#x}\n    CS: {:#x}\n    EFLAGS: {:#x}\n    RSP: {:#x}\n    SS: {:#x}\n]",
            self.rip,
            self.cs,
            self.eflags,
            self.rsp,
            self.ss))
    }
}

// #[derive(Debug)]
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

pub type HandlerFunc = extern "x86-interrupt" fn(InterruptStackFrame);
pub type DivergingHandlerFunc = extern "x86-interrupt" fn(InterruptStackFrame) -> !;
pub type PageFaultHandlerFunc = extern "x86-interrupt" fn(_: InterruptStackFrame, error: u64);
pub type HandlerFuncWithErrCode = extern "x86-interrupt" fn(_: InterruptStackFrame, error: u64);
pub type DivergingHandlerFuncWithErrCode = extern "x86-interrupt" fn(_: InterruptStackFrame, error: u64) -> !;

#[repr(usize)]
pub enum ExtraInterrupts {
    ApicTimer = 32,
}

impl IDT {
    fn new() -> Self {
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

    pub fn load(&mut self) {
        let ptr = &self.table as *const _ as u64;
        self.ptr.addr = ptr;
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

        self.table.breakpoint.addr_low = addr as u16;
        self.table.breakpoint.addr_mid = (addr >> 16) as u16;
        self.table.breakpoint.addr_high = (addr >> 32) as u32;

        self.table.breakpoint.gdt_selector = 0x08;
        self.table.breakpoint.options.set_present(true);
        &mut self.table.breakpoint.options
    }

    pub fn set_divide_error_handler(&mut self, handler: HandlerFunc) -> &mut IDTEntryOptions {
        let addr = handler as usize;

        self.table.divide_error.addr_low = addr as u16;
        self.table.divide_error.addr_mid = (addr >> 16) as u16;
        self.table.divide_error.addr_high = (addr >> 32) as u32;

        self.table.divide_error.gdt_selector = 0x08;
        self.table.divide_error.options.set_present(true);
        &mut self.table.divide_error.options
    }


    pub fn set_general_protection_handler(&mut self, handler: HandlerFuncWithErrCode) -> &mut IDTEntryOptions {
        let addr = handler as usize;

        self.table.general_protection.addr_low = addr as u16;
        self.table.general_protection.addr_mid = (addr >> 16) as u16;
        self.table.general_protection.addr_high = (addr >> 32) as u32;

        self.table.general_protection.gdt_selector = 0x08;
        self.table.general_protection.options.set_present(true);
        self.table.general_protection.options.set_dpl(3);
        &mut self.table.general_protection.options
    }

    pub fn set_page_fault_handler(&mut self, handler: PageFaultHandlerFunc) -> &mut IDTEntryOptions {
        let addr = handler as usize;

        self.table.page_fault.addr_low = addr as u16;
        self.table.page_fault.addr_mid = (addr >> 16) as u16;
        self.table.page_fault.addr_high = (addr >> 32) as u32;

        self.table.page_fault.gdt_selector = 0x08;
        self.table.page_fault.options.set_present(true);
        self.table.page_fault.options.set_dpl(3);
        &mut self.table.page_fault.options
    }

    pub fn set_extra_handler(&mut self, handler: HandlerFunc, index: ExtraInterrupts) -> &mut IDTEntryOptions {
        let addr = handler as usize;
        let index = index as usize - 32;

        self.table.extra[index].addr_low = addr as u16;
        self.table.extra[index].addr_mid = (addr >> 16) as u16;
        self.table.extra[index].addr_high = (addr >> 32) as u32;

        self.table.extra[index].gdt_selector = 0x08;
        self.table.extra[index].options.set_present(true);
        &mut self.table.extra[index].options
    }

    pub fn create_idt_on_heap() -> Box<IDT> {
        Box::new(IDT::new())
    }

    pub fn setup_idt(idt: &mut Box<IDT>) {
        idt.set_breakpoint_handler(bp_handler);
        idt.set_divide_error_handler(de_handler);
        idt.set_general_protection_handler(gp_handler);
        idt.set_page_fault_handler(pf_handler);
        idt.set_extra_handler(apic_timer_handler, ExtraInterrupts::ApicTimer);
        idt.load();
    }
}
