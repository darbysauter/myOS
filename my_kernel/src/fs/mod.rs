use core::convert::TryInto;
use core::str;

use alloc::borrow::ToOwned;
use alloc::vec;
use alloc::{string::String, vec::Vec};

use crate::ahci::{HbaPort, SECTOR_SIZE};
use crate::memory::page_table::PhysPage4KiB;
use crate::println;

#[derive(Debug)]
pub struct SimpleFS {
    files: Vec<SimpleFile>,
}

const FS_MAGIC: u32 = 0x34127777;

impl SimpleFS {
    pub fn new(data: Vec<u8>) -> Self {
        assert!(data.len() == SECTOR_SIZE);

        let mut files = vec![];

        let magic: u32 =
            u32::from_le_bytes(data[0..4].try_into().expect("Couldn't get offset [1]"));

        assert!(magic == FS_MAGIC);

        let num_files =
            u32::from_le_bytes(data[4..8].try_into().expect("Couldn't get offset [1]")) as usize;

        let mut str_start = 8;
        let mut str_end = str_start;
        let mut file_names = vec![];
        for _ in 0..num_files {
            while data[str_end] != 0 {
                str_end += 1;
            }
            str_end += 1; // str_end is now byte after null byte
            let file_name = str::from_utf8(
                data[str_start..str_end - 1]
                    .try_into()
                    .expect("Couldn't get offset [1]"),
            )
            .expect("couldnt parse string");
            str_start = str_end;

            file_names.push(file_name);
        }
        // round up to nearest multiple of 8
        let mut files_info = vec![];
        let file_info_start = (str_end + 7) & !7usize;
        for i in 0..num_files {
            let offset = u64::from_le_bytes(
                data[file_info_start + (i * 0x10)..file_info_start + (i * 0x10) + 8]
                    .try_into()
                    .expect("Couldn't get offset [1]"),
            ) as usize;
            let size = u64::from_le_bytes(
                data[file_info_start + (i * 0x10) + 8..file_info_start + (i * 0x10) + 0x10]
                    .try_into()
                    .expect("Couldn't get offset [1]"),
            ) as usize;
            files_info.push((offset, size));
        }

        for i in 0..num_files {
            files.push(SimpleFile {
                offset: files_info[i].0,
                size: files_info[i].1,
                name: file_names[i].to_owned(),
            })
        }

        SimpleFS { files: files }
    }

    pub fn load_file(
        &self,
        name: &str,
        port: &mut HbaPort,
        heap_phys_regions: &Vec<(&'static PhysPage4KiB, usize)>,
    ) -> Option<Vec<u8>> {
        for file in self.files.iter() {
            if file.name == name {
                let startl = (file.offset / SECTOR_SIZE) as u32;
                let starth = ((file.offset / SECTOR_SIZE) >> 32) as u32;
                let mut sectors = file.size / SECTOR_SIZE;
                if file.size % SECTOR_SIZE != 0 {
                    sectors += 1;
                }
                return Some(
                    port.read(startl, starth, sectors, &heap_phys_regions)
                        .expect("read failed"),
                );
            }
        }
        None
    }
}

#[derive(Debug)]
struct SimpleFile {
    offset: usize,
    size: usize,
    name: String,
}
