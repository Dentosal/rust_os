//! Hardware drivers

#[macro_use]
pub mod vga_buffer;

pub mod acpi;
pub mod ioapic;
pub mod pic;
pub mod pit;
pub mod tsc;
pub mod uart;
