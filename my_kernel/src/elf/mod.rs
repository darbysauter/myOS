use crate::bootloader_structs::BootInfo;
use alloc::vec::Vec;
use core::convert::TryInto;
use core::mem;
use core::slice;

#[derive(Debug)]
#[repr(C)]
pub struct ProgHeaderEntry {
    pub seg_type: u32,
    pub flags: u32,
    pub offset: usize,
    pub v_addr: usize,
    pub p_addr: usize,
    pub file_size: u64,
    pub mem_size: u64,
    pub align: u64,
}

#[derive(Debug)]
#[repr(C)]
pub struct SecHeaderEntry {
    pub name: u32,
    pub sec_type: u32,
    pub flags: u64,
    pub addr: usize,
    pub offset: usize,
    pub size: usize,
    pub link: u32,
    pub info: u32,
    pub addralign: u64,
    pub entsize: u64,
}

#[derive(Debug)]
#[repr(C)]
pub struct DynAddrEntry {
    pub addr: usize, // this address holds the actual address of object (used to look up)
    pub info: u64,
    pub points_to: usize, // this is the address of where the actual address would be assuming some base address (0x200000)
}

pub fn get_loadable_prog_header_entries(boot_info: &BootInfo) -> Vec<ProgHeaderEntry> {
    let e = {
        let ptr = boot_info.elf_location as *const u8;
        unsafe { slice::from_raw_parts(ptr, boot_info.elf_size as usize) }
    };

    let ph_off: usize = u64::from_le_bytes(
        e.get(0x20..0x28)
            .expect("Couldn't get offset [0]")
            .try_into()
            .expect("Couldn't get offset [1]"),
    )
    .try_into()
    .expect("Couldn't get offset [2]");

    let ph_ent_size: u16 = u16::from_le_bytes(
        e.get(0x36..0x38)
            .expect("Couldn't get ent size [0]")
            .try_into()
            .expect("Couldn't get ent size [1]"),
    )
    .try_into()
    .expect("Couldn't get ent size [2]");

    assert_eq!(ph_ent_size as usize, mem::size_of::<ProgHeaderEntry>());

    let ph_ent_num: u16 = u16::from_le_bytes(
        e.get(0x38..0x3a)
            .expect("Couldn't get ent num [0]")
            .try_into()
            .expect("Couldn't get ent num [1]"),
    )
    .try_into()
    .expect("Couldn't get ent num [2]");

    let prog_headers = {
        let ptr = (boot_info.elf_location + ph_off) as *const ProgHeaderEntry;
        unsafe { slice::from_raw_parts(ptr, ph_ent_num as usize) }
    };

    let mut vec = Vec::new();
    for entry in prog_headers {
        if entry.seg_type == 0x1 {
            vec.push(ProgHeaderEntry {
                seg_type: entry.seg_type,
                flags: entry.flags,
                offset: entry.offset,
                v_addr: entry.v_addr,
                p_addr: entry.p_addr,
                file_size: entry.file_size,
                mem_size: entry.mem_size,
                align: entry.align,
            });
        }
    }
    vec
}

pub fn get_global_offset_table(boot_info: &BootInfo) -> &mut [usize] {
    let e = {
        let ptr = boot_info.elf_location as *const u8;
        unsafe { slice::from_raw_parts(ptr, boot_info.elf_size as usize) }
    };

    let sh_off: usize = u64::from_le_bytes(
        e.get(0x28..0x30)
            .expect("Couldn't get offset [0]")
            .try_into()
            .expect("Couldn't get offset [1]"),
    )
    .try_into()
    .expect("Couldn't get offset [2]");

    let sh_ent_size: u16 = u16::from_le_bytes(
        e.get(0x3a..0x3c)
            .expect("Couldn't get ent size [0]")
            .try_into()
            .expect("Couldn't get ent size [1]"),
    )
    .try_into()
    .expect("Couldn't get ent size [2]");

    assert_eq!(sh_ent_size as usize, mem::size_of::<SecHeaderEntry>());

    let sh_ent_num: u16 = u16::from_le_bytes(
        e.get(0x3c..0x3e)
            .expect("Couldn't get ent num [0]")
            .try_into()
            .expect("Couldn't get ent num [1]"),
    )
    .try_into()
    .expect("Couldn't get ent num [2]");

    let sh_str_index: u16 = u16::from_le_bytes(
        e.get(0x3e..0x40)
            .expect("Couldn't get ent num [0]")
            .try_into()
            .expect("Couldn't get ent num [1]"),
    )
    .try_into()
    .expect("Couldn't get ent num [2]");

    let sec_headers = {
        let ptr = (boot_info.elf_location + sh_off) as *const SecHeaderEntry;
        unsafe { slice::from_raw_parts(ptr, sh_ent_num as usize) }
    };

    let sec_sh_names = &sec_headers[sh_str_index as usize];

    let sh_names_char_array = {
        let ptr = (boot_info.elf_location + sec_sh_names.offset) as *const u8;
        unsafe { slice::from_raw_parts(ptr, sec_sh_names.size) }
    };

    for entry in sec_headers {
        if sh_names_char_array.get(entry.name as usize..(entry.name + 4) as usize) == Some(b".got")
        {
            let got = {
                let ptr = (entry.addr) as *mut usize;
                assert_eq!(entry.size % mem::size_of::<usize>(), 0);
                unsafe { slice::from_raw_parts_mut(ptr, entry.size / mem::size_of::<usize>()) }
            };
            return got;
        }
    }
    panic!("Could not find GOT");
}
pub fn fix_relocatable_addrs(boot_info: &BootInfo, offset: usize) {
    let e = {
        let ptr = boot_info.elf_location as *const u8;
        unsafe { slice::from_raw_parts(ptr, boot_info.elf_size as usize) }
    };

    let ph_off: usize = u64::from_le_bytes(
        e.get(0x20..0x28)
            .expect("Couldn't get offset [0]")
            .try_into()
            .expect("Couldn't get offset [1]"),
    )
    .try_into()
    .expect("Couldn't get offset [2]");

    let ph_ent_size: u16 = u16::from_le_bytes(
        e.get(0x36..0x38)
            .expect("Couldn't get ent size [0]")
            .try_into()
            .expect("Couldn't get ent size [1]"),
    )
    .try_into()
    .expect("Couldn't get ent size [2]");

    assert_eq!(ph_ent_size as usize, mem::size_of::<ProgHeaderEntry>());

    let ph_ent_num: u16 = u16::from_le_bytes(
        e.get(0x38..0x3a)
            .expect("Couldn't get ent num [0]")
            .try_into()
            .expect("Couldn't get ent num [1]"),
    )
    .try_into()
    .expect("Couldn't get ent num [2]");

    let prog_headers = {
        let ptr = (boot_info.elf_location + ph_off) as *const ProgHeaderEntry;
        unsafe { slice::from_raw_parts(ptr, ph_ent_num as usize) }
    };

    for entry in prog_headers {
        if entry.seg_type == 0x2 {
            // PT_DYNAMIC
            let dyn_table: &[DynAddrEntry] = {
                unsafe {
                    let addr = (*((entry.v_addr + 0x28) as *const usize)) as *const DynAddrEntry;
                    let tbl_size = *((entry.v_addr + 0x38) as *const usize);
                    let ent_size = *((entry.v_addr + 0x48) as *const usize);
                    let count = *((entry.v_addr + 0x58) as *const usize);
                    assert_eq!(ent_size * count, tbl_size);
                    assert_eq!(ent_size, mem::size_of::<DynAddrEntry>());
                    slice::from_raw_parts(addr, tbl_size / mem::size_of::<DynAddrEntry>())
                }
            };

            for dyn_addr_entry in dyn_table {
                let addr = dyn_addr_entry.addr as *mut usize;
                unsafe {
                    *addr += offset as usize;
                }
            }
            return;
        }
    }
    panic!("Could not find Relocatable Table");
}
