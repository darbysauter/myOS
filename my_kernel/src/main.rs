#![no_std]
#![no_main]

use core::panic::PanicInfo;
use my_kernel::{ println, BootInfo, init};

// Force calling convention to sysv64
// Arguments are passed in order of:
// RDI, RSI, RDX, RCX, R8, R9
// We use RDI to pass BootInfo
#[no_mangle]
pub extern "sysv64" fn _start(boot_info: &BootInfo) -> ! {

    println!("<- (-_-) -> Hello From Rust Kernel!");

    init(boot_info)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

