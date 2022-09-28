use core::arch::asm;

use crate::cpu::{get_cpuid_feature_rdx, read_msr, write_msr};
use crate::interrupts::ExtraInterrupts;
use crate::memory::page_table::PML4;

const APIC_FEATURE_BIT: u16 = 9;

const IA32_APIC_BASE_MSR: u32 = 0x1B;

pub fn check_apic() -> bool {
    get_cpuid_feature_rdx(APIC_FEATURE_BIT)
}

pub fn get_apic_base() -> u64 {
    unsafe { read_msr(IA32_APIC_BASE_MSR) as u64 & 0xffff_ffff_ffff_f000 }
}

pub fn set_apic_base(base: u64) {
    unsafe { write_msr(IA32_APIC_BASE_MSR, base) }
}

pub unsafe fn enable_apic(base: u64) {
    *((base + 0xf0) as *mut u32) |= 0x100;
}

pub unsafe fn set_apic_tpr(base: u64, val: u32) {
    *((base + 0x80) as *mut u32) = val;
}

pub unsafe fn apic_timer_int_index(base: u64, index: ExtraInterrupts) {
    *((base + 0x320) as *mut u32) = 0x20000 | index as u32;
}

pub unsafe fn apic_timer_set_divide(base: u64, divide: u32) {
    *((base + 0x3e0) as *mut u32) = divide;
}

pub unsafe fn apic_timer_set_count(base: u64, count: u32) {
    *((base + 0x380) as *mut u32) = count;
}

pub unsafe fn apic_end_of_interrupt(base: u64) {
    *((base + 0xb0) as *mut u32) = 0;
}

pub fn ident_map_apic_page(base: u64, pml4: &mut PML4) {
    assert_eq!(base & 0xffff_ffff_ffff_f000, base);
    unsafe {
        pml4.map_frame_4k(base as usize, base as usize, true, true, None);
    }
}

pub fn disable_pic() {
    unsafe {
        asm!(
            "  mov al, 0xff;
                out 0xa1, al;
                out 0x21, al;"
        );
    }
}

pub fn start_apic_timer(base: u64) {
    unsafe {
        apic_timer_set_divide(base, 0b1011);
        apic_timer_int_index(base, ExtraInterrupts::ApicTimer);
        apic_timer_set_count(base, 10000000);

        apic_end_of_interrupt(base);
    }
}
