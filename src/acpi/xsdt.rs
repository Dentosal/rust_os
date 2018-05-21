// TODO: Some kind of version problem


use acpi::rsdt;

#[repr(C,packed)]
struct ACPISDTHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[derive(Debug)]
pub enum XSDPParseError {
    RSDPParseError(rsdt::RSDPParseError),
    IncorrectChecksum
}

pub unsafe fn get_xsdp() -> Result<bool, XSDPParseError> {
    match rsdt::get_rsdp_and_parse() {
        Ok(addr) => {
            rprintln!("OK {:#x}", addr);
            // TODO: checksum
            Ok(true)
        }
        Err(e) => {Err(XSDPParseError::RSDPParseError(e))}
    }
}
