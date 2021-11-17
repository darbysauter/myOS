use crate::println;
use crate::interrupts::*;

pub extern "x86-interrupt" fn bp_handler(sf: InterruptStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", sf);
    loop{}
}


pub extern "x86-interrupt" fn de_handler(sf: InterruptStackFrame)
{
    println!("EXCEPTION: DIVIDE\n{:#?}", sf);
    loop{}
}