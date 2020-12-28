// TODO: Some kind of version problem

use super::rsdt;

#[derive(Debug)]
pub enum XSDPParseError {
    RSDPParseError(rsdt::RSDPParseError),
    IncorrectChecksum,
}

pub unsafe fn get_xsdp() -> Result<bool, XSDPParseError> {
    match rsdt::get_rsdp_and_parse() {
        Ok(result) => {
            todo!();
            Ok(true)
        },
        Err(e) => Err(XSDPParseError::RSDPParseError(e)),
    }
}
