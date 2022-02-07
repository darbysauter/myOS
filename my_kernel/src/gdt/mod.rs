use alloc::boxed::Box;

#[repr(C, packed)]
pub struct GDT {
    size: u16,
    addr: u64,
    null_seg: u64,
    code_seg: u64,
    data_seg: u64,
    end_seg: u64, // just used to get size of all segments
}

impl GDT {
    #[allow(unaligned_references)]
    fn new() -> Self {
        let mut gdt = GDT {
            size: 0,
            addr: 0,
            null_seg: 0,
            kern_code_seg: 0x00209A0000000000,
            kern_data_seg: 0x0000920000000000,
            user_code_seg: 0x00209A0000000000,
            user_data_seg: 0x0000920000000000,
            end_seg: 0,
        };
        gdt.size = (&gdt.end_seg as *const _ as usize - &gdt.null_seg as *const _ as usize) as u16;
        gdt
    }

    #[allow(unaligned_references)]
    pub fn load(&mut self) {
        self.addr = &self.null_seg as *const _ as u64;

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
                "mov ss, ax"); // TODO: reload CS register
        }
    }

    pub fn create_gdt_on_heap() -> Box<GDT> {
        Box::new(GDT::new())
    }

    pub fn setup_gdt(gdt: &mut Box<GDT>) {
        gdt.load();
        gdt.reload_segments();
    }
}