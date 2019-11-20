use alloc::vec::Vec;
use core::ptr;
use spin::Mutex;
use x86_64::structures::paging as pg;
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::structures::paging::{Mapper, PageTable};

mod allocators;
mod area;
mod constants;
mod map;
pub mod paging;
pub mod prelude;
mod utils;

use crate::multitasking::{ElfImage, Process};
use crate::util::elf_parser::{self, ELFData, ELFProgramHeader};

pub use self::allocators::*;
pub use self::prelude::*;

use self::paging::PageMap;

use d7alloc::{HEAP_SIZE, HEAP_START};

pub struct MemoryController {
    /// Kernel page table
    pub page_map: PageMap,
    /// Physical memory allocator
    pub frame_allocator: frame_allocator::Allocator,
    /// Kernel stack allocator
    stack_allocator: stack_allocator::StackAllocator,
    /// Virtual address space allocator
    virtual_allocator: virtual_allocator::VirtualAllocator,
}

impl MemoryController {
    pub fn alloc_stack(&mut self, size_in_pages: usize) -> Option<Stack> {
        self.stack_allocator.alloc_stack(
            &mut self.page_map,
            &mut self.frame_allocator,
            size_in_pages,
        )
    }

    /// Allocates a set of physical memory frames
    pub fn alloc_frames(&mut self, size_in_pages: usize) -> Vec<PhysFrame> {
        (0..size_in_pages)
            .map(|_| {
                self.frame_allocator
                    .allocate_frame()
                    .expect("Could not allocate frame")
            })
            .collect()
    }

    /// Allocates a set of physical memory frames filled with zeros.
    ///
    /// Flushes the TLB.
    pub fn alloc_frames_zeroed(&mut self, size_in_pages: usize) -> Vec<PhysFrame> {
        // Use a temporary virtual memory area
        let tmp_area = self.alloc_virtual_area(1);
        let tmp_page = Page::from_start_address(tmp_area.start).unwrap();

        let frames = self.alloc_frames(size_in_pages);
        for frame in frames.iter() {
            unsafe {
                self.page_map
                    .map_to(
                        PT_VADDR,
                        tmp_page,
                        frame.clone(),
                        Flags::PRESENT | Flags::NO_EXECUTE | Flags::WRITABLE,
                    )
                    .flush();

                let p: *mut u8 = tmp_area.start.as_mut_ptr();
                ptr::write_bytes(p, 0, frame.size() as usize);
            }
        }

        // Deallocate temporary area
        unsafe {
            self.page_map.unmap(PT_VADDR, tmp_page).flush();
        }
        self.free_virtual_area(tmp_area);

        frames
    }

    /// Allocates a contiguous virtual memory area
    pub fn alloc_virtual_area(&mut self, size_in_pages: u64) -> Area {
        let start = self.virtual_allocator.allocate(size_in_pages);
        Area::new_pages(start, size_in_pages)
    }

    /// Frees a virtual memory area
    pub fn free_virtual_area(&mut self, area: Area) {
        self.virtual_allocator.free(area.start, area.size_pages());
    }

    /// Allocate a contiguous virtual address block,
    /// and page-map it with the given flags.
    ///
    /// This function alwrays flushes the TLB.
    ///
    /// Requires that the kernel page tables are active.
    pub fn alloc_pages(&mut self, size_in_pages: usize, flags: Flags) -> Area {
        self.alloc_both(size_in_pages, flags).1
    }

    /// Allocate and page-map memory, and
    /// return both the physical frame and the virtual address block
    ///
    /// This function alwrays flushes the TLB.
    ///
    /// Requires that the kernel page tables are active.
    pub fn alloc_both(&mut self, size_in_pages: usize, flags: Flags) -> (Vec<PhysFrame>, Area) {
        let mut frames: Vec<PhysFrame> = self.alloc_frames(size_in_pages);

        let start = self.virtual_allocator.allocate(size_in_pages as u64);

        for (page_index, frame) in frames.iter().enumerate() {
            unsafe {
                self.page_map
                    .map_to(
                        PT_VADDR,
                        Page::from_start_address(start + (page_index as u64) * PAGE_SIZE_BYTES)
                            .unwrap(),
                        frame.clone(),
                        flags,
                    )
                    .flush();
            }
        }

        (frames, Area::new_pages(start, size_in_pages as u64))
    }

    /// Maps process page tables to (current) kernel tables.
    /// Returns None if the area is not mapped in the process tables.
    ///
    /// This function flushes the TLB multiple times,
    /// and the given area is mapped and flushed when returning.
    ///
    /// Requires that the kernel page tables are active.
    pub fn process_map_area(
        &mut self, process: &Process, area: Area, writable: bool,
    ) -> Option<Area> {
        let mut flags = Flags::PRESENT | Flags::NO_EXECUTE;
        if writable {
            flags |= Flags::WRITABLE;
        }

        unsafe {
            // Map process tables to kernel memory
            let tmp_area = self.alloc_virtual_area(1);
            let tmp_page = Page::from_start_address(tmp_area.start).unwrap();
            let pt_vaddr = tmp_area.start;
            self.page_map
                .map_table(PT_VADDR, tmp_page, &process.page_table, false)
                .flush();

            // Resolve the addresses
            let frame_addresses: Vec<PhysAddr> = area
                .pages()
                .map(|page| process.page_table.translate(pt_vaddr, page))
                .collect::<Option<_>>()?;

            // Unmap the process page table
            self.page_map.unmap(PT_VADDR, tmp_page).flush();
            self.free_virtual_area(tmp_area);

            // Map the are to kernel tables
            let result_area = self.alloc_virtual_area(area.size_pages());
            for (i, frame_start) in frame_addresses.into_iter().enumerate() {
                self.page_map
                    .map_to(
                        PT_VADDR,
                        Page::from_start_address(result_area.start + (i as u64) * PAGE_SIZE_BYTES)
                            .unwrap(),
                        PhysFrame::from_start_address(frame_start).unwrap(),
                        flags,
                    )
                    .flush()
            }

            Some(result_area)
        }
    }

    /// Allocate or free memory for a process.
    ///
    /// This function flushes the TLB multiple times.
    ///
    /// Requires that the kernel page tables are active.
    pub fn process_set_dynamic_memory(
        &mut self, process: &mut Process, new_size_bytes: u64,
    ) -> Option<u64> {
        let flags = Flags::PRESENT | Flags::NO_EXECUTE | Flags::WRITABLE;

        let old_frame_count = process.dynamic_memory_frames.len() as u64;
        let old_size_bytes = old_frame_count * PAGE_SIZE_BYTES;

        if old_size_bytes == new_size_bytes {
            return Some(new_size_bytes);
        }

        assert!(old_size_bytes == page_align_u64(old_size_bytes, false));
        let new_size_bytes = page_align_u64(new_size_bytes, true);

        // Map process tables to kernel memory
        let tmp_area = self.alloc_virtual_area(1);
        let tmp_page = Page::from_start_address(tmp_area.start).unwrap();
        let pt_vaddr = tmp_area.start;
        unsafe {
            self.page_map
                .map_table(PT_VADDR, tmp_page, &process.page_table, true)
                .flush();
        }

        if old_size_bytes < new_size_bytes {
            // Allocate more memory
            let add_bytes = new_size_bytes - old_size_bytes;
            let add_frames = add_bytes / PAGE_SIZE_BYTES;
            let new_frames = self.alloc_frames_zeroed(add_frames as usize);

            // Map to process memory space
            for (i, frame) in new_frames.iter().enumerate() {
                let page = Page::from_start_address(
                    PROCESS_DYNAMIC_MEMORY + (old_frame_count + (i as u64)) * PAGE_SIZE_BYTES,
                )
                .unwrap();

                unsafe {
                    process
                        .page_table
                        .map_to(tmp_area.start, page, frame.clone(), flags)
                        .ignore();
                }
            }

            // Store frame information into the Process struct
            process.dynamic_memory_frames.extend(new_frames);
        } else {
            // Deallocate memory
            unimplemented!("Process: deallocate memory");
        }

        // Unmap process tables
        unsafe {
            self.page_map.unmap(PT_VADDR, tmp_page).flush();
        }
        self.free_virtual_area(tmp_area);

        Some(new_size_bytes)
    }

    /// Loads a program from ELF ímage to physical memory.
    /// This function does not load the ELF to its p_vaddr, but
    /// rather returns a list of unmapped physical frames.
    ///
    /// This function internally uses TLB flushes.
    ///
    /// Requires that the kernel page tables are active.
    pub fn load_elf(&mut self, elf_image: ElfImage) -> Vec<(ELFProgramHeader, Vec<PhysFrame>)> {
        let elf = unsafe { elf_image.parse_elf() };

        let mut frames = Vec::new();
        for ph in elf.ph_table.iter().filter_map(|x| *x) {
            if ph.loadable() && ph.size_in_memory != 0 {
                // Reserve p_memsz memory and map them for writing
                let size_in_pages = page_align_u64(ph.size_in_memory, true) / PAGE_SIZE_BYTES;
                let page_frames = self.alloc_frames(size_in_pages as usize);
                let area = self.alloc_virtual_area(size_in_pages);

                // Map the page frames to the kernel page tables
                for (page_index, frame) in page_frames.iter().enumerate() {
                    unsafe {
                        self.page_map
                            .map_to(
                                PT_VADDR,
                                Page::from_start_address(
                                    area.start + (page_index as u64) * PAGE_SIZE_BYTES,
                                )
                                .unwrap(),
                                *frame,
                                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                            )
                            .flush();
                    }
                }

                unsafe {
                    // Clear the new frames bytes
                    // Full frames area cleared to prevent data leaks
                    ptr::write_bytes(area.start.as_mut_ptr::<u8>(), 0, area.size_bytes() as usize);

                    // Copy p_filesz bytes from p_offset to target
                    ptr::copy_nonoverlapping(
                        elf_image.as_ptr().offset(ph.offset as isize),
                        area.start.as_mut_ptr(),
                        ph.size_in_file as usize,
                    );
                }

                // Unmap
                for page_index in 0..size_in_pages {
                    unsafe {
                        self.page_map
                            .unmap(
                                PT_VADDR,
                                Page::from_start_address(
                                    area.start + (page_index as u64) * PAGE_SIZE_BYTES,
                                )
                                .unwrap(),
                            )
                            .flush();
                    }
                }

                // Free virtual memory area
                self.virtual_allocator.free(area.start, size_in_pages);

                // Append frames to the result
                frames.push((ph, page_frames));
            }
        }

        frames
    }
}

pub fn init() {
    // Receive raw kernel elf image data before it's overwritten
    let elf_metadata = unsafe { elf_parser::parse_kernel_elf() };

    // Receive memory map before it's overwritten
    let memory_map = map::load_memory_map();

    // initalize paging system
    unsafe {
        paging::enable_nxe();
        paging::enable_write_protection();
    }

    // Remap kernel and get page table
    let mut page_map = unsafe { paging::init(elf_metadata) };

    // Initialize frame allocator
    let mut frame_allocator = unsafe { self::frame_allocator::Allocator::new(memory_map) };

    // Identity map heap
    let heap_start_page = pg::Page::containing_address(VirtAddr::new(HEAP_START));
    let heap_end_page = pg::Page::containing_address(VirtAddr::new(HEAP_START + HEAP_SIZE - 1));
    for page in pg::Page::range_inclusive(heap_start_page, heap_end_page) {
        let frame = frame_allocator.allocate_frame().expect("Out of memory");
        unsafe {
            page_map
                .map_to(
                    PT_VADDR,
                    page,
                    frame,
                    Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                )
                .flush();
        }
    }

    let stack_allocator = {
        let stack_alloc_start = heap_end_page + 1;
        let stack_alloc_end = stack_alloc_start + 100;
        let stack_alloc_range = pg::Page::range_inclusive(stack_alloc_start, stack_alloc_end);
        stack_allocator::StackAllocator::new(stack_alloc_range)
    };

    let mem_ctrl = MemoryController {
        page_map,
        frame_allocator,
        stack_allocator,
        virtual_allocator: virtual_allocator::VirtualAllocator::new(),
    };

    let mut guard = MEM_CTRL_CONTAINER.lock();
    *guard = Some(mem_ctrl);
}

/// Late initialization, executed when disk drivers are available
pub fn init_late() {
    // Prepare a kernel stack for syscalls
    syscall_stack::init();

    // Load process switching code
    process_common_code::init();
}

/// Static memory controller
static MEM_CTRL_CONTAINER: Mutex<Option<MemoryController>> = Mutex::new(None);

pub fn configure<F, T>(mut f: F) -> T
where F: FnMut(&mut MemoryController) -> T {
    let mut guard = MEM_CTRL_CONTAINER.lock();
    if let Some(ref mut mem_ctrl) = *guard {
        f(mem_ctrl)
    } else {
        unreachable!("Memory controller missing");
    }
}
