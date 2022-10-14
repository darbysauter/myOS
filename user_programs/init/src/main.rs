#![no_std]
#![no_main]

use core::panic::PanicInfo;

use user_lib::syscalls::{create_proc, print};

#[no_mangle]
pub extern "sysv64" fn _start() -> ! {
    print();
    create_proc();
    print();
    create_proc();
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
