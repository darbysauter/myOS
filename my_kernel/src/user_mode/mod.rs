use crate::cpu::{read_msr, write_msr};
use crate::gdt::{USER_CODE_SEL, USER_DATA_SEL};
use crate::memory::mappings::{ELF_NEW_BASE, ELF_OLD_BASE};
use crate::memory::stack::USER_STACK_TOP;
use crate::println;

pub fn enter_user_mode() -> ! {
    unsafe {
        let user_data_sel: u64 = USER_DATA_SEL;
        let user_code_sel: u64 = USER_CODE_SEL;
        let user_stack: usize = USER_STACK_TOP;
        let addr_to_exec: usize = (execute_in_user as *const () as usize);
        asm!(
            "push {:r}",
            "push {:r}",
            "pushfq",
            "push {:r}",
            "push {:r}",
            "iretq",
            in(reg) user_data_sel,
            in(reg) user_stack,
            in(reg) user_code_sel,
            in(reg) addr_to_exec);
    }
    loop {}
}

pub fn enable_syscalls() {
    let addr_to_exec: usize = (syscall_test as *const () as usize);
    unsafe {
        // enable syscall extension
        asm!("mov rcx, 0xC0000080", "rdmsr", "or rax, 1", "wrmsr",);

        // set syscall/sysret seg selectors
        write_msr(0xC0000081, 0x0010000800000000);

        // set syscall handler
        write_msr(0xC0000082, addr_to_exec as u64);
    }
}

extern "C" {
    fn syscall_test();
}

global_asm!(
    ".data",
    ".global",
    "rsp_storage:",
    ".quad  0xffeeddccbbaa9988",
    ".global",
    "rcx_storage:",
    ".quad  0xffeeddccbbaa9988",
    ".text",
    ".global",
    "syscall_test:",
    "mov rsp_storage[rip], rsp",
    "mov rcx_storage[rip], rcx",
    "call syscall_handler",
    "mov rsp, rsp_storage[rip]",
    "mov rcx, rcx_storage[rip]",
    "sysretq"
);

#[no_mangle]
extern "C" fn syscall_handler(syscall: u64) -> u64 {
    let mut return_addr: u64;
    unsafe {
        asm!(
            "mov {:r}, rcx",
            out(reg) return_addr,
        );
    }

    println!("ret addr: {:#x}", return_addr);
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
