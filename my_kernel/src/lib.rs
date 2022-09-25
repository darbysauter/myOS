#![no_std]
#![cfg_attr(test, no_main)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(alloc_error_handler)]
#![feature(const_mut_refs)]
#![feature(allocator_api)]
#![feature(slice_ptr_get)]
#![feature(abi_x86_interrupt)]
#![feature(box_into_inner)]
#![feature(vec_into_raw_parts)]

pub use bootloader_structs::BootInfo;
use init::phase1::phase1_init;

pub mod ahci;
pub mod apic;
pub mod bootloader_structs;
pub mod cpu;
pub mod elf;
pub mod gdt;
pub mod init;
pub mod interrupts;
pub mod memory;
pub mod pci;
pub mod tss;
pub mod user_mode;
pub mod vga_buffer;

extern crate alloc;

pub fn init(boot_info: &BootInfo) -> ! {
    phase1_init(boot_info);
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
