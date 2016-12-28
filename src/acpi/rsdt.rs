//! This module is only used to retrieve XSDT address from RSDT.
//! More info: http://wiki.osdev.org/RSDP

use core::mem::size_of;

const RSDP_SIGNATURE: &'static [u8; 8] = b"RSD PTR ";

#[repr(C,packed)]
struct RSDPDescriptor {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32
}

#[repr(C,packed)]
struct RSDPDescriptor20 {
    first_part: RSDPDescriptor,
    length: u32,
    xsdt_address: u64,
    extendedchecksum: u8,
    reserved: [u8; 3]
}

#[derive(Debug)]
pub enum RSDPParseError {
    NotFound,
    UnsupportedVersion,
    IncorrectChecksum
}

/// Addtional info: http://wiki.osdev.org/RSDP#Validating_the_RSDP
pub unsafe fn get_rsdp_and_parse() -> Result<u64, RSDPParseError> {
    // TODO: Check hard-coded points before looping, at least: 0x000fa6a0 (bochs)

    // Scan EBDA
    // TODO: use actual value from BDA (must be saved just after boot) instead of 0x9fc00
    let ebda_start  = 0x9fc00; // HACK
    let ebda_end    = ebda_start+0x10*0x64;
    for p in (ebda_start..ebda_end).step_by(0x10) {
        // Is this an ACPI structure (quick signature check)
        rprintln!("F1");
        if &*(p as *const [u8; 8]) == RSDP_SIGNATURE {
            return parse_rsdp(p);
        }
    }

    // Scan from 0x000E0000 to 0x000FFFFF
    let area_start  = 0xe0000;
    let area_end    = 0xfffff;
    for p in (area_start..area_end).step_by(0x10) {
        rprintln!("F2");
        // Is this an ACPI structure (quick signature check)
        if &*(p as *const [u8; 8]) == RSDP_SIGNATURE {
            return parse_rsdp(p);
        }
    }

    // We didn't find it. Fortunately for us (this function), the caller will handle this.
    Err(RSDPParseError::NotFound)
}

/// Verifying checksums: http://wiki.osdev.org/RSDP#Checksum_validation
unsafe fn parse_rsdp(p: usize) -> Result<u64, RSDPParseError> {
    // Version detection and first checksum field
    let ref basic_rsdpd: RSDPDescriptor = *(p as *const _);
    // let basic_rsdpd_bytes: [u8; size_of::<RSDPDescriptor>()] = *(p as *const _);
    let basic_rsdpd_bytes: [u8; 20] = *(p as *const _); // XXX: size_of is not const-expr, counted bytes by hand

    // Revision:  0 = "ACPI 1.0", 2 = "ACPI 2.0+" (http://wiki.osdev.org/RSDP#Detecting_ACPI_Version)
    rprintln!("revision: {:}", basic_rsdpd.revision);
    if basic_rsdpd.revision != 2 {
        return Err(RSDPParseError::UnsupportedVersion);
    }
    // Checksum
    let csum_1: u32 = basic_rsdpd_bytes.iter().fold(0,|a, &b| a + b) as u32;
    if (csum_1 & 0xFF) != 0 {
        return Err(RSDPParseError::IncorrectChecksum);
    }

    // Full structure, find XSDT and check second checksum
    let ref rsdpd: RSDPDescriptor20 = *(p as *const _);
    // let rsdpd_new_bytes: [u8; size_of::<RSDPDescriptor>()] = *(p as *const _);
    let rsdpd_new_bytes: [u8; 64] = *(p as *const _); // XXX: size_of is not const-expr, counted bytes by hand
    let csum_2: u32 = rsdpd_new_bytes.iter().fold(0,|a, &b| a + b) as u32;
    if (csum_2 & 0xFF) != 0 {
        return Err(RSDPParseError::IncorrectChecksum);
    }

    Ok(rsdpd.xsdt_address)
}
