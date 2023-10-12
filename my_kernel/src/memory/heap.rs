use crate::memory::frame_allocator::LinkedListFrameAllocator;
use crate::memory::heap::block_alloc::BlockAllocator;
use crate::memory::page_table::PhysPage4KiB;
use crate::println;
use alloc::boxed::Box;
use alloc::vec::Vec;

pub mod block_alloc;
pub mod linked_list_alloc;

pub const HEAP_START: usize = 0xFFFF_A000_0000_0000;
pub const HEAP_SIZE: usize = 8192 * 1024; // 8192 KiB, this should always be a multiple of 4KiB

#[global_allocator]
static ALLOCATOR: Locked<BlockAllocator> = Locked::new(BlockAllocator::new());

// Called while identity mapped
pub fn init_heap_phase1(
    frame_alloc: &mut LinkedListFrameAllocator,
) -> (&'static PhysPage4KiB, usize) {
    if HEAP_START % 0x1000 != 0 || HEAP_SIZE % 0x1000 != 0 {
        panic!("HEAP not 4KiB aligned");
    }

    let mut last_page: usize = 0x0;
    let mut first_page: usize = 0x0;
    let mut pages: usize = 0;
    for _page in (HEAP_START..HEAP_START + HEAP_SIZE).step_by(0x1000) {
        if let Some(new_page) = frame_alloc.allocate() {
            let new_page = new_page as *const PhysPage4KiB as usize;
            if last_page != 0 && last_page + 0x1000 != new_page {
                unsafe {
                    frame_alloc.deallocate(&mut *(new_page as *mut PhysPage4KiB));
                    ALLOCATOR.lock().init(first_page, pages * 0x1000);
                    println!(
                        "Partially initialized heap with size {} KiB",
                        (pages * 0x1000) / 1024
                    );
                    return (&*(first_page as *const PhysPage4KiB), pages);
                }
            }
            pages += 1;
            if first_page == 0 {
                first_page = new_page;
            }
            last_page = new_page;
        } else {
            panic!("Out of Pages");
        }
    }
    unsafe {
        ALLOCATOR.lock().init(first_page, HEAP_SIZE);
        println!(
            "Fully initialized heap with size {} KiB",
            (pages * 0x1000) / 1024
        );
        (&*(first_page as *const PhysPage4KiB), pages)
    }
}

pub fn init_heap_phase2(
    frame_alloc: &mut LinkedListFrameAllocator,
    pages_used: usize,
) -> Vec<(&'static PhysPage4KiB, usize)> {
    let increase_size = HEAP_SIZE - (pages_used * 0x1000);
    let new_start = HEAP_START + (pages_used * 0x1000);
    let mut phys_regions = Vec::new();
    if increase_size == 0 {
        return phys_regions;
    }
    if HEAP_START % 0x1000 != 0 || HEAP_SIZE % 0x1000 != 0 {
        panic!("HEAP not 4KiB aligned");
    }

    let mut last_page: usize = 0x0;
    let mut first_page: usize = 0x0;
    let mut pages: usize = 0;
    for _page in (new_start..HEAP_START + HEAP_SIZE).step_by(0x1000) {
        if let Some(new_page) = frame_alloc.allocate() {
            let new_page = new_page as *const PhysPage4KiB as usize;
            if last_page != 0 && last_page + 0x1000 != new_page {
                unsafe {
                    ALLOCATOR.lock().extend(first_page, increase_size);
                    phys_regions.push((&*(first_page as *const PhysPage4KiB), pages));
                    println!("Extended heap with size {} KiB", (pages * 0x1000) / 1024);
                }
                first_page = new_page;
                pages = 0;
            }
            pages += 1;
            if first_page == 0 {
                first_page = new_page;
            }
            last_page = new_page;
        } else {
            panic!("Out of Pages");
        }
    }
    unsafe {
        ALLOCATOR.lock().extend(first_page, increase_size);
        phys_regions.push((&*(first_page as *const PhysPage4KiB), pages));
        println!("Extended heap with size {} KiB", (pages * 0x1000) / 1024);
        phys_regions
    }
}

/// # Safety
/// This should only be called for when `object` is a reference to virtual mem and we want to translate it to physical
pub unsafe fn translate_mut_ref_to_phys<'a, T>(
    heap_regions: &Vec<(&PhysPage4KiB, usize)>,
    object: &'a mut T,
) -> &'a mut T {
    let o = object as *const T as usize;
    // println!("orig addr: {:#x}", o);
    let mut offset: usize = (o - HEAP_START) & 0xffff_ffff_ffff_f000;
    for (start_page, num_pages) in heap_regions {
        let offset_in_pages = offset / 0x1000;
        if offset_in_pages > *num_pages {
            offset -= num_pages * 0x1000;
            continue;
        }
        // should be in this region
        let start_page = (*start_page) as *const PhysPage4KiB as usize;
        let phys_page = start_page + offset;
        let phys_addr = phys_page + (o & 0xfff);
        // println!("new addr: {:#x}", phys_addr);
        let phys_ref = &mut (*(phys_addr as *mut T));
        return phys_ref;
    }
    panic!("Did not find region");
}

/// # Safety
/// This should only be called for when `object` is a reference to virtual mem and we want to translate it to physical
pub unsafe fn translate_ref_to_phys<'a, T>(
    heap_regions: &Vec<(&PhysPage4KiB, usize)>,
    object: &'a T,
) -> &'a T {
    let o = object as *const T as usize;
    // println!("orig addr: {:#x}", o);
    let mut offset: usize = (o - HEAP_START) & 0xffff_ffff_ffff_f000;
    for (start_page, num_pages) in heap_regions {
        let offset_in_pages = offset / 0x1000;
        if offset_in_pages > *num_pages {
            offset -= num_pages * 0x1000;
            continue;
        }
        // should be in this region
        let start_page = (*start_page) as *const PhysPage4KiB as usize;
        let phys_page = start_page + offset;
        let phys_addr = phys_page + (o & 0xfff);
        // println!("new addr: {:#x}", phys_addr);
        let phys_ref = &mut (*(phys_addr as *mut T));
        return phys_ref;
    }
    panic!("Did not find region");
}

/// # Safety
/// This should only be called for when `object` is a reference to virtual mem and we want to translate it to physical
pub unsafe fn translate_usize_to_phys(
    heap_regions: &Vec<(&PhysPage4KiB, usize)>,
    object: usize,
) -> usize {
    let o = object;
    // println!("orig addr: {:#x}", o);
    let mut offset: usize = (o - HEAP_START) & 0xffff_ffff_ffff_f000;
    for (start_page, num_pages) in heap_regions {
        let offset_in_pages = offset / 0x1000;
        if offset_in_pages > *num_pages {
            offset -= num_pages * 0x1000;
            continue;
        }
        // should be in this region
        let start_page = (*start_page) as *const PhysPage4KiB as usize;
        let phys_page = start_page + offset;
        let phys_addr = phys_page + (o & 0xfff);
        // println!("new addr: {:#x}", phys_addr);
        let phys_ref = phys_addr;
        return phys_ref;
    }
    panic!("Did not find region");
}

/// # Safety
/// This should only be called for when `object` is a reference to physical mem and we want to translate it to virtual
pub unsafe fn translate_ref_to_virt<'a, T>(
    heap_regions: &Vec<(&PhysPage4KiB, usize)>,
    object: &'a mut T,
) -> &'a mut T {
    let mut o = object as *const T as usize;
    // println!("orig addr: {:#x}", o);
    let mut offset: usize = 0;
    for (start_page, num_pages) in heap_regions {
        let start_page = *start_page as *const PhysPage4KiB as usize;
        let end_page = start_page + 0x1000 * ((*num_pages) - 1);
        let o_page = o & 0xffff_ffff_ffff_f000;
        if o_page >= start_page && o_page <= end_page {
            // in this region
            offset += o_page - start_page;
            break;
        }
        offset += num_pages * 0x1000;
    }
    o = HEAP_START + offset + (o & 0xfff);

    // println!("new addr: {:#x}", o);
    &mut (*(o as *mut T))
}

/// # Safety
/// This should only be called for when `object` is a reference to physical mem and we want to translate it to virtual
pub unsafe fn translate_usize_to_virt(
    heap_regions: &Vec<(&PhysPage4KiB, usize)>,
    object: usize,
) -> usize {
    let mut o = object;
    // println!("orig addr: {:#x}", o);
    let mut offset: usize = 0;
    for (start_page, num_pages) in heap_regions {
        let start_page = *start_page as *const PhysPage4KiB as usize;
        let end_page = start_page + 0x1000 * ((*num_pages) - 1);
        let o_page = o & 0xffff_ffff_ffff_f000;
        if o_page >= start_page && o_page <= end_page {
            // in this region
            offset += o_page - start_page;
            break;
        }
        offset += num_pages * 0x1000;
    }
    o = HEAP_START + offset + (o & 0xfff);

    // println!("new addr: {:#x}", o);
    o
}

/// # Safety
/// This should only be called for when Box is a reference to physical mem and we want to translate it to virtual
pub unsafe fn translate_box<T>(
    heap_regions: &Vec<(&'static PhysPage4KiB, usize)>,
    object: Box<T>,
) -> *mut T {
    let mut o = Box::<T>::into_raw(object) as *mut Box<T> as *mut T as usize;
    // println!("orig addr: {:#x}", o);
    let mut offset: usize = 0;
    for (start_page, num_pages) in heap_regions {
        let start_page = *start_page as *const PhysPage4KiB as usize;
        let end_page = start_page + 0x1000 * ((*num_pages) - 1);
        let o_page = o & 0xffff_ffff_ffff_f000;
        if o_page >= start_page && o_page <= end_page {
            // in this region
            offset += o_page - start_page;
            break;
        }
        offset += num_pages * 0x1000;
    }
    o = HEAP_START + offset + (o & 0xfff);

    // println!("new addr: {:#x}", o);
    o as *mut T
}

/// # Safety
/// This should only be called for when `vec` is a reference to physical mem and we want to translate it to virtual
pub unsafe fn translate_box_vec<T>(
    heap_regions: &Vec<(&'static PhysPage4KiB, usize)>,
    vec: Vec<T>,
) -> *mut Vec<T> {
    let (arr_ptr, len, cap) = vec.into_raw_parts();
    let mut arr_ptr = arr_ptr as usize;

    let mut offset: usize = 0;
    for (start_page, num_pages) in heap_regions {
        let start_page = *start_page as *const PhysPage4KiB as usize;
        let end_page = start_page + 0x1000 * ((*num_pages) - 1);
        let vec_page = arr_ptr & 0xffff_ffff_ffff_f000;
        if vec_page >= start_page && vec_page <= end_page {
            // in this region
            offset += vec_page - start_page;
            break;
        }
        offset += num_pages * 0x1000;
    }
    arr_ptr = HEAP_START + offset + (arr_ptr & 0xfff);

    let rebuilt = Vec::from_raw_parts(arr_ptr as *mut T, len, cap);

    let object = Box::new(rebuilt); // puts vec obj on heap

    let mut o = Box::<Vec<T>>::into_raw(object) as *mut Box<Vec<T>> as *mut Vec<T> as usize;

    // println!("o: {:#x} ptr: {:#x}", o, ptr);

    let mut offset: usize = 0;
    for (start_page, num_pages) in heap_regions {
        let start_page = *start_page as *const PhysPage4KiB as usize;
        let end_page = start_page + 0x1000 * ((*num_pages) - 1);
        let o_page = o & 0xffff_ffff_ffff_f000;
        if o_page >= start_page && o_page <= end_page {
            // in this region
            offset += o_page - start_page;
            break;
        }
        offset += num_pages * 0x1000;
    }
    o = HEAP_START + offset + (o & 0xfff);

    o as *mut Vec<T>
}

pub fn fix_heap_after_remap(heap_regions: &Vec<(&PhysPage4KiB, usize)>) {
    ALLOCATOR.lock().fix_heap_after_remap(heap_regions);
}

pub fn print_heap() {
    let ll_alloc_size = ALLOCATOR.lock().print_ll_regions();
    let block_alloc_size = ALLOCATOR.lock().print_block_regions();
    let total_bytes = ll_alloc_size + block_alloc_size;
    println!(
        "Heap Total Free: {:#x} Bytes | {:#} KiB",
        total_bytes,
        total_bytes / 1024
    );
    ALLOCATOR.lock().print_heap_stats();
}

pub fn heap_sanity_check() {
    let ll_alloc_size = ALLOCATOR.lock().get_ll_regions();
    let block_alloc_size = ALLOCATOR.lock().get_block_regions();
    let total_bytes = ll_alloc_size + block_alloc_size;
    ALLOCATOR.lock().heap_sanity_check(total_bytes);
}

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}
