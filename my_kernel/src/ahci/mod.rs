use core::mem;

use alloc::{boxed::Box, vec::Vec};

use crate::{
    memory::{
        heap::{translate_ref_to_phys, translate_usize_to_phys, translate_usize_to_virt},
        page_table::PhysPage4KiB,
    },
    println,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum FisType {
    FisTypeRegH2d = 0x27,   // Register FIS - host to device
    FisTypeRegD2h = 0x34,   // Register FIS - device to host
    FisTypeDmaAct = 0x39,   // DMA activate FIS - device to host
    FisTypeDmaSetup = 0x41, // DMA setup FIS - bidirectional
    FisTypeData = 0x46,     // Data FIS - bidirectional
    FisTypeBist = 0x58,     // BIST activate FIS - bidirectional
    FisTypePioSetup = 0x5F, // PIO setup FIS - device to host
    FisTypeDevBits = 0xA1,  // Set device bits FIS - device to host
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
struct FisRegH2d {
    fis_type: u8, // FIS_TYPE_REG_H2D
    pmult: u8,    // xoooxxxx
    // highest bit is 1 for command or 0 for control
    // lowest nibble is for port multiplier
    command: u8,  // Command register
    featurel: u8, // Feature register, 7:0
    lba0: u8,     // LBA low register, 7:0
    lba1: u8,     // LBA mid register, 15:8
    lba2: u8,     // LBA high register, 23:16
    device: u8,   // Device register
    lba3: u8,     // LBA register, 31:24
    lba4: u8,     // LBA register, 39:32
    lba5: u8,     // LBA register, 47:40
    featureh: u8, // Feature register, 15:8
    countl: u8,   // Count register, 7:0
    counth: u8,   // Count register, 15:8
    icc: u8,      // Isochronous command completion
    control: u8,  // Control register
    rsv1: u8,     // Reserved
    rsv2: u8,     // Reserved
    rsv3: u8,     // Reserved
    rsv4: u8,     // Reserved
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
struct FisRegD2h {
    fis_type: u8, // FIS_TYPE_REG_D2H
    pmult: u8,    // oxooxxxx
    // 2nd highest bit is for interrupt
    // lowest nibble is for port multiplier
    status: u8, // Status register
    error: u8,  // Error register
    lba0: u8,   // LBA low register, 7:0
    lba1: u8,   // LBA mid register, 15:8
    lba2: u8,   // LBA high register, 23:16
    device: u8, // Device register
    lba3: u8,   // LBA register, 31:24
    lba4: u8,   // LBA register, 39:32
    lba5: u8,   // LBA register, 47:40
    rsv2: u8,   // Reserved
    countl: u8, // Count register, 7:0
    counth: u8, // Count register, 15:8
    rsv3: u8,   // Reserved
    rsv4: u8,   // Reserved
    rsv5: u8,   // Reserved
    rsv6: u8,   // Reserved
    rsv7: u8,   // Reserved
    rsv8: u8,   // Reserved
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
struct FisDevBits {
    fis_type: u8,
    pmult: u8, // oxooxxxx
    // uint8_t pmport:4;
    // uint8_t rsvd:2;
    // uint8_t i:1;
    // uint8_t n:1;
    status: u8,
    // uint8_t statusl:3;
    // uint8_t rsvd2:1;
    // uint8_t statush:3;
    // uint8_t rsvd3:1;
    error: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
struct FisData {
    fis_type: u8, // FIS_TYPE_DATA
    pmult: u8,    // ooooxxxx
    // lowest nibble is for port multiplier
    rsv1: u8,  // Reserved
    rsv2: u8,  // Reserved
    data: u32, // Payload
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
struct FisPioSetup {
    fis_type: u8, // FIS_TYPE_PIO_SETUP
    pmult: u8,    // oxxoxxxx
    // 2nd highest bit is for interrupt
    // 3rd highest bit is for direction (1 is d2h)
    // lowest nibble is for port multiplier
    status: u8,   // Status register
    error: u8,    // Error register
    lba0: u8,     // LBA low register, 7:0
    lba1: u8,     // LBA mid register, 15:8
    lba2: u8,     // LBA high register, 23:16
    device: u8,   // Device register
    lba3: u8,     // LBA register, 31:24
    lba4: u8,     // LBA register, 39:32
    lba5: u8,     // LBA register, 47:40
    rsv2: u8,     // Reserved
    countl: u8,   // Count register, 7:0
    counth: u8,   // Count register, 15:8
    rsv3: u8,     // Reserved
    e_status: u8, // New value of status register
    tc: u16,      // Transfer count
    rsv4: u8,     // Reserved
    rsv5: u8,     // Reserved
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
struct FisDmaSetup {
    fis_type: u8, // FIS_TYPE_DMA_SETUP
    pmult: u8,    // xxxoxxxx
    // highest bit auto activate
    // 2nd highest bit is for interrupt
    // 3rd highest bit is for direction (1 is d2h)
    // lowest nibble is for port multiplier
    rsvd0: u8,         // Reserved
    rsvd1: u8,         // Reserved
    dmabuffer_id: u64, // DMA Buffer Identifier. Used to Identify DMA buffer in host memory.
    // SATA Spec says host specific and not in Spec. Trying AHCI spec might work.
    rsvd: u32,           //More reserved
    dmabuf_offset: u32,  //Byte offset into buffer. First 2 bits must be 0
    transfer_count: u32, //Number of bytes to transfer. Bit 0 must be 0
    resvd: u32,          //Reserved
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct HbaMem {
    // 0x00 - 0x2B, Generic Host Control
    pub cap: u32,     // 0x00, Host capability
    pub ghc: u32,     // 0x04, Global host control
    pub is: u32,      // 0x08, Interrupt status
    pub pi: u32,      // 0x0C, Port implemented
    pub vs: u32,      // 0x10, Version
    pub ccc_ctl: u32, // 0x14, Command completion coalescing control
    pub ccc_pts: u32, // 0x18, Command completion coalescing ports
    pub em_loc: u32,  // 0x1C, Enclosure management location
    pub em_ctl: u32,  // 0x20, Enclosure management control
    pub cap2: u32,    // 0x24, Host capabilities extended
    pub bohc: u32,    // 0x28, BIOS/OS handoff control and status

    // 0x2C - 0x9F, Reserved
    rsv: [u8; 0xA0 - 0x2C],

    // 0xA0 - 0xFF, Vendor specific registers
    vendor: [u8; 0x100 - 0xA0],

    // 0x100 - 0x10FF, Port control registers
    pub ports: [HbaPort; 32], // 1 ~ 32
}

impl Default for HbaMem {
    #[inline]
    fn default() -> HbaMem {
        HbaMem {
            cap: 0,
            ghc: 0,
            is: 0,
            pi: 0,
            vs: 0,
            ccc_ctl: 0,
            ccc_pts: 0,
            em_loc: 0,
            em_ctl: 0,
            cap2: 0,
            bohc: 0,
            rsv: [0; 0xA0 - 0x2C],
            vendor: [0; 0x100 - 0xA0],
            ports: [HbaPort::default(); 32],
        }
    }
}

impl HbaMem {
    // returns index of implemented ports
    pub fn implemented_ports(&self) -> Vec<usize> {
        let mut ind_sel = 1;
        let mut vec = Vec::new();
        for i in 0..32 {
            if ind_sel & self.pi != 0 {
                vec.push(i);
            }
            ind_sel = ind_sel << 1;
        }
        vec
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct HbaPort {
    pub clb: u32,     // 0x00, command list base address, 1K-byte aligned
    pub clbu: u32,    // 0x04, command list base address upper 32 bits
    pub fb: u32,      // 0x08, FIS base address, 256-byte aligned
    pub fbu: u32,     // 0x0C, FIS base address upper 32 bits
    pub is: u32,      // 0x10, interrupt status
    pub ie: u32,      // 0x14, interrupt enable
    pub cmd: u32,     // 0x18, command and status
    pub rsv0: u32,    // 0x1C, Reserved
    pub tfd: u32,     // 0x20, task file data
    pub sig: u32,     // 0x24, signature
    pub ssts: u32,    // 0x28, SATA status (SCR0:SStatus)
    pub sctl: u32,    // 0x2C, SATA control (SCR2:SControl)
    pub serr: u32,    // 0x30, SATA error (SCR1:SError)
    pub sact: u32,    // 0x34, SATA active (SCR3:SActive)
    pub ci: u32,      // 0x38, command issue
    pub sntf: u32,    // 0x3C, SATA notification (SCR4:SNotification)
    pub fbs: u32,     // 0x40, FIS-based switch control
    rsv1: [u32; 11],  // 0x44 ~ 0x6F, Reserved
    vendor: [u32; 4], // 0x70 ~ 0x7F, vendor specific
}

impl Default for HbaPort {
    #[inline]
    fn default() -> HbaPort {
        HbaPort {
            clb: 0,
            clbu: 0,
            fb: 0,
            fbu: 0,
            is: 0,
            ie: 0,
            cmd: 0,
            rsv0: 0,
            tfd: 0,
            sig: 0,
            ssts: 0,
            sctl: 0,
            serr: 0,
            sact: 0,
            ci: 0,
            sntf: 0,
            fbs: 0,
            rsv1: [0; 11],
            vendor: [0; 4],
        }
    }
}

const HBA_PxCMD_ST: u32 = 0x0001;
const HBA_PxCMD_FRE: u32 = 0x0010;
const HBA_PxCMD_FR: u32 = 0x4000;
const HBA_PxCMD_CR: u32 = 0x8000;

const ATA_DEV_BUSY: u32 = 0x80;
const ATA_DEV_DRQ: u32 = 0x08;

const ATA_CMD_READ_DMA: u8 = 0xC8;
const ATA_CMD_READ_DMA_EX: u8 = 0x25;
const ATA_CMD_WRITE_DMA: u8 = 0xCA;
const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;

const HBA_PxIS_TFES: u32 = 1 << 30; // TFES - Task File Error Status

impl HbaPort {
    pub fn port_rebase(&mut self, heap_regions: &Vec<(&PhysPage4KiB, usize)>) -> Box<PortSetup> {
        self.stop_cmd(); // Stop command engine

        let mut port_setup = Box::new(PortSetup::default());

        let cmd_list = unsafe {
            (translate_ref_to_phys(heap_regions, &port_setup.cmd_list)) as *const _ as usize
        };
        self.clb = (cmd_list & 0xffffffff) as u32;
        self.clbu = ((cmd_list >> 32) & 0xffffffff) as u32;

        let fis_entry = unsafe {
            (translate_ref_to_phys(heap_regions, &port_setup.fis_entry)) as *const _ as usize
        };
        self.fb = (fis_entry & 0xffffffff) as u32;
        self.fbu = ((fis_entry >> 32) & 0xffffffff) as u32;

        for i in 0..32 {
            port_setup.cmd_list[i].prdtl = 8; // 8 prdt entries per command table
                                              // 256 bytes per command table, 64+16+48+16*8
                                              // Command table offset: 40K + 8K*portno + cmdheader_index*256
            let cmd_table = unsafe {
                (translate_ref_to_phys(heap_regions, &(port_setup.cmd_table[i]))) as *const _
                    as usize
            };
            port_setup.cmd_list[i].ctba = (cmd_table & 0xffffffff) as u32;
            port_setup.cmd_list[i].ctbau = ((cmd_table >> 32) & 0xffffffff) as u32;
        }

        self.start_cmd(); // Start command engine

        port_setup
    }

    // Start command engine
    fn start_cmd(&mut self) {
        // Wait until CR (bit15) is cleared
        while self.cmd & HBA_PxCMD_CR != 0 {}
        // Set FRE (bit4) and ST (bit0)
        self.cmd |= HBA_PxCMD_FRE;
        self.cmd |= HBA_PxCMD_ST;
    }

    // Stop command engine
    fn stop_cmd(&mut self) {
        // Clear ST (bit0)
        self.cmd &= !HBA_PxCMD_ST;

        // Clear FRE (bit4)
        self.cmd &= !HBA_PxCMD_FRE;

        // Wait until FR (bit14), CR (bit15) are cleared
        loop {
            if self.cmd & HBA_PxCMD_FR != 0 {
                continue;
            }
            if self.cmd & HBA_PxCMD_CR != 0 {
                continue;
            }
            break;
        }
    }

    fn cmd_header<'a>(
        &'a self,
        slot: usize,
        heap_regions: &Vec<(&PhysPage4KiB, usize)>,
    ) -> &'a mut HbaCmdHeader {
        let ptr = self.clb as usize + ((self.clbu as usize) << 32);
        let ptr = ptr + slot * mem::size_of::<HbaCmdHeader>();
        let ptr = unsafe { translate_usize_to_virt(heap_regions, ptr) };
        unsafe { &mut *(ptr as *mut HbaCmdHeader) as &'a mut HbaCmdHeader }
    }

    pub fn read(
        &mut self,
        startl: u32,
        starth: u32,
        count: u32,
        buf: &mut Vec<u16>,
        heap_regions: &Vec<(&PhysPage4KiB, usize)>,
    ) -> bool {
        let mut count = count;
        self.is = u32::MAX; // Clear pending interrupt bits
        let mut spin = 0; // Spin lock timeout counter
        let slot = self.find_cmdslot();
        if slot.is_none() {
            return false;
        }
        let slot = slot.unwrap();

        let cmdheader = self.cmd_header(slot, heap_regions);
        let size = (mem::size_of::<FisRegH2d>() / mem::size_of::<u32>()) as u8;
        cmdheader.cfl(size); // Command FIS size
        cmdheader.w(0); // Read from device
        cmdheader.prdtl = (((count - 1) >> 4) + 1) as u16; // PRDT entries count

        let mut cmdtbl = cmdheader.cmd_table(heap_regions);

        cmdtbl.clear();

        // 8K bytes (16 sectors) per PRDT
        let mut buf_addr = buf.as_mut_ptr() as usize;
        buf_addr = unsafe { translate_usize_to_phys(heap_regions, buf_addr) };
        let mut ind = 0;
        for i in 0..(cmdheader.prdtl - 1) as usize {
            cmdtbl.prdt_entry[i].dba = (buf_addr & 0xffffffff) as u32;
            cmdtbl.prdt_entry[i].dbau = ((buf_addr >> 32) & 0xffffffff) as u32;
            cmdtbl.prdt_entry[i].dbc = 8 * 1024 - 1; // 8K bytes (this value should always be set to 1 less than the actual value)
            cmdtbl.prdt_entry[i].interrupt(true);
            buf_addr += 2 * 4 * 1024; // 4K words
            count -= 16; // 16 sectors
            ind = i;
        }
        // Last entry
        cmdtbl.prdt_entry[ind].dba = (buf_addr & 0xffffffff) as u32;
        cmdtbl.prdt_entry[ind].dbau = ((buf_addr >> 32) & 0xffffffff) as u32;
        cmdtbl.prdt_entry[ind].dbc = (count << 9) - 1; // 512 bytes per sector
        cmdtbl.prdt_entry[ind].interrupt(true);

        // Setup command
        // FIS_REG_H2D *cmdfis = &cmdtbl.cfis;
        let cmdfis = unsafe { &mut *(&cmdtbl.cfis as *const _ as usize as *mut FisRegH2d) };

        cmdfis.fis_type = FisType::FisTypeRegH2d as u8;
        cmdfis.pmult = 0b10000000; // Command
        cmdfis.command = ATA_CMD_READ_DMA_EX;

        cmdfis.lba0 = startl as u8;
        cmdfis.lba1 = (startl >> 8) as u8;
        cmdfis.lba2 = (startl >> 16) as u8;
        cmdfis.device = 1 << 6; // LBA mode

        cmdfis.lba3 = (startl >> 24) as u8;
        cmdfis.lba4 = starth as u8;
        cmdfis.lba5 = (starth >> 8) as u8;

        cmdfis.countl = (count & 0xFF) as u8;
        cmdfis.counth = ((count >> 8) & 0xFF) as u8;

        // The below loop waits until the port is no longer busy before issuing a new command
        while (self.tfd & (ATA_DEV_BUSY | ATA_DEV_DRQ)) != 0 && spin < 1000000 {
            spin += 1;
        }
        if spin == 1000000 {
            panic!("Port is hung\n");
        }

        self.ci = 1 << slot; // Issue command

        // Wait for completion
        loop {
            // In some longer duration reads, it may be helpful to spin on the DPS bit
            // in the PxIS port field as well (1 << 5)
            if (self.ci & (1 << slot)) == 0 {
                break;
            }
            if (self.is & HBA_PxIS_TFES) != 0
            // Task file error
            {
                panic!("Read disk error\n");
            }
        }

        // Check again
        if self.is & HBA_PxIS_TFES != 0 {
            panic!("Read disk error\n");
        }

        return true;
    }

    // Find a free command list slot
    fn find_cmdslot(&mut self) -> Option<usize> {
        // If not set in SACT and CI, the slot is free
        let mut slots = (self.sact | self.ci);
        for i in 0..32 {
            if ((slots & 1) == 0) {
                return Some(i);
            }
            slots >>= 1;
        }
        return None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PortSetup {
    cmd_list: [HbaCmdHeader; 32],
    fis_entry: HbaFis,
    cmd_table: [HbaCmdTbl; 32],
}

impl Default for PortSetup {
    #[inline]
    fn default() -> PortSetup {
        PortSetup {
            cmd_list: [HbaCmdHeader::default(); 32],
            fis_entry: HbaFis::default(),
            cmd_table: [HbaCmdTbl::default(); 32],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct HbaCmdHeader {
    // DW0
    config: u8, // pwalllll
    // uint8_t  cfl:5;		// Command FIS length in DWORDS, 2 ~ 16
    // uint8_t  a:1;		// ATAPI
    // uint8_t  w:1;		// Write, 1: H2D, 0: D2H
    // uint8_t  p:1;		// Prefetchable
    status: u8, // pppprcbr
    // uint8_t  r:1;		// Reset
    // uint8_t  b:1;		// BIST
    // uint8_t  c:1;		// Clear busy upon R_OK
    // uint8_t  rsv0:1;		// Reserved
    // uint8_t  pmp:4;		// Port multiplier port
    prdtl: u16, // Physical region descriptor table length in entries

    // DW1
    prdbc: u32, // Physical region descriptor byte count transferred

    // DW2, 3
    ctba: u32,  // Command table descriptor base address
    ctbau: u32, // Command table descriptor base address upper 32 bits

    // DW4 - 7
    rsv1: [u32; 4], // Reserved
}

impl HbaCmdHeader {
    fn cmd_table<'a>(&'a self, heap_regions: &Vec<(&PhysPage4KiB, usize)>) -> &'a mut HbaCmdTbl {
        let ptr = self.ctba as usize + ((self.ctbau as usize) << 32);
        let ptr = unsafe { translate_usize_to_virt(heap_regions, ptr) };
        unsafe { &mut *(ptr as *mut HbaCmdTbl) as &'a mut HbaCmdTbl }
    }

    fn cfl(&mut self, cfl: u8) {
        self.config = cfl & 0b11111;
    }

    fn w(&mut self, cfl: u8) {
        self.config = cfl & 0b1000000;
    }
}

impl Default for HbaCmdHeader {
    #[inline]
    fn default() -> HbaCmdHeader {
        HbaCmdHeader {
            config: 0,
            status: 0,
            prdtl: 0,
            prdbc: 0,
            ctba: 0,
            ctbau: 0,
            rsv1: [0; 4],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct HbaFis {
    // 0x00
    dsfis: FisDmaSetup, // DMA Setup FIS
    pad0: [u8; 4],

    // 0x20
    psfis: FisPioSetup, // PIO Setup FIS
    pad1: [u8; 12],

    // 0x40
    rfis: FisRegD2h, // Register – Device to Host FIS
    pad2: [u8; 4],

    // 0x58
    sdbfis: FisDevBits, // Set Device Bit FIS

    // 0x60
    ufis: [u8; 64],

    // 0xA0
    rsv: [u8; 0x100 - 0xA0],
}

impl Default for HbaFis {
    #[inline]
    fn default() -> HbaFis {
        HbaFis {
            dsfis: FisDmaSetup::default(),
            pad0: [0; 4],
            psfis: FisPioSetup::default(),
            pad1: [0; 12],
            rfis: FisRegD2h::default(),
            pad2: [0; 4],
            sdbfis: FisDevBits::default(),
            ufis: [0; 64],
            rsv: [0; 0x100 - 0xA0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct HbaCmdTbl {
    // 0x00
    cfis: [u8; 64], // Command FIS

    // 0x40
    acmd: [u8; 16], // ATAPI command, 12 or 16 bytes

    // 0x50
    rsv: [u8; 48], // Reserved

    // 0x80
    prdt_entry: [HbaPrdtEntry; 32], // Physical region descriptor table entries, 0 ~ 65535
}

impl HbaCmdTbl {
    fn clear(&mut self) {
        self.cfis.iter_mut().for_each(|m| *m = 0);
        self.acmd.iter_mut().for_each(|m| *m = 0);
        self.rsv.iter_mut().for_each(|m| *m = 0);
        self.prdt_entry
            .iter_mut()
            .for_each(|m| *m = HbaPrdtEntry::default());
    }
}

impl Default for HbaCmdTbl {
    #[inline]
    fn default() -> HbaCmdTbl {
        HbaCmdTbl {
            cfis: [0; 64],
            acmd: [0; 16],
            rsv: [0; 48],
            prdt_entry: [HbaPrdtEntry::default(); 32],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
struct HbaPrdtEntry {
    dba: u32,  // Data base address
    dbau: u32, // Data base address upper 32 bits
    rsv0: u32, // Reserved

    // DW3
    dbc: u32, // byte count 22bit , top bit interrupt on complete
}

impl HbaPrdtEntry {
    fn interrupt(&mut self, enable: bool) {
        if enable {
            self.dbc |= 1 << 31;
        } else {
            self.dbc &= !(1 << 31);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AhciDevType {
    AHCI_DEV_NULL,
    AHCI_DEV_SATA,
    AHCI_DEV_SEMB,
    AHCI_DEV_PM,
    AHCI_DEV_SATAPI,
}

const HBA_PORT_IPM_ACTIVE: u8 = 1;
const HBA_PORT_DET_PRESENT: u8 = 3;

const SATA_SIG_ATA: u32 = 0x00000101; // SATA drive
const SATA_SIG_ATAPI: u32 = 0xEB140101; // SATAPI drive
const SATA_SIG_SEMB: u32 = 0xC33C0101; // Enclosure management bridge
const SATA_SIG_PM: u32 = 0x96690101; // Port multiplier

pub fn check_type(port: &HbaPort) -> AhciDevType {
    let ssts = port.ssts;

    let ipm: u8 = ((ssts >> 8) & 0x0F) as u8;
    let det: u8 = (ssts & 0x0F) as u8;

    if det != HBA_PORT_DET_PRESENT {
        // Check drive status
        return AhciDevType::AHCI_DEV_NULL;
    }
    if ipm != HBA_PORT_IPM_ACTIVE {
        return AhciDevType::AHCI_DEV_NULL;
    }

    match port.sig {
        SATA_SIG_ATAPI => return AhciDevType::AHCI_DEV_SATAPI,
        SATA_SIG_SEMB => return AhciDevType::AHCI_DEV_SEMB,
        SATA_SIG_PM => return AhciDevType::AHCI_DEV_PM,
        _ => return AhciDevType::AHCI_DEV_SATA,
    }
}
