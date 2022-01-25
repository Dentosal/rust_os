//! DMA / VirtIO memory buffers (requiring "low" memory)

use spin::Mutex;
use x86_64::structures::paging as pg;
use x86_64::{PhysAddr, VirtAddr};

use super::super::constants::{DMA_MEMORY_SIZE, DMA_MEMORY_START};

const DMA_BLOCK_SIZE: usize = 0x1000;
const DMA_BLOCKS: usize = round_up_block(DMA_MEMORY_SIZE as usize);

const fn round_up_block(s: usize) -> usize {
    (s + (DMA_BLOCK_SIZE - 1)) / DMA_BLOCK_SIZE
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BlockState {
    Free,
    Used,
}
pub struct Allocator {
    /// Blocks
    blocks: [BlockState; DMA_BLOCKS],
}

impl Allocator {
    /// Unsafe, as the caller is responsibe that this is not intialized multiple times
    unsafe fn new() -> Self {
        Self {
            blocks: [BlockState::Free; DMA_BLOCKS],
        }
    }

    pub fn allocate(&mut self, size: usize) -> DMARegion {
        assert!(size != 0);

        let size_blocks = round_up_block(size) as usize;

        if size_blocks > self.blocks.len() {
            panic!("Not enough of DMA memory");
        }

        'outer: for start in 0..(self.blocks.len() - size_blocks) {
            for offset in 0..size_blocks {
                if self.blocks[start + offset] != BlockState::Free {
                    continue 'outer;
                }
            }

            for offset in 0..size_blocks {
                self.blocks[start + offset] = BlockState::Used;
            }

            return DMARegion {
                start: DMA_MEMORY_START + start * DMA_BLOCK_SIZE,
                size_blocks,
            };
        }

        panic!("Out of DMA memory");
    }

    pub fn free(&mut self, _region: DMARegion) {
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DMARegion {
    pub start: PhysAddr,
    size_blocks: usize,
}

lazy_static::lazy_static! {
    pub static ref DMA_ALLOCATOR: Mutex<Allocator> = Mutex::new(unsafe{Allocator::new()});
}
