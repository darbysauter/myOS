use core::arch::asm;

pub fn get_cpuid_feature_rdx(bit: u16) -> bool {
    let mut features: u64;
    unsafe {
        asm!( "mov rax, 0; cpuid; mov {}, rdx", out(reg) features);
    }
    features & (1 << bit) != 0
}

const MSR_FEATURE_BIT: u16 = 5;

pub fn check_msr() -> bool {
    get_cpuid_feature_rdx(MSR_FEATURE_BIT)
}

pub unsafe fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") low, in("edx") high);
}

pub unsafe fn read_msr(msr: u32) -> u64 {
    let mut low: u32;
    let mut high: u32;
    asm!("mov ecx, {:e}; rdmsr; mov {:e}, eax; mov {:e}, edx", in(reg) msr, out(reg) low, out(reg) high);

    ((high as u64) << 32) | (low as u64)
}
