use crate::memory::stack::KERN_STACK_TOP;
use alloc::boxed::Box;
use core::mem;

#[repr(C, packed)]
pub struct TSS {
    res0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    res1: u32,
    res2: u32,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    res3: u32,
    res4: u32,
    res5: u16,
    iomap_off: u16,
}

impl TSS {
    fn new() -> Self {
        TSS {
            res0: 0,
            rsp0: KERN_STACK_TOP as u64,
            rsp1: 0,
            rsp2: 0,
            res1: 0,
            res2: 0,
            ist1: 0,
            ist2: 0,
            ist3: 0,
            ist4: 0,
            ist5: 0,
            ist6: 0,
            ist7: 0,
            res3: 0,
            res4: 0,
            res5: 0,
            iomap_off: 0,
        }
    }

    pub fn create_tss_on_heap() -> Box<TSS> {
        Box::new(TSS::new())
    }

    pub fn create_gdt_entry(tss: &Box<TSS>) -> (u64, u64) {
        let base: u64 = core::ptr::addr_of!(tss.res0) as usize as u64;
        let limit: u64 = mem::size_of::<TSS>() as u64;

        // mask key: b -> base, l -> limit
        // bits are in order
        //                      bbxlxxbbbbbbllll
        let base_value: u64 = 0x0000890000000000;

        let base_hi: u64 = (0xff000000 & base) << 32;
        let base_lo: u64 = (0x00ffffff & base) << 16;
        let limit_hi: u64 = (0x000f0000 & limit) << 32;
        let limit_lo: u64 = 0x0000ffff & limit;

        let final_value_lo = base_value | base_hi | base_lo | limit_hi | limit_lo;

        let final_value_hi: u64 = (0xffffffff00000000 & base) >> 32;

        (final_value_hi, final_value_lo)
    }
}
