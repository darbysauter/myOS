use super::Locked;
use crate::alloc::vec::Vec;
use crate::memory::heap::linked_list_alloc::LinkedListAllocator;
use crate::memory::page_table::PhysPage4KiB;
use crate::println;
use alloc::alloc::GlobalAlloc;
use alloc::alloc::Layout;
use core::mem;

#[derive(Debug)]
struct ListNode {
    next: Option<&'static mut ListNode>,
}

// powers of 2 up to 4096 (page size)
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

pub struct BlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: LinkedListAllocator,
    pub total_memory: u64,
}

impl BlockAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        BlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: LinkedListAllocator::new(),
            total_memory: 0,
        }
    }

    pub fn print_heap_stats(&mut self) {
        let total_used_bytes = self.fallback_allocator.used_memory;
        println!(
            "Heap Total Used(tracked): {:#x} Bytes | {:#} KiB",
            total_used_bytes,
            total_used_bytes / 1024
        );
        println!(
            "Heap Total Size(tracked): {:#x} Bytes | {:#} KiB",
            self.total_memory,
            self.total_memory / 1024
        );
    }

    pub fn heap_sanity_check(&mut self, free_bytes: u64) {
        let total_used_bytes = self.fallback_allocator.used_memory;
        let total_free_bytes = self.total_memory - total_used_bytes;
        if free_bytes != total_free_bytes {
            let dif = if total_free_bytes > free_bytes {
                total_free_bytes - free_bytes
            } else {
                free_bytes - total_free_bytes
            };
            println!(
                "tracked free bytes: {:#x} free according to allocators: {:#x} dif: {:#x}",
                total_free_bytes, free_bytes, dif
            );
            panic!("Heap disagreed");
        } else {
            println!("Heap Sanity Check Passed");
        }
    }

    pub fn print_block_regions(&mut self) -> u64 {
        let mut total_bytes = 0;
        for (idx, list) in self.list_heads.iter().enumerate() {
            let mut list = list;
            let mut num = 0;
            while let Some(l) = list {
                // println!("{:#x}", l);
                list = &l.next;
                num += 1;
            }
            let bytes = num * BLOCK_SIZES[idx];
            total_bytes += bytes;
            if bytes != 0 {
                println!("region size: {:#} bytes: {:#x}", BLOCK_SIZES[idx], bytes);
            }
        }
        total_bytes as u64
    }

    pub fn get_block_regions(&mut self) -> u64 {
        let mut total_bytes = 0;
        for (idx, list) in self.list_heads.iter().enumerate() {
            let mut list = list;
            let mut num = 0;
            while let Some(l) = list {
                list = &l.next;
                num += 1;
            }
            let bytes = num * BLOCK_SIZES[idx];
            total_bytes += bytes;
        }
        total_bytes as u64
    }

    pub fn print_ll_regions(&mut self) -> u64 {
        self.fallback_allocator.print_regions()
    }

    pub fn get_ll_regions(&mut self) -> u64 {
        self.fallback_allocator.get_regions()
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback_allocator.init(heap_start, heap_size);
        self.total_memory += heap_size as u64;
    }

    pub unsafe fn extend(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback_allocator.init(heap_start, heap_size);
        self.total_memory += heap_size as u64;
    }

    unsafe fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        self.fallback_allocator.alloc(layout)
    }

    // TODO: still need to do relocation here
    pub fn fix_heap_after_remap(&mut self, heap_regions: &Vec<(&PhysPage4KiB, usize)>) {
        for i in 0..self.list_heads.len() {
            if let Some(node) = self.list_heads[i].take() {
                let mut rev_list: Option<&'static mut ListNode> = None;
                let mut node = node;
                // println!("head: {:p} idx: {:#x}", node, i);
                while let Some(next) = node.next.take() {
                    // println!("node: {:p} idx: {:#x}", next, i);
                    node.next = rev_list;
                    rev_list = Some(node);
                    node = next;
                }
                node.next = rev_list;
                rev_list = Some(node);
                self.list_heads[i] = rev_list;
            }
        }
        self.fallback_allocator.fix_heap_after_remap(heap_regions);
    }
}

fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

// Think about how this handles non aligned allocations
unsafe impl GlobalAlloc for Locked<BlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                match allocator.list_heads[index].take() {
                    Some(node) => {
                        // println!("increased [block]: {:#x} cur: {:#x}", BLOCK_SIZES[index], allocator.fallback_allocator.used_memory);
                        allocator.fallback_allocator.used_memory += BLOCK_SIZES[index] as u64;
                        allocator.list_heads[index] = node.next.take();
                        node as *mut ListNode as *mut u8
                    }
                    None => {
                        // no block exists in list => allocate new block
                        let block_size = BLOCK_SIZES[index];
                        // only works if all block sizes are a power of 2
                        let block_align = block_size;
                        let layout = Layout::from_size_align(block_size, block_align).unwrap();
                        allocator.fallback_alloc(layout)
                    }
                }
            }
            None => allocator.fallback_alloc(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                // println!("decreased [block]: {:#x} cur: {:#x}", BLOCK_SIZES[index], allocator.fallback_allocator.used_memory);
                allocator.fallback_allocator.used_memory -= BLOCK_SIZES[index] as u64;
                let new_node = ListNode {
                    next: allocator.list_heads[index].take(),
                };
                // verify that block has size and alignment required for storing node
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
                let new_node_ptr = ptr as *mut ListNode;
                new_node_ptr.write(new_node);
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
            None => {
                allocator.fallback_allocator.dealloc(ptr, layout);
            }
        }
    }
}
