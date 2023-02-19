//! Physical memory allocator

use core::alloc::{AllocError, Allocator as AllocatorTrait, Layout};
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use spin::Mutex;
use x86_64::PhysAddr;

use allogator::BuddyGroupAllocator;

use crate::memory::{constants, phys_to_virt};

use super::super::area::PhysMemoryRange;
use super::super::map::MAX_OK_ENTRIES;
use super::super::prelude::*;

use super::Allocation;

fn _to_allocation(physptr: NonNull<[u8]>, layout: Layout) -> Allocation {
    let start_raw = physptr.cast::<u8>().as_ptr() as u64;
    log::debug!("allocate {:x} {:?}", start_raw, layout);
    Allocation {
        start: PhysAddr::new(start_raw),
        layout,
    }
}

/// A marker type of OOM conditions
#[derive(Debug, Clone, Copy)]
pub struct OutOfMemory;

/// Physical memory allocator
static PHYS_ALLOCATOR: Mutex<MaybeUninit<BuddyGroupAllocator>> = Mutex::new(MaybeUninit::uninit());

/// # Safety
/// The caller must ensure that this is not intialized multiple times
pub unsafe fn init(areas: [Option<PhysMemoryRange>; MAX_OK_ENTRIES]) {
    log::debug!("Setting up heap allocator");

    let mut block_count = 0;
    // Safety: we are creating `MaybeUninit`s, they do not require initalization
    let mut blocks: [MaybeUninit<allogator::MemoryBlock>; MAX_OK_ENTRIES] =
        unsafe { MaybeUninit::uninit().assume_init() };

    for area in areas {
        if let Some(area) = area {
            blocks[block_count].write(allogator::MemoryBlock {
                // Here we use the static higher-half phys access, that
                // has to be take into account when returning allocated memory
                ptr: NonNull::new(phys_to_virt(area.start()).as_mut_ptr()).unwrap(),
                len: (area.size_bytes() as usize).next_power_of_two() >> 1,
            });
            block_count += 1;
        }
    }

    // let blocks = &mut blocks[..block_count];
    let blocks = &mut blocks[..1];
    // Safety: the data is initialized now
    let blocks: &mut [allogator::MemoryBlock] = unsafe { core::mem::transmute::<_, _>(blocks) };

    let inner = BuddyGroupAllocator::new(blocks, MIN_PAGE_SIZE_BYTES as usize);

    let mut a = PHYS_ALLOCATOR.try_lock().expect("Already locked");
    a.write(inner);
}

pub(super) fn undo_offset_ptr(p: *mut u8) -> *mut u8 {
    unsafe {
        (p as u64)
            .checked_sub(constants::HIGHER_HALF_START.as_u64())
            .expect("Inverse phys_to_virt not possible for this address") as *mut _
    }
}

fn undo_offset(nn: NonNull<[u8]>) -> NonNull<[u8]> {
    let (p, metadata) = nn.to_raw_parts();
    let inverted = unsafe { NonNull::new_unchecked(undo_offset_ptr(p.cast().as_ptr())).cast() };
    NonNull::from_raw_parts(inverted, metadata)
}

pub fn allocate(layout: Layout) -> Result<Allocation, OutOfMemory> {
    log::trace!("Allocate {:?}", layout);
    let guard = PHYS_ALLOCATOR.lock();
    let inner = unsafe { guard.assume_init_ref() };
    let ia = inner.allocate(layout).map_err(|_| OutOfMemory)?;
    log::trace!("Allocated at {:p} {:?}", ia, layout);
    Ok(_to_allocation(undo_offset(ia), layout))
}

pub fn allocate_zeroed(layout: Layout) -> Result<Allocation, OutOfMemory> {
    log::trace!("Allocate zeroed {:?}", layout);
    let guard = PHYS_ALLOCATOR.lock();
    let inner = unsafe { guard.assume_init_ref() };
    let ia = inner.allocate_zeroed(layout).map_err(|_| OutOfMemory)?;
    log::trace!("Allocated at {:p} {:?}", ia, layout);
    Ok(_to_allocation(undo_offset(ia), layout))
}

/// # Safety
/// Caller must make sure that the deallocation is only done once.
/// This takes &mut and should usually be used only in the Allocation destructor
pub(super) unsafe fn _deallocate(p: &mut Allocation) {
    log::trace!("Deallocate {:?}", p);
    let guard = PHYS_ALLOCATOR.lock();
    let inner = unsafe { guard.assume_init_ref() };
    unsafe {
        inner.deallocate(
            // Reapply offset
            NonNull::new_unchecked(phys_to_virt(p.phys_start()).as_mut_ptr()),
            p.layout,
        )
    }
}
