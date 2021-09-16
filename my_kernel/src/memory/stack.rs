use crate::memory::frame_allocator::LinkedListFrameAllocator;
use crate::memory::page_table::{ PhysPage4KiB, PML4 };

pub const STACK_SIZE: usize = 20 * 1024; // 20 KiB, this should always be a multiple of 4KiB
pub const KERN_STACK_TOP: usize = 0xFFFF_F000_0000_0000;

pub fn create_new_stack_and_map(frame_alloc: &mut LinkedListFrameAllocator, pml4: &mut PML4) -> &'static PhysPage4KiB {
    let kern_top_page = KERN_STACK_TOP & 0xfffffffffffff000;
    let kern_bot_page = (KERN_STACK_TOP - STACK_SIZE) & 0xfffffffffffff000;
    
    let mut last_page: usize = 0x0;
    let mut first_page: usize = 0x0;
    for vpage in (kern_bot_page .. kern_top_page).step_by(0x1000) {
        if let Some(new_page) = frame_alloc.allocate() {
            let new_page = new_page as *const PhysPage4KiB as usize;
            if last_page != 0 && last_page + 0x1000 != new_page {
                panic!("pages were not contiguous | last: {:#x} cur: {:#x}", last_page, new_page);
            }
            if first_page == 0 {
                first_page = new_page;
            }
            last_page = new_page;

            unsafe {
                pml4.map_frame_4k(new_page, vpage, true, false, None);
            }
        } else {
            panic!("Out of Pages");
        }
    }
    unsafe {
        return &*(last_page as *const PhysPage4KiB)
    }
}