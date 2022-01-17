use alloc::vec::Vec;
use core::alloc::Layout;
use core::mem;
use core::ptr;
use core::slice;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use x86_64::structures::paging as pg;
use x86_64::structures::paging::Mapper;
use x86_64::structures::paging::PageTableFlags as Flags;

mod allocators;
mod area;
mod map;
mod utils;

pub mod constants;
pub mod paging;
pub mod prelude;

pub mod phys;
pub mod rust_heap;
pub mod virt;

use crate::multitasking::{ElfImage, Process};
use crate::util::elf_parser::{self, ELFData, ELFHeader, ELFProgramHeader};

pub use self::allocators::*;
pub use self::prelude::*;

use self::paging::PageMap;

use d7alloc::{HEAP_SIZE, HEAP_START};

/// Flag to determine if allocations are available
static ALLOCATOR_READY: AtomicBool = AtomicBool::new(false);

/// Checks if allocations are available
pub fn can_allocate() -> bool {
    ALLOCATOR_READY.load(Ordering::Acquire)
}

// impl MemoryController {
//     /// Allocates a contiguous virtual memory area
//     pub fn alloc_virtual_area(&mut self, size_in_pages: u64) -> Area {
//         let start = self.virtual_allocator.allocate(size_in_pages);
//         Area::new_pages(start, size_in_pages)
//     }

//     /// Frees a virtual memory area
//     pub fn free_virtual_area(&mut self, area: Area) {
//         self.virtual_allocator.free(area.start, area.size_pages());
//     }

//     /// Maps list of physical frames to given virtual memory area.
//     ///
//     /// This function flushes the TLB.
//     ///
//     /// Requires that the kernel page tables are active.
//     pub unsafe fn map_area(&mut self, area: Area, frames: &[PhysFrame], writable: bool) {
//         let mut flags = Flags::PRESENT | Flags::NO_EXECUTE;
//         if writable {
//             flags |= Flags::WRITABLE;
//         }

//         for (i, frame) in frames.iter().enumerate() {
//             self.page_map
//                 .map_to(
//                     PT_VADDR,
//                     Page::from_start_address(area.start + (i as u64) * PAGE_SIZE_BYTES).unwrap(),
//                     frame.clone(),
//                     flags,
//                 )
//                 .flush();
//         }
//     }

//     /// Unmaps a virtual memory area.
//     ///
//     /// This function flushes the TLB.
//     ///
//     /// Requires that the kernel page tables are active.
//     pub unsafe fn unmap_area(&mut self, area: Area) {
//         for page in area.page_starts() {
//             self.page_map
//                 .unmap(PT_VADDR, Page::from_start_address(page).unwrap())
//                 .flush();
//         }
//     }

//     /// Uses process page tables to map area from the process memory
//     /// space to the kernel page tables.
//     ///
//     /// This function flushes the TLB multiple times,
//     /// and the given area is mapped and flushed when returning.
//     ///
//     /// Requires that the kernel page tables are active.
//     #[must_use]
//     pub unsafe fn process_map_area(
//         &mut self, process: &Process, area: Area, writable: bool,
//     ) -> Option<Area> {
//         // Map process tables to kernel memory
//         let tmp_area = self.alloc_virtual_area(1);
//         let tmp_page = Page::from_start_address(tmp_area.start).unwrap();
//         let pt_vaddr = tmp_area.start;
//         self.page_map
//             .map_table(PT_VADDR, tmp_page, &process.page_table, false)
//             .flush();

//         // Resolve the addresses
//         let frame_addrs: Vec<PhysAddr> = area
//             .page_starts()
//             .map(|page| process.page_table.translate(pt_vaddr, page))
//             .collect::<Option<_>>()?;

//         let frames: Vec<PhysFrame> = frame_addrs
//             .into_iter()
//             .map(|start| PhysFrame::from_start_address(start).unwrap())
//             .collect();

//         // Unmap the process page table
//         self.page_map.unmap(PT_VADDR, tmp_page).flush();
//         self.free_virtual_area(tmp_area);

//         // Map the are to kernel tables
//         let result_area = self.alloc_virtual_area(area.size_pages());
//         self.map_area(result_area, &frames, writable);
//         Some(result_area)
//     }

//     /// Make a range of bytes from process memory space immutably available.
//     /// The returned `Area` should be freed when the data is no longer needed.
//     #[must_use]
//     pub unsafe fn process_slice<'a>(
//         &mut self, process: &Process, count: u64, ptr: VirtAddr,
//     ) -> Option<(Area, &'a [u8])> {
//         let area = Area::new_containing_block(ptr, count);
//         let offset: u64 = ptr.as_u64() - area.start.as_u64();
//         let pmap = self.process_map_area(process, area, false)?;
//         let slice: &[u8] =
//             unsafe { slice::from_raw_parts((pmap.start + offset).as_ptr(), count as usize) };
//         Some((pmap, slice))
//     }

//     /// Make a range of bytes from process memory space mutably available.
//     /// The returned `Area` should be freed when the data is no longer needed.
//     #[must_use]
//     pub unsafe fn process_slice_mut<'a>(
//         &mut self, process: &Process, count: u64, ptr: VirtAddr,
//     ) -> Option<(Area, &'a mut [u8])> {
//         let area = Area::new_containing_block(ptr, count);
//         let offset: u64 = ptr.as_u64() - area.start.as_u64();
//         let pmap = self.process_map_area(process, area, true)?;
//         let slice: &mut [u8] = unsafe {
//             slice::from_raw_parts_mut((pmap.start + offset).as_mut_ptr(), count as usize)
//         };
//         Some((pmap, slice))
//     }

//     /// Write bytes into the process memory space
//     pub unsafe fn process_write_bytes(
//         &mut self, process: &Process, bytes: &[u8], ptr: VirtAddr,
//     ) -> Option<()> {
//         let area = Area::new_containing_block(ptr, bytes.len() as u64);
//         let offset: u64 = ptr.as_u64() - area.start.as_u64();
//         let pmap = self.process_map_area(process, area, true)?;
//         ptr::copy_nonoverlapping(
//             bytes.as_ptr(),
//             (pmap.start + offset).as_mut_ptr(),
//             bytes.len(),
//         );
//         self.unmap_area(pmap);
//         self.free_virtual_area(area);
//         Some(())
//     }

//     /// Write a value into the process memory space
//     pub unsafe fn process_write_value<T>(
//         &mut self, process: &Process, value: T, ptr: VirtAddr,
//     ) -> Option<()> {
//         self.process_write_bytes(
//             process,
//             core::slice::from_raw_parts((&value) as *const T as *const u8, mem::size_of::<T>()),
//             ptr,
//         )
//     }

//     /// Allocate or free memory for a process.
//     ///
//     /// This function flushes the TLB multiple times.
//     ///
//     /// Requires that the kernel page tables are active.
//     pub fn process_set_dynamic_memory(
//         &mut self, process: &mut Process, new_size_bytes: u64,
//     ) -> Option<u64> {
//         todo!();
//         // let flags = Flags::PRESENT | Flags::NO_EXECUTE | Flags::WRITABLE;

//         // let old_frame_count = process.dynamic_memory_frames.len() as u64;
//         // let old_size_bytes = old_frame_count * PAGE_SIZE_BYTES;

//         // if old_size_bytes == new_size_bytes {
//         //     return Some(new_size_bytes);
//         // }

//         // assert!(old_size_bytes == page_align_u64(old_size_bytes, false));
//         // let new_size_bytes = page_align_u64(new_size_bytes, true);

//         // // Map process tables to kernel memory
//         // let tmp_area = self.alloc_virtual_area(1);
//         // let tmp_page = Page::from_start_address(tmp_area.start).unwrap();
//         // let pt_vaddr = tmp_area.start;
//         // unsafe {
//         //     self.page_map
//         //         .map_table(PT_VADDR, tmp_page, &process.page_table, true)
//         //         .flush();
//         // }

//         // if old_size_bytes < new_size_bytes {
//         //     // Allocate more memory
//         //     let add_bytes = new_size_bytes - old_size_bytes;
//         //     let add_frames = add_bytes / PAGE_SIZE_BYTES;
//         //     let new_frames = self.alloc_frames_zeroed(add_frames as usize);

//         //     // Map to process memory space
//         //     for (i, frame) in new_frames.iter().enumerate() {
//         //         let page = Page::from_start_address(
//         //             PROCESS_DYNAMIC_MEMORY + (old_frame_count + (i as u64)) * PAGE_SIZE_BYTES,
//         //         )
//         //         .unwrap();

//         //         unsafe {
//         //             process
//         //                 .page_table
//         //                 .map_to(tmp_area.start, page, frame.clone(), flags)
//         //                 .ignore();
//         //         }
//         //     }

//         //     // Store frame information into the Process struct
//         //     process.dynamic_memory_frames.extend(new_frames);
//         // } else {
//         //     // Deallocate memory
//         //     unimplemented!("Process: deallocate memory");
//         // }

//         // // Unmap process tables
//         // unsafe {
//         //     self.page_map.unmap(PT_VADDR, tmp_page).flush();
//         // }
//         // self.free_virtual_area(tmp_area);

//         // Some(new_size_bytes)
//     }

//     /// Allocate or free memory for a process.
//     ///
//     /// This function flushes the TLB multiple times.
//     ///
//     /// Requires that the kernel page tables are active.
//     pub fn process_map_physical(
//         &mut self, process: &mut Process, new_size_bytes: u64,
//     ) -> Option<u64> {
//         todo!();
//         // let flags = Flags::PRESENT | Flags::NO_EXECUTE | Flags::WRITABLE;

//         // let old_frame_count = process.dynamic_memory_frames.len() as u64;
//         // let old_size_bytes = old_frame_count * PAGE_SIZE_BYTES;

//         // if old_size_bytes == new_size_bytes {
//         //     return Some(new_size_bytes);
//         // }

//         // assert!(old_size_bytes == page_align_u64(old_size_bytes, false));
//         // let new_size_bytes = page_align_u64(new_size_bytes, true);

//         // // Map process tables to kernel memory
//         // let tmp_area = self.alloc_virtual_area(1);
//         // let tmp_page = Page::from_start_address(tmp_area.start).unwrap();
//         // let pt_vaddr = tmp_area.start;
//         // unsafe {
//         //     self.page_map
//         //         .map_table(PT_VADDR, tmp_page, &process.page_table, true)
//         //         .flush();
//         // }

//         // if old_size_bytes < new_size_bytes {
//         //     // Allocate more memory
//         //     let add_bytes = new_size_bytes - old_size_bytes;
//         //     let add_frames = add_bytes / PAGE_SIZE_BYTES;
//         //     let new_frames = self.alloc_frames_zeroed(add_frames as usize);

//         //     // Map to process memory space
//         //     for (i, frame) in new_frames.iter().enumerate() {
//         //         let page = Page::from_start_address(
//         //             PROCESS_DYNAMIC_MEMORY + (old_frame_count + (i as u64)) * PAGE_SIZE_BYTES,
//         //         )
//         //         .unwrap();

//         //         unsafe {
//         //             process
//         //                 .page_table
//         //                 .map_to(tmp_area.start, page, frame.clone(), flags)
//         //                 .ignore();
//         //         }
//         //     }

//         //     // Store frame information into the Process struct
//         //     process.dynamic_memory_frames.extend(new_frames);
//         // } else {
//         //     // Deallocate memory
//         //     unimplemented!("Process: deallocate memory");
//         // }

//         // // Unmap process tables
//         // unsafe {
//         //     self.page_map.unmap(PT_VADDR, tmp_page).flush();
//         // }
//         // self.free_virtual_area(tmp_area);

//         // Some(new_size_bytes)
//     }

// }

/// # Safety: the caller must ensure that this is called only once
pub unsafe fn init() {
    // Receive raw kernel elf image data before it's overwritten
    let elf_metadata = unsafe { elf_parser::parse_kernel_elf() };

    // Receive memory map before it's overwritten
    let memory_info = map::load_memory_map();

    // initalize paging system
    unsafe {
        paging::enable_nxe();
        paging::enable_write_protection();
    }

    // Remap kernel and get page table
    unsafe { paging::init(elf_metadata) };

    // Map all physical memory to the higher half
    let a = PhysFrame::from_start_address(PhysAddr::zero()).unwrap();
    let b = PhysFrame::containing_address(PhysAddr::new(memory_info.max_memory));
    {
        let mut page_map = paging::PAGE_MAP.try_lock().unwrap();
        for frame in PhysFrame::range_inclusive(a, b) {
            let f_u64 = frame.start_address().as_u64();
            let page = Page::from_start_address(VirtAddr::new(
                constants::HIGHER_HALF_START.as_u64() + f_u64,
            ))
            .unwrap();

            unsafe {
                page_map
                    .map_to(
                        PT_VADDR,
                        page,
                        frame,
                        Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                    )
                    .flush(); // TODO: flush all at once
            }
        }
    }

    // Initialize frame allocator
    self::phys::init(memory_info.allocatable);

    log::trace!("A");

    // InitRD
    crate::initrd::init(elf_metadata);

    log::trace!("B");

    // Prepare a kernel stack for syscalls
    syscall_stack::init();

    log::trace!("C");

    // Load process switching code
    process_common_code::init();

    log::trace!("D");

    // Set allocations available
    ALLOCATOR_READY.store(true, Ordering::Release);
    log::debug!("Allocation is now available");
}

/// Convert PhysAddr to VirtAddr, only accessible in by the kernel,
/// i.e. requires kernel page tables to be active.
///
/// This uses the constant always-available higher-half physical memory mapping.
pub fn phys_to_virt(offset: PhysAddr) -> VirtAddr {
    VirtAddr::new(
        constants::HIGHER_HALF_START
            .as_u64()
            .checked_add(offset.as_u64())
            .expect("phys_to_virt overflow"),
    )
}
