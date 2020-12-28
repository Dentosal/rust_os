mod tables;

pub use self::tables::madt::ACPI_DATA;

pub fn init() {
    tables::init_rsdt();
    tables::madt::init();
    tables::fadt::init();
}
