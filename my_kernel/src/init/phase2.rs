use alloc::vec;

use crate::ahci::HbaMem;
use crate::alloc::vec::Vec;
use crate::apic::{
    disable_pic, enable_apic, get_apic_base, set_apic_base, set_apic_tpr, start_apic_timer,
};
use crate::memory::frame_allocator::LinkedListFrameAllocator;
use crate::memory::heap::{heap_sanity_check, print_heap};
use crate::memory::page_table::{PhysPage4KiB, PML4};
use crate::memory::stack::create_new_user_stack_and_map;
use crate::println;
use crate::tss::*;
use crate::user_mode::{enable_syscalls, enter_user_mode};
use crate::{ahci, gdt::*, pci};
use crate::{fs, interrupts::*};

// At this point we have elf loadable segments, heap and stack all mapped into high memory
// The Page tables are on the heap.
// The old elf loadable regions that were in low memory are unmapped
// It is trivial now to map pages, allocate pages, and allocate memory on heap
pub fn phase2_init(
    pml4: &mut PML4,
    mut frame_alloc: LinkedListFrameAllocator,
    heap_phys_regions: Vec<(&'static PhysPage4KiB, usize)>,
) -> ! {
    // memory diagnostics
    println!("frame alloc has {:#x} free pages", frame_alloc.frame_count);
    heap_sanity_check();
    print_heap();

    // create new tss
    let tss = TSS::create_tss_on_heap();
    let (tss_hi, tss_lo) = TSS::create_gdt_entry(&tss);

    // TODO: Map TSS into user space

    // create new gdt and load it
    let mut gdt = GDT::create_gdt_on_heap(tss_hi, tss_lo);
    GDT::setup_gdt(&mut gdt);

    // create new idt and load it
    let mut idt = IDT::create_idt_on_heap();
    IDT::setup_idt(&mut idt);

    // unsafe {
    //     asm!("int3");
    // }

    let pci_devices = pci::pci_probe();

    let mut abar: usize = 0;
    for dev in pci_devices {
        if pci::get_class_id(dev.bus, dev.slot, dev.function) == 0x01
            && pci::get_sub_class_id(dev.bus, dev.slot, dev.function) == 0x06
        {
            println!(
                "{:#?} prog: {} bar5: {:#x}",
                dev,
                pci::get_sub_prog_if(dev.bus, dev.slot, dev.function),
                pci::get_bar_5(dev.bus, dev.slot, dev.function)
            );
            abar = pci::get_bar_5(dev.bus, dev.slot, dev.function) as usize;
        }
    }

    let mut abar: &'static mut HbaMem = unsafe {
        pml4.map_frame_4k(abar, abar, true, false, Some(&heap_phys_regions));
        &mut *(abar as *mut HbaMem) as &'static mut HbaMem
    };

    let mut sata_ports = Vec::new();
    for i in abar.implemented_ports() {
        if ahci::check_type(&abar.ports[i]) == ahci::AhciDevType::AHCI_DEV_SATA {
            sata_ports.push(i);
        }
    }

    let mut ports_setup = Vec::new();
    for i in sata_ports {
        let port_setup = abar.ports[i].port_rebase(&heap_phys_regions);
        ports_setup.push(port_setup);

        println!("about to read");

        let data = abar.ports[i]
            .read(0, 0, 1, &heap_phys_regions)
            .expect("read failed");

        let fs = fs::SimpleFS::new(data);

        println!("{:#?}", fs);
    }

    // enable_syscalls();
    // create_new_user_stack_and_map(&mut frame_alloc, pml4, &heap_phys_regions);
    // enter_user_mode();

    // let apic_base = get_apic_base();
    // set_apic_base(apic_base);
    // unsafe {
    //     enable_apic(apic_base);
    //     set_apic_tpr(apic_base, 0);
    // }
    // disable_pic();
    // enable_hardware_interrupts();
    // start_apic_timer(apic_base);

    println!("We didn't die! :)");
    loop {}
}
