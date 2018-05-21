/// Invalidate the given address in the TLB
#[inline]
pub unsafe fn flush(addr: usize) {
    asm!("invlpg ($0)" :: "r" (addr) : "memory");
}

/// Invalidate the TLB completely
#[inline]
pub unsafe fn flush_all() {
    register!(cr3, register!(cr3));
}
