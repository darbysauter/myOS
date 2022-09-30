use core::arch::asm;

#[repr(u64)]
enum Syscalls {
    Print = 0,
}

unsafe extern "C" fn syscall_0(syscall: Syscalls) -> u64 {
    let mut ret: u64;
    unsafe {
        asm!(
            "mov rax, {:r}",
            "syscall",
            "mov {:r}, rax",
            in(reg) syscall as u64,
            out(reg) ret
        );
    }
    ret
}

pub fn print() -> u64 {
    unsafe { syscall_0(Syscalls::Print) }
}
