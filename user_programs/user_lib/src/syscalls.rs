use core::arch::asm;

#[repr(u64)]
enum Syscall {
    Print = 0,
    CreateProc = 1,
}

unsafe extern "C" fn syscall_0(syscall: Syscall) -> u64 {
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
    unsafe { syscall_0(Syscall::Print) }
}

pub fn create_proc() -> u64 {
    unsafe { syscall_0(Syscall::CreateProc) }
}
