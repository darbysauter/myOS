use alloc::vec::Vec;
use core::arch::asm;

pub fn pci_config_read_word(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
    let address: u32;
    let lbus: u32 = bus as u32;
    let lslot: u32 = slot as u32;
    let lfunc: u32 = func as u32;

    // Create configuration address as per Figure 1
    address = ((lbus << 16) | (lslot << 11) | (lfunc << 8) | (offset as u32 & 0xFC) | (0x80000000))
        as u32;

    // Write out the address
    outl(0xCF8, address);
    // Read in the data
    // (offset & 2) * 8) = 0 will choose the first word of the 32-bit register
    let tmp = ((inl(0xCFC) >> ((offset & 2) * 8)) & 0xFFFF) as u16;
    return tmp;
}

fn get_vendor_id(bus: u8, slot: u8, function: u8) -> u16 {
    pci_config_read_word(bus, slot, function, 0)
}

fn get_device_id(bus: u8, slot: u8, function: u8) -> u16 {
    pci_config_read_word(bus, slot, function, 2)
}

pub fn get_class_id(bus: u8, slot: u8, function: u8) -> u16 {
    let r0 = pci_config_read_word(bus, slot, function, 0xA);
    return (r0 & !0x00FF) >> 8;
}

pub fn get_sub_class_id(bus: u8, slot: u8, function: u8) -> u16 {
    let r0 = pci_config_read_word(bus, slot, function, 0xA);
    return r0 & !0xFF00;
}

pub fn get_sub_prog_if(bus: u8, slot: u8, function: u8) -> u16 {
    let r0 = pci_config_read_word(bus, slot, function, 0x8);
    return (r0 & !0x00FF) >> 8;
}

pub fn get_bar_5(bus: u8, slot: u8, function: u8) -> u32 {
    let r0 = pci_config_read_word(bus, slot, function, 0x24);
    let r1 = pci_config_read_word(bus, slot, function, 0x26);
    return r0 as u32 + ((r1 as u32) << 16);
}

#[derive(Debug)]
pub struct PciDevice {
    pub vendor: u16,
    pub device: u16,
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
}

pub fn pci_probe() -> Vec<PciDevice> {
    let mut available_devices = Vec::new();
    for bus in 0..0xff {
        for slot in 0..0xff {
            for function in 0..0xff {
                let vendor = get_vendor_id(bus, slot, function);
                if vendor == 0xffff {
                    continue;
                }
                let device = get_device_id(bus, slot, function);
                available_devices.push(PciDevice {
                    vendor,
                    device,
                    bus,
                    slot,
                    function,
                });
            }
        }
    }
    available_devices
}

fn outl(port: u16, val: u32) {
    unsafe { asm!("outl %eax, %dx", in("eax") val, in("dx") port, options(att_syntax)) }
}

fn inl(port: u16) -> u32 {
    unsafe {
        let ret: u32;
        asm!("inl %dx, %eax", out("eax") ret, in("dx") port, options(att_syntax));
        ret
    }
}
