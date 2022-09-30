#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
pub extern "sysv64" fn _start() -> ! {
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
