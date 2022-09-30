#![no_std]
#![no_main]

use core::panic::PanicInfo;

use user_lib::syscalls::print;

#[no_mangle]
pub extern "sysv64" fn _start() -> ! {
    print();
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
