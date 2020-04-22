use bitflags::bitflags;

bitflags! {
    pub struct MemoryProtectionFlags: u8 {
        const READ      = (1 << 0);
        const WRITE     = (1 << 1);
        const EXECUTE   = (1 << 2);
    }
}
