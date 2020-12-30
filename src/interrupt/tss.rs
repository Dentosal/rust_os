use core::mem::size_of;
use core::sync::atomic::{AtomicU8, Ordering};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

use crate::memory::constants::TSS_ADDR;

static USED_TSS: AtomicU8 = AtomicU8::new(0);

/// Adds to an array of immutable GDTs, one for each processor core
pub fn store(tss: TaskStateSegment) -> &'static TaskStateSegment {
    let index = USED_TSS.fetch_add(1, Ordering::SeqCst);
    let new_base = TSS_ADDR.as_u64() + (index as u64) * (size_of::<TaskStateSegment>()) as u64;
    let ptr = new_base as *mut TaskStateSegment;
    unsafe {
        ptr.write(tss);
        &*ptr
    }
}
