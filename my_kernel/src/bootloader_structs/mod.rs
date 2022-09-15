// don't allow any paddings, just incase we make a mistake
// with the BootInfo in assembly
#[repr(C, packed)]
pub struct BootInfo {
    pub mem_map_entries: u32,
    pub mem_map: usize,
    pub elf_location: usize,
    pub elf_size: u32,
    pub stack_location: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct E820MemoryRegion {
    pub start_addr: u64,
    pub len: u64,
    pub region_type: u32,
    pub acpi_extended_attributes: u32,
}
