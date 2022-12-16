use alloc::vec::Vec;

use crate::apic::{
    disable_pic, enable_apic, get_apic_base, set_apic_base, set_apic_tpr, start_apic_timer,
};
use crate::cpu::write_msr;
use crate::gdt::{USER_CODE_SEL, USER_DATA_SEL};
use crate::interrupts::enable_hardware_interrupts;
use crate::memory::heap::translate_usize_to_phys;

use crate::memory::page_table::{current_page_table, PhysPage4KiB, PML4};
use crate::memory::stack::{KERN_STACK_TOP, USER_STACK_TOP};
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
        syscall_stack = KERN_STACK_TOP;
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
    static mut syscall_stack: usize;
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
    mov r9, rax
    call syscall_handler
    mov rcx, user_cr3[rip]
    mov cr3, rcx
    pop r11
    pop rcx
    mov rsp, rsp_storage[rip]
    sysretq"
);

#[repr(u64)]
#[derive(Debug)]
enum Syscall {
    Print = 0,
    CreateProc = 1,
    EnableTimer = 2,
}

#[no_mangle]
extern "sysv64" fn syscall_handler(
    _arg0: u64,
    _arg1: u64,
    _arg2: u64,
    _arg3: u64,
    _arg4: u64,
    syscall: Syscall,
) -> u64 {
    match syscall {
        Syscall::Print => println!("Syscall num: {:#?}", syscall),
        Syscall::CreateProc => println!("Syscall num: {:#?}", syscall),
        Syscall::EnableTimer => {
            println!("Enabling timer");
            let apic_base = get_apic_base();
            set_apic_base(apic_base);
            unsafe {
                enable_apic(apic_base);
                set_apic_tpr(apic_base, 0);
            }
            disable_pic();
            println!("Enabling hwi");
            enable_hardware_interrupts();
            println!("starting timer");
            start_apic_timer(apic_base);
        }
    }

    let ret: u64 = 0x11223344AABBCCDD;
    ret
}

// fn test_syscall(syscall: u64) -> u64 {
//     let mut ret: u64;
//     unsafe {
//         asm!(
//             "mov rax, {:r}",
//             "syscall",
//             "mov {:r}, rax",
//             in(reg) syscall,
//             out(reg) ret
//         );
//     }
//     ret
// }

// fn execute_in_user() {
//     let mut ret_val = 0;
//     while ret_val == 0 {
//         ret_val = test_syscall(1234);
//     }
//     loop {}
// }
