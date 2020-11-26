use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use libd7::{syscall, PhysAddr, VirtAddr};

static MAPPED: AtomicBool = AtomicBool::new(false);
static VIRTUAL_ADDR: VirtAddr = unsafe { VirtAddr::new_unsafe(0x10_0000_0000) }; // Should be free

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DMARegion {
    pub phys: PhysAddr,
    pub virt: VirtAddr,
}
impl DMARegion {
    pub fn allocate(size_bytes: usize) -> Self {
        let phys = syscall::dma_allocate(size_bytes as u64).unwrap();

        // Assumes that DMA block is on the first page.
        // Keep in sync with plan.md
        if !MAPPED.compare_and_swap(false, true, Ordering::SeqCst) {
            unsafe {
                syscall::mmap_physical(
                    PhysAddr::new(0),
                    VIRTUAL_ADDR,
                    size_bytes as u64,
                    syscall::MemoryProtectionFlags::READ | syscall::MemoryProtectionFlags::WRITE,
                )
                .unwrap();
            }
        }

        Self {
            phys,
            virt: VIRTUAL_ADDR + phys.as_u64(),
        }
    }
}
