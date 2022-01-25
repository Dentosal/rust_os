use alloc::string::String;
use core::mem::size_of;
use hashbrown::HashMap;

use crate::memory::{self, prelude::*};

pub mod fadt;
pub mod madt;
mod rsdt;

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct SDTHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

static LEGAZY_ACPI: spin::Once<bool> = spin::Once::new();
static RSDT_ENTRIES: spin::Once<HashMap<[u8; 4], PhysAddr>> = spin::Once::new();

pub fn init_rsdt() {
    match unsafe { rsdt::get_rsdp_and_parse() } {
        Ok(result) => {
            let header_ptr_u64 = match result {
                rsdt::ParseResult::RSDT(ptr_u32) => ptr_u32 as u64,
                rsdt::ParseResult::XSDT(ptr_u64) => ptr_u64,
            };
            let body_ptr_size = match result {
                rsdt::ParseResult::RSDT(_) => 4,
                rsdt::ParseResult::XSDT(_) => 8,
            };
            LEGAZY_ACPI.call_once(|| match result {
                rsdt::ParseResult::RSDT(_) => true,
                rsdt::ParseResult::XSDT(_) => false,
            });

            let vptr = memory::phys_to_virt(PhysAddr::new(header_ptr_u64));
            let root_header: SDTHeader = unsafe { *vptr.as_ptr() };

            assert_eq!(&root_header.signature, b"RSDT");

            let entry_count =
                (root_header.length as usize - size_of::<SDTHeader>()) / body_ptr_size;

            // Loop through body pointers
            let mut result = HashMap::new();
            for index in 0..entry_count {
                let vptr_ptr = memory::phys_to_virt(PhysAddr::new(
                    header_ptr_u64 + (size_of::<SDTHeader>() + index * body_ptr_size) as u64,
                ));

                unsafe {
                    let vptr = match body_ptr_size {
                        4 => {
                            let p: u32 = *vptr_ptr.as_ptr();
                            PhysAddr::new(p as u64)
                        },
                        8 => {
                            let p: u64 = *vptr_ptr.as_ptr();
                            PhysAddr::new(p)
                        },
                        _ => unreachable!(),
                    };
                    let header: SDTHeader = *memory::phys_to_virt(vptr).as_ptr();
                    log::trace!("RSDT {:?}", String::from_utf8_lossy(&header.signature));
                    result.insert(header.signature, vptr);
                };
            }
            RSDT_ENTRIES.call_once(|| result);
        },
        Err(error) => panic!("RSDT error {:?}", error),
    }
}

pub fn is_legacy_acpi() -> bool {
    *LEGAZY_ACPI.poll().expect("RSDT unintialized")
}

pub fn rsdt_get(key: &[u8; 4]) -> Option<PhysAddr> {
    RSDT_ENTRIES
        .poll()
        .expect("RSDT unintialized")
        .get(key)
        .cloned()
}
