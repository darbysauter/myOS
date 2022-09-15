use crate::{ println, print };
use crate::interrupts::*;
use crate::apic::apic_end_of_interrupt;

pub extern "x86-interrupt" fn bp_handler(sf: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", sf);
    loop{}
}

pub extern "x86-interrupt" fn de_handler(sf: InterruptStackFrame) {
    println!("EXCEPTION: DIVIDE\n{:#?}", sf);
    loop{}
}

pub extern "x86-interrupt" fn gp_handler(sf: InterruptStackFrame, error: u64) {
    println!("EXCEPTION: GP\n{:#?} error: {}", sf, error);
    loop{}
}

pub extern "x86-interrupt" fn pf_handler(sf: InterruptStackFrame, error: u64) {
    println!("EXCEPTION: PF\n{:#?} error: {}", sf, error);
    loop{}
}

pub extern "x86-interrupt" fn apic_timer_handler(_sf: InterruptStackFrame) {
    print!(".");
    unsafe {
        apic_end_of_interrupt(0xfee00000);
    }
}
