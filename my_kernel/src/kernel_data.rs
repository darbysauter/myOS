use lazy_static::lazy_static;
use spin::Mutex;

use crate::tss::TSS;

const MAX_PROCESSES: usize = 0x100;

#[derive(Clone, Copy)]
struct ProcessInfo {
    _cr3: usize,
}

pub struct KernelData {
    _tss: Option<TSS>,
    _process_info: [Option<ProcessInfo>; MAX_PROCESSES],
}

lazy_static! {
    pub static ref KERNEL_DATA: Mutex<KernelData> = Mutex::new(KernelData {
        _tss: None,
        _process_info: [None; MAX_PROCESSES],
    });
}
