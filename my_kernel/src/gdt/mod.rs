use crate::println;

#[repr(C, packed)]
pub struct GlobalDescriptorTable {
    size: u16,
    addr: u64,
    null_seg: u64,
    code_seg: u64,
    data_seg: u64,
    end_seg: u64, // just used to get size of all segments
}

impl GlobalDescriptorTable {
    pub fn new() -> Self {
        let mut gdt = GlobalDescriptorTable {
            size: 0,
            addr: 0,
            null_seg: 0,
            code_seg: 0x00209A0000000000,
            data_seg: 0x0000920000000000,
            end_seg: 0,
        };
        gdt.size = (&gdt.end_seg as *const _ as usize - &gdt.null_seg as *const _ as usize) as u16;
        gdt
    }

    pub fn load(&mut self) {
        unsafe {
            self.addr = &self.null_seg as *const _ as u64;
        }

        let ptr = self as *const _ as usize;
        // loop{}
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
}