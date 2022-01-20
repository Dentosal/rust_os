// Represents useful attributes from 64-bit elf file

use core::{mem, ptr};

const KERNEL_ELF_IMAGE_POSITION: usize = 0x10_0000; // must match with plan.md
const MAX_PH_ENTRY_COUNT: usize = 20;

const ELF_MAGIC: u32 = 0x464c457f;
const ELF_BITNESS_64: u8 = 2;
const ELF_LITTLE_ENDIAN: u8 = 1;
const CURRENT_ELF_VERSION: u8 = 1;
const ELF_ARCH_X86_64: u16 = 0x3E;
const ELF_PH_TABLE_ENTRY_SIZE: u16 = 56;

#[derive(Debug, Copy, Clone)]
pub struct ELFData {
    pub header: ELFHeader,
    pub ph_table: [Option<ELFProgramHeader>; MAX_PH_ENTRY_COUNT],
}
impl ELFData {
    pub fn last_addr(&self) -> u64 {
        self.ph_table
            .iter()
            .copied()
            .filter_map(|p| Some(p?.virtual_address + p?.size_in_memory))
            .max()
            .unwrap()
    }
}

bitflags! {
    #[repr(transparent)]
    pub struct ELFPermissionFlags: u32 {
        const EXECUTABLE    = 1 << 0;
        const WRITABLE      = 1 << 1;
        const READABLE      = 1 << 2;
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct ELFHeader {
    magic: u32,
    bitness: u8,
    endianness: u8,
    elf_version: u8,
    abi_version: u8,
    _undef: u64,
    elf_type: u16,
    instrucion_set: u16,
    elf_version_2: u32,
    pub program_entry_pos: u64,
    ph_table_position: u64,
    sh_table_position: u64,
    flags: u32,
    header_size: u16,
    ph_table_entry_size: u16,
    ph_table_entry_count: u16,
    sh_table_entry_size: u16,
    sh_table_entry_count: u16,
    sh_table_names: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ELFProgramHeader {
    pub header_type: u32,
    pub flags: ELFPermissionFlags,
    pub offset: u64,
    pub virtual_address: u64,
    pub _undef: u64, // physical address, unused
    pub size_in_file: u64,
    pub size_in_memory: u64,
    pub alignment: u64,
}
impl ELFProgramHeader {
    pub fn loadable(&self) -> bool {
        self.header_type == 1
    }
    pub fn has_flag(&self, flag: ELFPermissionFlags) -> bool {
        let flags = self.flags;
        flags.contains(flag)
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ELFParsingError {
    NotElf,
    Not64Bit,
    WrongEndianness,
    WrongVersion,
    WrongAbi,
    WrongInstrcutionSet,
    WrongElfVersion,
    InvalidELF,
    FeatureSupportMissing,
    EmptyHeader,
}

pub unsafe fn parse_elf(data: &[u8]) -> Result<ELFData, ELFParsingError> {
    let elf_header: ELFHeader =
        ptr::read_unaligned(data[..mem::size_of::<ELFHeader>()].as_ptr().cast());

    if elf_header.magic != ELF_MAGIC {
        Err(ELFParsingError::NotElf)
    } else if elf_header.bitness != ELF_BITNESS_64 {
        Err(ELFParsingError::Not64Bit)
    } else if elf_header.ph_table_entry_size != ELF_PH_TABLE_ENTRY_SIZE {
        Err(ELFParsingError::InvalidELF)
    } else if elf_header.endianness != ELF_LITTLE_ENDIAN {
        Err(ELFParsingError::WrongEndianness)
    } else if elf_header.elf_version != CURRENT_ELF_VERSION
        || elf_header.elf_version_2 != CURRENT_ELF_VERSION as u32
    {
        Err(ELFParsingError::WrongVersion)
    } else if elf_header.instrucion_set != ELF_ARCH_X86_64 {
        Err(ELFParsingError::WrongInstrcutionSet)
    } else {
        let mut elf_data = ELFData {
            header: elf_header,
            ph_table: [None; MAX_PH_ENTRY_COUNT],
        };

        // get program headers
        let mut ph_table = 0;
        for index in 0..elf_data.header.ph_table_entry_count {
            let offset = (elf_data.header.ph_table_position as usize)
                + (elf_data.header.ph_table_entry_size as usize) * (index as usize);

            let ph: ELFProgramHeader = ptr::read_unaligned(
                data[offset..offset + mem::size_of::<ELFProgramHeader>()]
                    .as_ptr()
                    .cast(),
            );

            match ph.header_type as usize {
                1 => {
                    // load, (needed)
                    elf_data.ph_table[ph_table] = Some(ph);
                    ph_table += 1;
                },
                0x60000000 => {}, // OS Specific 0, decompression tables, (but unused here)
                _ => {},          // unknown, not supported
            }
        }

        Ok(elf_data)
    }
}

pub unsafe fn parse_kernel_elf() -> ELFData {
    let slice = core::slice::from_raw_parts(
        KERNEL_ELF_IMAGE_POSITION as *const u8,
        0x1_0000_0000, // Just some high-enough number
    );

    match parse_elf(slice) {
        Ok(header) => header,
        Err(error) => panic!("Could not receive kernel image data: {:?}", error),
    }
}
