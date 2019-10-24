/// Hardware drivers

#[macro_use]
pub mod vga_buffer;

pub mod acpi;
pub mod pic;
// pub mod apic;
pub mod disk_io;
pub mod keyboard;
// pub mod nic;
pub mod pci;
pub mod pit;
pub mod virtio;
// pub mod ide;
