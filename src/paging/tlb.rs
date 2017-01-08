/// Invalidate the given address in the TLB
pub unsafe fn flush(addr: usize) {
    asm!("invlpg ($0)" :: "r" (addr) : "memory");
}

/// Invalidate the TLB completely
pub unsafe fn flush_all() {
    register!(cr3, register!(cr3));
}
