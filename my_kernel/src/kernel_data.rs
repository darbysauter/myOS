use lazy_static::lazy_static;
use spin::Mutex;

use crate::tss::TSS;

const MAX_PROCESSES: usize = 0x100;

#[derive(Clone, Copy)]
struct ProcessInfo {
    cr3: usize,
}

pub struct KernelData {
    tss: Option<TSS>,
    process_info: [Option<ProcessInfo>; MAX_PROCESSES],
}

lazy_static! {
    pub static ref KERNEL_DATA: Mutex<KernelData> = Mutex::new(KernelData {
        tss: None,
        process_info: [None; MAX_PROCESSES],
    });
}
