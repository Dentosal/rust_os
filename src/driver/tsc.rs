//! Intel guarantees that TSC will not overflow within 10 years of last
//! CPU reset (or counter reset).

use core::arch::asm;

/// Reset TSC to zero.
/// This should be used between deadlines if TSC value is near overflow.
/// # Warning
/// Do not reset the counter if a deadline is currently being used.
#[inline]
pub fn reset() {
    unsafe {
        asm!("wrmsr",
            in("ecx") 0x10, // TSC MSR
            in("edx") 0u32,
            in("eax") 0u32,
            options(nostack, nomem)
        )
    }
}

/// Read TSC value, serializing
#[inline]
pub fn read() -> u64 {
    let rdx: u64;
    let rax: u64;
    unsafe {
        asm!(
            "rdtscp", // Serializing read
            out("rdx") rdx,
            out("rax") rax,
            out("rcx") _, // processor id
            options(nomem, nostack)
        )
    }

    (rdx << 32) | (rax & 0xffff_ffff)
}

/// Sets deadline
#[inline]
pub fn set_deadline(deadline: u64) {
    // log::trace!("Set deadline {} (current {})", deadline, read());
    unsafe {
        asm!("wrmsr",
            in("ecx") 0x6e0,
            in("edx") (deadline >> 32) as u32,
            in("eax") deadline as u32,
            options(nostack, nomem)
        )
    }
}

/// Cancels deadline and disarms timer
#[inline]
pub fn clear_deadline() {
    unsafe {
        asm!("wrmsr",
            in("ecx") 0x6e0,
            in("edx") 0u32,
            in("eax") 0u32,
            options(nostack, nomem)
        )
    }
}
