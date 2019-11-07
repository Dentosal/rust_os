//! Constants memory-related things
//! These MUST be kept in sync with from src/asm_routines and plan.md

use x86_64::{PhysAddr, VirtAddr};

use super::prelude::*;

// Boot stage mmap
pub(super) const BOOT_TMP_MMAP_BUFFER: PhysAddr = unsafe { PhysAddr::new_unchecked(0x2000) };

// Boot stage page tables
pub(super) const BOOT_TMP_PAGE_TABLE_P4: PhysAddr = unsafe { PhysAddr::new_unchecked(0x60000) };

// Kernel position and size
pub const KERNEL_LOCATION: PhysAddr = unsafe { PhysAddr::new_unchecked(0x1_000_000) };
pub const KERNEL_SIZE_LIMIT: usize = 0x200_000; // TODO: find a solution, or document and test properly
pub const KERNEL_END: PhysAddr =
    unsafe { PhysAddr::new_unchecked(KERNEL_LOCATION.as_u64() + KERNEL_SIZE_LIMIT as u64) };

// Page table location
pub const PAGE_TABLES_LOCATION: PhysAddr = unsafe { PhysAddr::new_unchecked(0x10_000_000) };
pub const PAGE_TABLES_SIZE_LIMIT: usize = 0x1_000_000;
pub const PAGE_TABLES_END: PhysAddr = unsafe {
    PhysAddr::new_unchecked(PAGE_TABLES_LOCATION.as_u64() + PAGE_TABLES_SIZE_LIMIT as u64)
};

// Mark for allocators
pub const MEMORY_RESERVED_BELOW: PhysAddr = PAGE_TABLES_END;

// Kernel stack for system calls
pub const SYSCALL_STACK: VirtAddr = unsafe { VirtAddr::new_unchecked_raw(0x11_000_000) };

// Process virtual memory area
pub const PROCESS_COMMON_CODE: VirtAddr = unsafe { VirtAddr::new_unchecked_raw(0x200_000) };
pub const PROCESS_STACK: VirtAddr = unsafe { VirtAddr::new_unchecked_raw(0x400_000) };
pub const PROCESS_STACK_SIZE_PAGES: u64 = 2;
pub const PROCESS_STACK_SIZE_BYTES: u64 = PAGE_SIZE_BYTES * PROCESS_STACK_SIZE_PAGES;
pub const PROCESS_STACK_END: VirtAddr =
    unsafe { VirtAddr::new_unchecked_raw(PROCESS_STACK.as_u64() + PROCESS_STACK_SIZE_BYTES) };
