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
        loop {}
    }
    ret

    /*
    RAX=000000000000001b RBX=0000000068747541 RCX=ffffe00000000000 RDX=0000000000000023
    RSI=ffff80000001ce50 RDI=00000000000004d2 RBP=0000000000000007 RSP=ffffdfffffffffe8
    R8 =0000000000000001 R9 =ffffefffffffff90 R10=ffff8000000045c0 R11=0000000000000001
    R12=0000000000000000 R13=0000000000000000 R14=0000000000000000 R15=0000000000000000
    RIP=ffff80000001ce45 RFL=00000086 [--S--P-] CPL=3 II=0 A20=1 SMM=0 HLT=0
    ES =0000 0000000000000000 00000000 00001300
    CS =0023 0000000000000000 00000000 0020fa00 DPL=3 CS64 [-R-]
    SS =001b 0000000000000000 00000000 0000f200 DPL=3 DS   [-W-]
    DS =0000 0000000000000000 00000000 00001300
    FS =0000 0000000000000000 00000000 00001300
    GS =0000 0000000000000000 00000000 00001300
    LDT=0000 0000000000000000 0000ffff 00008200 DPL=0 LDT
    TR =0028 ffffa00000014080 00000068 00008900 DPL=0 TSS64-avl
    GDT=     ffffa0000001420a 00000038
    IDT=     ffffa00000014290 00000fff
    CR0=80000011 CR2=0000000000000000 CR3=0000000000240000 CR4=00000020
    DR0=0000000000000000 DR1=0000000000000000 DR2=0000000000000000 DR3=0000000000000000
    DR6=00000000ffff0ff0 DR7=0000000000000400
    EFER=0000000000000501

    AFTER

    RAX=11223344aabbccdd RBX=0000000068747541 RCX=ffff80000001ce4e RDX=0000000000000001
    RSI=0000000000000000 RDI=ffff80000003e510 RBP=0000000000000007 RSP=ffffdffffffffe30
    R8 =ffffdffffffff6f8 R9 =0000000000000004 R10=ffffdfffffffff98 R11=0000000000000096
    R12=0000000000000000 R13=0000000000000000 R14=0000000000000000 R15=0000000000000000
    RIP=ffff80000001ce56 RFL=00000096 [--S-AP-] CPL=3 II=0 A20=1 SMM=0 HLT=0
    ES =0000 0000000000000000 00000000 00001300
    CS =0023 0000000000000000 ffffffff 00a0fb00 DPL=3 CS64 [-RA]
    SS =001b 0000000000000000 ffffffff 00c0f300 DPL=3 DS   [-WA]
    DS =0000 0000000000000000 00000000 00001300
    FS =0000 0000000000000000 00000000 00001300
    GS =0000 0000000000000000 00000000 00001300
    LDT=0000 0000000000000000 0000ffff 00008200 DPL=0 LDT
    TR =0028 ffffa00000014080 00000068 00008900 DPL=0 TSS64-avl
    GDT=     ffffa0000001420a 00000038
    IDT=     ffffa00000014290 00000fff
    CR0=80000011 CR2=0000000000000000 CR3=0000000000240000 CR4=00000020
    DR0=0000000000000000 DR1=0000000000000000 DR2=0000000000000000 DR3=0000000000000000
    DR6=00000000ffff0ff0 DR7=0000000000000400
    EFER=0000000000000501
    */
}

fn execute_in_user() {
    let mut ret_val = 0;
    while ret_val == 0 {
        ret_val = test_syscall(1234);
    }
    loop {}
}
