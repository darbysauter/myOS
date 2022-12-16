use alloc::boxed::Box;
use core::arch::asm;

#[repr(C, packed)]
pub struct GDT {
    size: u16,
    addr: u64,
    null_seg: u64,
    kern_code_seg: u64,
    kern_data_seg: u64,
    user_data_seg: u64,
    user_code_seg: u64,
    tss_lo: u64,
    tss_hi: u64,
    end_seg: u64, // just used to get size of all segments
}

pub const KERN_CODE_SEL: u64 = 1 << 3;
pub const KERN_DATA_SEL: u64 = 2 << 3;
pub const USER_CODE_SEL: u64 = 4 << 3 | 3;
pub const USER_DATA_SEL: u64 = 3 << 3 | 3;
pub const TSS_SEL: u64 = 5;

impl GDT {
    #[allow(unaligned_references)]
    fn new(tss_hi: u64, tss_lo: u64) -> Self {
        let mut gdt = GDT {
            size: 0,
            addr: 0,
            null_seg: 0,
            kern_code_seg: 0x00209A0000000000,
            kern_data_seg: 0x0000920000000000,
            user_data_seg: 0x0000F20000000000,
            user_code_seg: 0x0020FA0000000000,
            tss_lo: tss_lo,
            tss_hi: tss_hi,
            end_seg: 0,
        };
        gdt.size = (core::ptr::addr_of!(gdt.end_seg) as usize
            - core::ptr::addr_of!(gdt.null_seg) as usize) as u16;
        gdt
    }

    #[allow(unaligned_references)]
    pub fn load(&mut self) {
        self.addr = core::ptr::addr_of!(self.null_seg) as u64;

        let ptr = self as *const _ as usize;
        if ptr % 0x8 != 0 {
            panic!("GDT pointer not aligned");
        }
        unsafe {
            asm!( "lgdt [{}]", in(reg) ptr);
        }
    }

    pub fn reload_segments(&mut self) {
        unsafe {
            asm!(
                "mov ax, 0x10",
                "mov ds, ax",
                "mov es, ax",
                "mov fs, ax",
                "mov gs, ax",
                "mov ss, ax"
            ); // TODO: reload CS register
        }
    }

    pub fn load_tss(&mut self) {
        unsafe {
            asm!(
                "mov ax, (5 * 8)", // selectoor 5 for tss
                "ltr ax"
            );
        }
    }

    pub fn create_gdt_on_heap(tss_hi: u64, tss_lo: u64) -> Box<GDT> {
        Box::new(GDT::new(tss_hi, tss_lo))
    }

    pub fn setup_gdt(gdt: &mut Box<GDT>) {
        gdt.load();
        gdt.reload_segments();
        gdt.load_tss();
    }
}
