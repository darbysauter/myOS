use crate::alloc::vec::Vec;
use crate::memory::heap::translate_usize_to_virt;
use crate::memory::page_table::PhysPage4KiB;
use crate::{print, println};
use alloc::alloc::Layout;
use core::mem;
use core::ptr;

#[derive(Debug)]
struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

pub struct LinkedListAllocator {
    head: ListNode,
    pub used_memory: u64,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        LinkedListAllocator {
            head: ListNode::new(0),
            used_memory: 0,
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr)
    }

    fn find_region(
        &mut self,
        size: usize,
        align: usize,
    ) -> Option<(&'static mut ListNode, usize, Option<(usize, usize)>)> {
        let mut current = &mut self.head;
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                let next = region.next.take();
                let mut beginning_excess: Option<(usize, usize)> = None;
                if region.start_addr() < alloc_start {
                    beginning_excess =
                        Some((region.start_addr(), alloc_start - region.start_addr()));
                }
                let ret = Some((current.next.take().unwrap(), alloc_start, beginning_excess));
                current.next = next;
                return ret;
            } else {
                current = current.next.as_mut().unwrap();
            }
        }
        None
    }

    pub fn print_regions(&mut self) -> u64 {
        let mut current = &mut self.head;
        let mut total_bytes: usize = 0;
        print!("Linked List Alloc Regions: [");
        while let Some(ref mut region) = current.next {
            print!("{:#x}, ", region.size);
            total_bytes += region.size;
            current = current.next.as_mut().unwrap();
        }
        println!("]");
        println!("Linked List Alloc Total Size: {:#x}", total_bytes);
        total_bytes as u64
    }

    pub fn get_regions(&mut self) -> u64 {
        let mut current = &mut self.head;
        let mut total_bytes: usize = 0;
        while let Some(ref mut region) = current.next {
            total_bytes += region.size;
            current = current.next.as_mut().unwrap();
        }
        total_bytes as u64
    }

    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }

    pub unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let (size, align) = LinkedListAllocator::size_align(layout);

        if let Some((region, alloc_start, excess)) = self.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            // println!("increased [LL]: {:#x} cur: {:#x}", alloc_end - alloc_start, self.used_memory);
            self.used_memory += alloc_end as u64 - alloc_start as u64;
            if excess_size > 0 {
                self.add_free_region(alloc_end, excess_size);
            }
            if let Some((start, size)) = excess {
                self.add_free_region(start, size);
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let (size, _) = LinkedListAllocator::size_align(layout);

        // println!("decreased [LL]: {:#x} cur: {:#x}", size, self.used_memory);
        self.used_memory -= size as u64;
        self.add_free_region(ptr as usize, size)
    }

    pub fn fix_heap_after_remap(&mut self, heap_regions: &Vec<(&PhysPage4KiB, usize)>) {
        let mut current = &mut self.head;
        while let Some(ref mut region) = current.next {
            unsafe {
                let x = region as *mut _ as usize as *mut usize;
                if *x & 0xFFFF_0000_0000_0000 == 0 {
                    // needs to be translated
                    // println!("ADDR2: {:#x}", *x);
                    *x = translate_usize_to_virt(heap_regions, *x);
                    // println!("ADDR2: {:#x}", *x);
                }
            }
            current = current.next.as_mut().unwrap();
        }
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
