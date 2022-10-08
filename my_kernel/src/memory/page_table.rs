use alloc::alloc::{Global, Layout};
use core::alloc::Allocator;

use crate::memory::heap::{translate_mut_ref_to_phys, translate_ref_to_virt, HEAP_START};
use crate::println;
use alloc::vec::Vec;
use core::arch::asm;
use core::mem;
use core::ptr::NonNull;

// Assumes IA-32e Paging and CR4.PCIDE = 0
// No support for 1GiB Pages

const PAGE_TABLE_SIZE: usize = 512;
const RECUR_INDEX: usize = 0x1ff;

#[derive(Debug)]
#[repr(align(4096))]
#[repr(C)]
pub struct PML4 {
    // Page Map Level 4 no support for flags now
    pub entries: [PML4E; PAGE_TABLE_SIZE],
}

impl PML4 {
    pub unsafe fn new(
        heap_regions: Option<&Vec<(&'static PhysPage4KiB, usize)>>,
    ) -> &'static mut Self {
        // TODO: Why did i do static ref???
        let ptr = Global.allocate_zeroed(Layout::new::<PML4>());

        if ptr.is_err() {
            panic!("Alloc Error");
        }

        let pml4 = &mut *(ptr.unwrap().as_mut_ptr() as *mut PML4);
        let mut pml4_recur = &mut *(ptr.unwrap().as_mut_ptr() as *mut PDPT);
        if let Some(heap_regions) = heap_regions {
            pml4_recur = translate_mut_ref_to_phys(heap_regions, pml4_recur);
        }
        pml4.add(RECUR_INDEX, pml4_recur, true, true);

        pml4
    }

    pub unsafe fn add(&mut self, index: usize, pdpt: &PDPT, writable: bool, user_accessable: bool) {
        let phys_ptr = pdpt as *const PDPT as usize;
        if phys_ptr % 0x1000 != 0 || phys_ptr & 0xfff0000000000000 != 0 {
            panic!("PDPT not aligned");
        }
        if index >= self.entries.len() {
            panic!("index out of range");
        }

        let mut data = phys_ptr & 0xffffffffff000;
        if writable {
            data |= 0b10;
        }
        if user_accessable {
            data |= 0b100;
        }
        data |= 0b1; // present
        self.entries[index].data = data as u64;
    }

    pub unsafe fn map_frame_4k(
        &mut self,
        paddr: usize,
        vaddr: usize,
        writable: bool,
        user_accessable: bool,
        heap_regions: Option<&Vec<(&'static PhysPage4KiB, usize)>>,
    ) {
        if paddr % 0x1000 != 0 {
            panic!("paddr not aligned");
        }
        if vaddr % 0x1000 != 0 {
            panic!("vaddr not aligned");
        }
        let (pml4_ind, pdpt_ind, pd_ind, pt_ind) = indicies_of_vaddr(vaddr);

        let pml4e = &self.entries[pml4_ind];
        let pdpt = if let Some(phys_pdpt) = pml4e.pdpt() {
            if let Some(heap_regions) = heap_regions {
                translate_ref_to_virt(heap_regions, phys_pdpt)
            } else {
                phys_pdpt
            }
        } else {
            let virt_pdpt = PDPT::new();
            let phys_pdpt = if let Some(heap_regions) = heap_regions {
                translate_mut_ref_to_phys(heap_regions, virt_pdpt)
            } else {
                &(*virt_pdpt)
            };
            self.add(pml4_ind, phys_pdpt, writable, true);
            virt_pdpt
        };
        let pdpte = &pdpt.entries[pdpt_ind];

        let pd = if let Some(phys_pd) = pdpte.pd() {
            if let Some(heap_regions) = heap_regions {
                translate_ref_to_virt(heap_regions, phys_pd)
            } else {
                phys_pd
            }
        } else {
            let virt_pd = PD::new();
            let phys_pd = if let Some(heap_regions) = heap_regions {
                translate_mut_ref_to_phys(heap_regions, virt_pd)
            } else {
                &(*virt_pd)
            };
            pdpt.add(pdpt_ind, phys_pd, writable, true);
            virt_pd
        };
        let pde = &pd.entries[pd_ind];

        let pt = if let Some(phys_pt) = pde.pt() {
            if let Some(heap_regions) = heap_regions {
                translate_ref_to_virt(heap_regions, phys_pt)
            } else {
                phys_pt
            }
        } else {
            let virt_pt = PT::new();
            let phys_pt = if let Some(heap_regions) = heap_regions {
                translate_mut_ref_to_phys(heap_regions, virt_pt)
            } else {
                &(*virt_pt)
            };
            pd.add(pd_ind, phys_pt, writable, true);
            virt_pt
        };
        let pte = &pt.entries[pt_ind];

        if pte.present() {
            panic!("pte already maps a frame - vaddr: {:#x}", vaddr);
        } else {
            pt.add(pt_ind, paddr, writable, user_accessable);
        }
    }

    pub unsafe fn unmap_frame_4k(
        &mut self,
        vaddr: &VirtPage4KiB,
        heap_regions: Option<&Vec<(&PhysPage4KiB, usize)>>,
    ) -> &'static PhysPage4KiB {
        let vaddr = vaddr as *const VirtPage4KiB as usize;
        if vaddr % 0x1000 != 0 {
            panic!("vaddr not aligned");
        }
        let (pml4_ind, pdpt_ind, pd_ind, pt_ind) = indicies_of_vaddr(vaddr);

        let pml4e = &mut self.entries[pml4_ind];
        let pdpt = if let Some(phys_pdpt) = pml4e.pdpt() {
            if let Some(heap_regions) = heap_regions {
                translate_ref_to_virt(heap_regions, phys_pdpt)
            } else {
                phys_pdpt
            }
        } else {
            panic!("No pdpt for this vaddr");
        };
        let pdpte = &mut pdpt.entries[pdpt_ind];

        let pd = if let Some(phys_pd) = pdpte.pd() {
            if let Some(heap_regions) = heap_regions {
                translate_ref_to_virt(heap_regions, phys_pd)
            } else {
                phys_pd
            }
        } else {
            panic!("No pd for this vaddr");
        };
        let pde = &mut pd.entries[pd_ind];

        let pt = if let Some(phys_pt) = pde.pt() {
            if let Some(heap_regions) = heap_regions {
                translate_ref_to_virt(heap_regions, phys_pt)
            } else {
                phys_pt
            }
        } else {
            panic!("No pt for this vaddr");
        };
        let pte = &mut pt.entries[pt_ind];

        let frame = if let Some(frame) = pte.page() {
            frame
        } else {
            panic!("No frame for this vaddr");
        };

        pte.clear();

        // check if we should remove a pt
        for pte in pt.entries.iter() {
            if pte.present() {
                return &(*(frame as *const PhysPage4KiB));
            }
        }

        pde.clear();
        if let Some(ptr) = NonNull::new(pt as *mut PT as usize as *mut u8) {
            Global.deallocate(ptr, Layout::new::<PT>());
        }

        // check if we shoould remove a pd
        for pde in pd.entries.iter() {
            if pde.present() {
                return &(*(frame as *const PhysPage4KiB));
            }
        }

        pdpte.clear();
        if let Some(ptr) = NonNull::new(pd as *mut PD as usize as *mut u8) {
            Global.deallocate(ptr, Layout::new::<PD>());
        }

        // check if we shoould remove a pdpt
        for pdpte in pdpt.entries.iter() {
            if pdpte.present() {
                return &(*(frame as *const PhysPage4KiB));
            }
        }

        pml4e.clear();
        if let Some(ptr) = NonNull::new(pdpt as *mut PDPT as usize as *mut u8) {
            Global.deallocate(ptr, Layout::new::<PDPT>());
        }

        &(*(frame as *const PhysPage4KiB))
    }

    pub fn get_pdpt_recursive(
        &self,
        index: usize,
        heap_phys_regions: &Vec<(&PhysPage4KiB, usize)>,
    ) -> &'static mut PDPT {
        let phys_addr_ptr = indicies_to_vaddr(0x1ff, 0x1ff, 0x1ff, 0x1ff, index);
        // this ^ address will recursively point to the phys addr of the PDPT
        unsafe {
            let phys_addr = *(phys_addr_ptr as *const usize);
            let pdpt_page = phys_addr & 0xffff_ffff_ffff_f000;
            let mut offset: usize = 0;
            for (start_page, num_pages) in heap_phys_regions {
                let start_page = *start_page as *const PhysPage4KiB as usize;
                let end_page = start_page + 0x1000 * ((*num_pages) - 1);
                if pdpt_page >= start_page && pdpt_page <= end_page {
                    // in this region
                    offset += pdpt_page - start_page;
                    let virt_addr = HEAP_START + offset;
                    return &mut *(virt_addr as *mut PDPT);
                }
                offset += num_pages * 0x1000;
            }
            panic!("could not find {:#x}", pdpt_page)
        }
    }
}

fn indicies_of_vaddr(vaddr: usize) -> (usize, usize, usize, usize) {
    if (vaddr & 0x_8000_0000_0000 == 0x_8000_0000_0000
        && vaddr & 0xffff_8000_0000_0000 != 0xffff_8000_0000_0000)
        || (vaddr & 0x_8000_0000_0000 == 0 && vaddr & 0xffff_8000_0000_0000 != 0)
    {
        panic!(
            "Vaddr not cannonical: {:#x} {:#x}",
            vaddr,
            vaddr & 0xffff_f000_0000_0000
        );
    }
    let pml4_ind: usize = (vaddr >> 39) & 0x1ff;
    let pdpt_ind: usize = (vaddr >> 30) & 0x1ff;
    let pd_ind: usize = (vaddr >> 21) & 0x1ff;
    let pt_ind: usize = (vaddr >> 12) & 0x1ff;
    (pml4_ind, pdpt_ind, pd_ind, pt_ind)
}

fn indicies_to_vaddr(
    pml4_ind: usize,
    pdpt_ind: usize,
    pd_ind: usize,
    pt_ind: usize,
    p_ind: usize,
) -> usize {
    if pml4_ind > 0x1ff || pdpt_ind > 0x1ff || pd_ind > 0x1ff || pt_ind > 0x1ff || p_ind > 0x1ff {
        panic!("Invalid offset");
    }
    let mut addr: usize = 0;
    addr += pml4_ind << 39;
    addr += pdpt_ind << 30;
    addr += pd_ind << 21;
    addr += pt_ind << 12;
    addr += p_ind * mem::size_of::<PDPTE>();
    if addr & 0x_8000_0000_0000 == 0x_8000_0000_0000 {
        addr |= 0xffff_0000_0000_0000;
    }
    addr
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PML4E {
    // Page Map Level 4 Entry
    data: u64,
}

impl PML4E {
    #[allow(dead_code)]
    #[inline(always)]
    pub fn present(&self) -> bool {
        self.data & 0b1 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn writable(&self) -> bool {
        self.data & 0b10 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn user_accessable(&self) -> bool {
        self.data & 0b100 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn was_accessed(&self) -> bool {
        self.data & 0b100000 != 0
    }

    fn clear(&mut self) {
        self.data = 0;
    }

    #[inline(always)]
    pub fn pdpt(&self) -> Option<&'static mut PDPT> {
        if self.present() {
            unsafe { Some(&mut *(((self.data as usize) & 0xffffffffff000) as *mut PDPT)) }
        } else {
            None
        }
    }
}

#[derive(Debug)]
#[repr(align(4096))]
#[repr(C)]
pub struct PDPT {
    // Page Directory Pointer Table
    pub entries: [PDPTE; PAGE_TABLE_SIZE],
}

impl PDPT {
    pub unsafe fn new() -> &'static mut Self {
        let ptr = Global.allocate_zeroed(Layout::new::<PDPT>());

        if ptr.is_err() {
            panic!("Alloc Error");
        }

        &mut *(ptr.unwrap().as_mut_ptr() as *mut PDPT)
    }

    pub unsafe fn add(&mut self, index: usize, pd: &PD, writable: bool, user_accessable: bool) {
        let phys_ptr = pd as *const PD as usize;
        if phys_ptr % 0x1000 != 0 || phys_ptr & 0xfff0000000000000 != 0 {
            panic!("PD not aligned");
        }
        if index >= self.entries.len() {
            panic!("index out of range");
        }

        let mut data = phys_ptr & 0xffffffffff000;
        if writable {
            data |= 0b10;
        }
        if user_accessable {
            data |= 0b100;
        }
        data |= 0b1; // present
        self.entries[index].data = data as u64;
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PDPTE {
    // Page Directory Pointer Table Entry
    data: u64,
}

impl PDPTE {
    #[allow(dead_code)]
    #[inline(always)]
    pub fn present(&self) -> bool {
        self.data & 0b1 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn writable(&self) -> bool {
        self.data & 0b10 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn user_accessable(&self) -> bool {
        self.data & 0b100 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn was_accessed(&self) -> bool {
        self.data & 0b100000 != 0
    }

    fn clear(&mut self) {
        self.data = 0;
    }

    #[inline(always)]
    pub fn pd(&self) -> Option<&'static mut PD> {
        if self.present() {
            unsafe { Some(&mut *(((self.data as usize) & 0xffffffffff000) as *mut PD)) }
        } else {
            None
        }
    }
}

#[derive(Debug)]
#[repr(align(4096))]
#[repr(C)]
pub struct PD {
    // Page Directory
    pub entries: [PDE; PAGE_TABLE_SIZE],
}

impl PD {
    pub unsafe fn new() -> &'static mut Self {
        let ptr = Global.allocate_zeroed(Layout::new::<PD>());

        if ptr.is_err() {
            panic!("Alloc Error");
        }

        &mut *(ptr.unwrap().as_mut_ptr() as *mut PD)
    }

    pub unsafe fn add(&mut self, index: usize, pt: &PT, writable: bool, user_accessable: bool) {
        let phys_ptr = pt as *const PT as usize;
        if phys_ptr % 0x1000 != 0 || phys_ptr & 0xfff0000000000000 != 0 {
            panic!("PT not aligned");
        }
        if index >= self.entries.len() {
            panic!("index out of range");
        }

        let mut data = phys_ptr & 0xffffffffff000;
        if writable {
            data |= 0b10;
        }
        if user_accessable {
            data |= 0b100;
        }
        data |= 0b1; // present
        self.entries[index].data = data as u64;
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PDE {
    // Page Directory Entry
    data: u64,
}

#[derive(Debug)]
#[repr(align(0x200000))]
#[repr(C)]
pub struct PhysPage2MiB {
    entries: [u8; 0x200000],
}

impl PDE {
    #[allow(dead_code)]
    #[inline(always)]
    pub fn present(&self) -> bool {
        self.data & 0b1 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn writable(&self) -> bool {
        self.data & 0b10 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn user_accessable(&self) -> bool {
        self.data & 0b100 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn was_accessed(&self) -> bool {
        self.data & 0b100000 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn dirty(&self) -> bool {
        self.data & 0b1000000 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn big_page_enabled(&self) -> bool {
        self.data & 0b10000000 != 0
    }

    fn clear(&mut self) {
        self.data = 0;
    }

    #[inline(always)]
    pub fn pt(&self) -> Option<&'static mut PT> {
        if self.present() {
            if !self.big_page_enabled() {
                unsafe { Some(&mut *(((self.data as usize) & 0xffffffffff000) as *mut PT)) }
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn big_page(&self) -> Option<&'static PhysPage2MiB> {
        if self.present() {
            if self.big_page_enabled() {
                unsafe { Some(&*(((self.data as usize) & 0xfffffffe00000) as *const PhysPage2MiB)) }
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug)]
#[repr(align(4096))]
#[repr(C)]
pub struct PT {
    // Page Directory
    pub entries: [PTE; PAGE_TABLE_SIZE],
}

impl PT {
    pub unsafe fn new() -> &'static mut Self {
        let ptr = Global.allocate_zeroed(Layout::new::<PT>());

        if ptr.is_err() {
            if let Err(e) = ptr {
                panic!("Alloc Error {}", e);
            } else {
                panic!("Alloc Error");
            }
        }

        &mut *(ptr.unwrap().as_mut_ptr() as *mut PT)
    }

    pub unsafe fn add(&mut self, index: usize, page: usize, writable: bool, user_accessable: bool) {
        let phys_ptr = page;
        if phys_ptr % 0x1000 != 0 || phys_ptr & 0xfff0000000000000 != 0 {
            panic!("Page not aligned {:#x}", phys_ptr);
        }
        if index >= self.entries.len() {
            panic!("index out of range");
        }

        let mut data = phys_ptr & 0xffffffffff000;
        if writable {
            data |= 0b10;
        }
        if user_accessable {
            data |= 0b100;
        }
        data |= 0b1; // present
        self.entries[index].data = data as u64;
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PTE {
    // Page Directory Entry
    data: u64,
}

#[derive(Debug)]
#[repr(align(4096))]
#[repr(C)]
pub struct PhysPage4KiB {
    bytes: [u8; 0x1000],
}

#[derive(Debug)]
#[repr(align(4096))]
#[repr(C)]
pub struct VirtPage4KiB {
    bytes: [u8; 0x1000],
}

impl PTE {
    #[allow(dead_code)]
    #[inline(always)]
    pub fn present(&self) -> bool {
        self.data & 0b1 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn writable(&self) -> bool {
        self.data & 0b10 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn user_accessable(&self) -> bool {
        self.data & 0b100 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn was_accessed(&self) -> bool {
        self.data & 0b100000 != 0
    }

    #[allow(dead_code)]
    #[inline(always)]
    fn dirty(&self) -> bool {
        self.data & 0b1000000 != 0
    }

    fn clear(&mut self) {
        self.data = 0;
    }

    #[inline(always)]
    pub fn page(&self) -> Option<&'static PhysPage4KiB> {
        if self.present() {
            unsafe { Some(&*(((self.data as usize) & 0xffffffffff000) as *const PhysPage4KiB)) }
        } else {
            None
        }
    }
}

// unsafe as this reference is treated as static, but this is not fully true
// Should not be held for long
// This will lead to undefined behaviour when:
// current_page_table() called then cr3 is changed then this ref is used
// Possibly create another function that can be used with PML4's that have
// been created on the heap
pub unsafe fn current_page_table() -> &'static PML4 {
    let mut cr3: usize;
    asm!("mov {}, cr3", out(reg) cr3);
    &*(cr3 as *const PML4) as &PML4
}

pub fn set_page_table(pml4: &PML4) {
    let ptr = pml4 as *const PML4 as usize;
    if ptr % 0x1000 != 0 {
        panic!("PML4 not aligned");
    }
    unsafe {
        asm!( "mov cr3, {}", in(reg) ptr,);
    }
}
