use core::marker::PhantomData;
use core::{mem, ptr};

use crate::memory::PAGE_LAYOUT;

use super::Allocation;

const OVERHEAD: usize =
    mem::size_of::<*mut u8>() + mem::size_of::<Allocation>() + mem::size_of::<usize>();

/// An unordered list that owns allocation objects, useful as a
/// building block for allocator. The set is stored as an unrolled
/// linked list, which stores the bookkeeping metadata into the
/// allocation entries.
pub struct AllocationSet<T> {
    head: Option<ptr::NonNull<u8>>,
    type_: PhantomData<T>,
}

unsafe impl<T: Send> Send for AllocationSet<T> {}

impl<T> AllocationSet<T> {
    pub const EMPTY: Self = Self {
        head: None,
        type_: PhantomData,
    };

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    fn push_allocation(&mut self, block: Allocation) {
        debug_assert!(block.size() > OVERHEAD);

        unsafe {
            let this = ptr::NonNull::new_unchecked(block.mapped_start().as_mut_ptr());
            let prev = self.head.replace(this);

            // Link pointer
            let p: *mut Option<ptr::NonNull<u8>> = this.as_ptr().cast();
            ptr::write(p, prev);

            // Allocation itself
            // TODO: really only the layout has to be stored, as we already have the pointer
            let p: *mut Allocation = p.add(1).cast();
            ptr::write(p, block);

            // Item count on this page
            let p: *mut usize = p.add(1).cast();
            ptr::write(p, 0);
        }
    }

    // Returns error if this is full, along with the item that couldn't be pushed
    fn try_push(&mut self, value: T) -> Result<(), T> {
        let mut cursor = self.head;
        while let Some(item) = cursor {
            // Read next entry link
            let p: *mut Option<ptr::NonNull<u8>> = item.as_ptr().cast();
            cursor = unsafe { ptr::read(p) };

            // Read allocation info
            let p: *mut Allocation = unsafe { p.add(1).cast() };
            let block = unsafe { &*p };
            let size = block.size();

            // Read item count info
            let p: *mut usize = unsafe { p.add(1).cast() };
            let used_items = unsafe { &mut *p };

            // Calculate entry capacity
            let capacity_bytes = size.checked_sub(OVERHEAD).unwrap();
            let capacity = capacity_bytes / mem::size_of::<T>();

            // Check if full
            assert!(*used_items <= capacity);
            if *used_items == capacity {
                // Full, check next entry for space
                continue;
            }

            // We have space, write the new item
            let payload: *mut T = unsafe { p.add(1).cast() };
            let next_free_slot = unsafe { payload.add(*used_items) };
            unsafe {
                ptr::write(next_free_slot, value);
            }
            *used_items += 1;
            return Ok(());
        }

        Err(value)
    }

    pub fn push(&mut self, value: T) {
        if let Err(value) = self.try_push(value) {
            // Full, allocate more space
            // TODO: pick best allocation size
            self.push_allocation(super::allocate(PAGE_LAYOUT).expect("Out of memory"));
            if self.try_push(value).is_err() {
                unreachable!("Push failed after allocating more");
            }
        }
    }

    pub fn iterate_mut_first<F, R>(&self, mut f: F) -> Option<R>
    where F: FnMut(&mut T) -> Option<R> {
        let mut cursor = self.head;
        while let Some(item) = cursor {
            // Read next entry link
            let p: *mut Option<ptr::NonNull<u8>> = item.as_ptr().cast();
            let next_entry = unsafe { ptr::read(p) };
            debug_assert_ne!(cursor, next_entry);
            cursor = next_entry;

            // Skip over allocation info
            let p: *mut Allocation = unsafe { p.add(1).cast() };

            // Read item count info
            let p: *mut usize = unsafe { p.add(1).cast() };
            let used_items = unsafe { *p };

            // Iterate
            let payload: *mut T = unsafe { p.add(1).cast() };
            for i in 0..used_items {
                let item = unsafe { &mut *payload.add(i) };
                let result = f(item);
                if result.is_some() {
                    return result;
                }
            }
        }

        None
    }
}
