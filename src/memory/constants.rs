//! Constants memory-related things
//! These MUST be kept in sync with from src/asm_routines and plan.md

use x86_64::PhysAddr;

// Boot stage mmap
pub(super) const BOOT_TMP_MMAP_BUFFER: PhysAddr = unsafe { PhysAddr::new_unchecked(0x2000) };

// Boot stage page tables
pub(super) const BOOT_TMP_PAGE_TABLE_P4: PhysAddr = unsafe { PhysAddr::new_unchecked(0x60000) };

// Kernel position and size
pub const KERNEL_LOCATION: PhysAddr = unsafe { PhysAddr::new_unchecked(0x100_000) };
pub const KERNEL_SIZE_LIMIT: usize = 0x200_000; // TODO: find a solution, or document and test properly
pub const KERNEL_END: PhysAddr =
    unsafe { PhysAddr::new_unchecked(KERNEL_LOCATION.as_u64() + KERNEL_SIZE_LIMIT as u64) };
pub const MEMORY_RESERVED_BELOW: PhysAddr = KERNEL_END;
