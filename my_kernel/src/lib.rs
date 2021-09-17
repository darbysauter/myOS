#![no_std]
#![cfg_attr(test, no_main)]
#![feature(asm)]
#![feature(alloc_error_handler)]
#![feature(const_mut_refs)]
#![feature(allocator_api)]
#![feature(slice_ptr_get)]
#![feature(abi_x86_interrupt)]
#![feature(box_into_inner)]
#![feature(vec_into_raw_parts)]

use init::phase1::phase1_init;
pub use bootloader_structs::BootInfo;

pub mod vga_buffer;
pub mod memory;
pub mod elf;
pub mod init;
pub mod bootloader_structs;
pub mod interrupts;

extern crate alloc;

pub fn init(boot_info: &BootInfo) -> ! {
    phase1_init(boot_info);
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
