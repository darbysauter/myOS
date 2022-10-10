use alloc::vec::Vec;

use crate::cpu::{read_msr, write_msr};
use crate::gdt::{USER_CODE_SEL, USER_DATA_SEL};
use crate::memory::heap::translate_usize_to_phys;
use crate::memory::mappings::{ELF_NEW_BASE, ELF_OLD_BASE};
use crate::memory::page_table::{current_page_table, PhysPage4KiB, PML4};
use crate::memory::stack::USER_STACK_TOP;
use crate::println;
use core::arch::{asm, global_asm};

pub fn enter_user_mode(
    entry_point: u64,
    user_pml4: &mut PML4,
    heap_regions: &Vec<(&'static PhysPage4KiB, usize)>,
) -> ! {
    unsafe {
        // save kernel cr3
        kern_cr3 = current_page_table() as *const _ as usize;
        let cr3 = user_pml4 as *const _ as usize;
        let cr3 = translate_usize_to_phys(heap_regions, cr3);
        let user_data_sel: u64 = USER_DATA_SEL;
        let user_code_sel: u64 = USER_CODE_SEL;
        let user_stack: usize = USER_STACK_TOP;

        asm!(
            "mov cr3, {:r}",
            "push {:r}",
            "push {:r}",
            "pushfq",
            "push {:r}",
            "push {:r}",
            "iretq",
            in(reg) cr3,
            in(reg) user_data_sel,
            in(reg) user_stack,
            in(reg) user_code_sel,
            in(reg) entry_point);
    }
    loop {}
}

pub fn enable_syscalls() {
    let addr_to_exec: usize = syscall_test as *const () as usize;
    unsafe {
        // enable syscall extension
        asm!("rdmsr", "or rax, 1", "wrmsr", in("rcx") 0xC0000080 as u32, in("rax") 0);

        // set syscall/sysret seg selectors
        write_msr(0xC0000081, 0x0010000800000000);

        // set syscall handler
        write_msr(0xC0000082, addr_to_exec as u64);
    }
}

extern "C" {
    fn syscall_test();
    static mut kern_cr3: usize;
}

global_asm!(
    ".data

    .global
    rsp_storage:
    .quad  0xffeeddccbbaa9988

    .global
    kern_cr3:
    .quad  0xffeeddccbbaa9988

    .global
    user_cr3:
    .quad  0xffeeddccbbaa9988

    .global
    syscall_stack:
    .quad  0xFFFFF00000000000


    .text

    .global
    syscall_test:
    mov rsp_storage[rip], rsp
    mov rsp, syscall_stack[rip]
    push rcx
    push r11
    mov rcx, 0x10
    mov ds, rcx
    mov es, rcx
    mov fs, rcx
    mov gs, rcx
    mov rcx, cr3
    mov user_cr3[rip], rcx
    mov rcx, kern_cr3[rip]
    mov cr3, rcx
    mov rcx, r10
    call syscall_handler
    mov rcx, user_cr3[rip]
    mov cr3, rcx
    pop r11
    pop rcx
    mov rsp, rsp_storage[rip]
    sysretq"
);

#[no_mangle]
extern "C" fn syscall_handler(syscall: u64) -> u64 {
    println!("Syscall num: {}", syscall);

    let ret: u64 = 0x11223344AABBCCDD;
    ret
}

fn test_syscall(syscall: u64) -> u64 {
    let mut ret: u64;
    unsafe {
        asm!(
            "mov rax, {:r}",
            "syscall",
            "mov {:r}, rax",
            in(reg) syscall,
            out(reg) ret
        );
    }
    ret
}

fn execute_in_user() {
    let mut ret_val = 0;
    while ret_val == 0 {
        ret_val = test_syscall(1234);
    }
    loop {}
}
