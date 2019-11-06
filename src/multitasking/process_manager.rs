use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr;
use spin::Mutex;
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::VirtAddr;

use crate::memory;
use crate::memory::paging::PageMap;
use crate::memory::prelude::*;
use crate::memory::process_common_code as pcc;
use crate::memory::{PROCESS_COMMON_CODE, PROCESS_STACK};
use crate::util::elf_parser::{self, ELFData};

use super::loader::ElfImage;
use super::process::Process;
use super::ProcessId;

const PROCESS_STACK_SIZE_PAGES: u64 = 2;

pub struct State {
    process_list: Vec<Process>,
    id_counter: ProcessId,
}

impl State {
    pub const unsafe fn new() -> Self {
        Self {
            process_list: Vec::new(),
            id_counter: ProcessId(0),
        }
    }

    /// Returns process by index for context switch
    pub fn get_at(&mut self, index: usize) -> Option<&mut Process> {
        self.process_list.get_mut(index)
    }

    /// Returns process count
    pub fn process_count(&self) -> usize {
        self.process_list.len()
    }

    /// Returns process ids
    pub fn process_ids(&self) -> Vec<ProcessId> {
        self.process_list.iter().map(|p| p.id()).collect()
    }

    /// Creates a new process.
    /// This function:
    /// * Creates a stack for the new process, and populates it for returning to the process
    /// * Creates a page table for the new process, and populates it with required kernel data
    /// * Loads executable from an ELF image
    /// Requires that the kernel page table is active.
    /// Returns ProcessId and PageMap for the process.
    unsafe fn create_process(&mut self, parent: Option<ProcessId>, elf: ElfImage) -> ProcessId {
        let pids = self.process_ids();

        // Infinite loop is not possible, since we will never
        // have 2**32 * 1000 bytes = 4.3 terabytes of memory for the process list only
        while pids.contains(&self.id_counter) {
            self.id_counter = self.id_counter.next();
        }

        let mut rsp: VirtAddr = VirtAddr::new_unchecked(0x12345678);
        let mut page_map: MaybeUninit<PageMap> = MaybeUninit::uninit();

        memory::configure(|mm| {
            // Allocate a stack for the process
            let stack_frames = mm.alloc_frames(PROCESS_STACK_SIZE_PAGES as usize);
            let stack_area = mm.alloc_virtual_area(PROCESS_STACK_SIZE_PAGES);

            // Populate the process stack
            for (page_index, frame) in stack_frames.iter().enumerate() {
                let vaddr = stack_area.start + (page_index as u64) * PAGE_SIZE_BYTES;
                unsafe {
                    // Map the actual stack frames to the kernel page tables
                    mm.page_map
                        .map_to(
                            PT_VADDR,
                            Page::from_start_address(vaddr).unwrap(),
                            frame.clone(),
                            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                        )
                        .flush();

                    // Zero the stack
                    ptr::write_bytes(vaddr.as_mut_ptr::<u8>(), 0, frame.size() as usize);

                    // Set start address to the right position, on the last page
                    if page_index == (PROCESS_STACK_SIZE_PAGES as usize) - 1 {
                        // Offset to leave registers zero when they are popped,
                        // plus space for the return address itself
                        ptr::write(
                            vaddr
                                .as_mut_ptr::<u64>()
                                .offset((PAGE_SIZE_BYTES / 8 - 1) as isize),
                            0x1_000_000u64,
                        );
                    }

                    // Unmap from kernel table
                    mm.page_map
                        .unmap(PT_VADDR, Page::from_start_address(vaddr).unwrap())
                        .flush();
                }
            }

            // Allocate own page table for the process
            let pt_frame = mm.alloc_frames(1)[0];

            // Mapping in the kernel space
            let pt_area = mm.alloc_virtual_area(1);

            // Map table to kernel space
            unsafe {
                mm.page_map
                    .map_to(
                        PT_VADDR,
                        Page::from_start_address(pt_area.start).unwrap(),
                        pt_frame,
                        Flags::PRESENT | Flags::WRITABLE,
                    )
                    .flush();
            }

            // Populate the page table of the process
            let mut pm =
                unsafe { PageMap::init(pt_area.start, pt_frame.start_address(), pt_area.start) };

            // Map the required kernel structures into the process tables
            unsafe {
                // Descriptor tables
                pm.map_to(
                    pt_area.start,
                    Page::from_start_address(VirtAddr::new_unchecked(0x0)).unwrap(),
                    PhysFrame::from_start_address(PhysAddr::new(pcc::PROCESS_IDT_PHYS_ADDR)).unwrap(),
                    // Flags::PRESENT | Flags::NO_EXECUTE,
                    // CPU likes to write to GDT(?) for some reason?
                    Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                )
                .ignore();

                // Common section for process switches
                pm.map_to(
                    pt_area.start,
                    Page::from_start_address(PROCESS_COMMON_CODE).unwrap(),
                    PhysFrame::from_start_address(PhysAddr::new(pcc::COMMON_ADDRESS_PHYS)).unwrap(),
                    Flags::PRESENT,
                )
                .ignore();

                // TODO: Rest of the structures? Are there any?
            }

            // Map process stack its own page table
            // No guard page is needed, as the page below the stack is read-only
            for (page_index, frame) in stack_frames.into_iter().enumerate() {
                let vaddr = PROCESS_STACK + (page_index as u64) * PAGE_SIZE_BYTES;
                unsafe {
                    pm.map_to(
                        pt_area.start,
                        Page::from_start_address(vaddr).unwrap(),
                        frame,
                        Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                    )
                    .ignore();
                }
            }

            // Set rsp
            rsp = PROCESS_STACK + (PROCESS_STACK_SIZE_PAGES * PAGE_SIZE_BYTES - 8 * (16 + 1));

            // Map the executable image to its own page table

            let elf_frames = unsafe { mm.load_elf(elf) };
            for (ph, frames) in elf_frames {
                assert!(ph.virtual_address >= 0x400_000);
                let start = VirtAddr::new(ph.virtual_address);

                let mut flags = Flags::PRESENT;
                if !ph.has_flag(elf_parser::ELFPermissionFlags::EXECUTABLE) {
                    flags |= Flags::NO_EXECUTE;
                }
                if !ph.has_flag(elf_parser::ELFPermissionFlags::READABLE) {
                    panic!("Non-readable pages are not supported (yet)");
                }
                if ph.has_flag(elf_parser::ELFPermissionFlags::WRITABLE) {
                    flags |= Flags::WRITABLE;
                }

                for (i, frame) in frames.into_iter().enumerate() {
                    let page =
                        Page::from_start_address(start + PAGE_SIZE_BYTES * (i as u64)).unwrap();
                    unsafe {
                        pm.map_to(pt_area.start, page, frame, flags).ignore();
                    }
                }
            }

            // TODO: Unmap process structures from kernel page map
            // ^ at least the process page table is not unmapped yet

            page_map.write(pm);
        });

        let pm = unsafe { page_map.assume_init() };

        let process = Process::new(self.id_counter, parent, pm.p4_addr(), rsp);
        let pid = process.id();

        self.process_list.push(process);
        self.id_counter = self.id_counter.next();
        pid
    }

    /// Creates a new process without a parent process
    pub fn spawn(&mut self, elf_image: ElfImage) -> ProcessId {
        unsafe { self.create_process(None, elf_image) }
    }

    /// Forks existing process, and returns the id of the created child processes
    // pub fn fork(&mut self, target: ProcessId) -> ProcessId {
    //     self.create_process(Some(target))
    // }

    /// Kills process, and returns whether the process existed at all
    pub fn kill(&mut self, target: ProcessId, status_code: u64) -> bool {
        match self.process_list.iter().position(|p| p.id() == target) {
            Some(index) => {
                // TODO: Send return code to subscribed processes
                self.process_list.swap_remove(index);
                true
            }
            None => false,
        }
    }
}

/// Wrapper for State
pub struct ProcessManager(UnsafeCell<Mutex<State>>);
unsafe impl Sync for ProcessManager {}
impl ProcessManager {
    pub const unsafe fn new() -> Self {
        Self(UnsafeCell::new(Mutex::new(State::new())))
    }

    pub fn try_fetch<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&State) -> T,
    {
        if let Some(ref state) = unsafe { (*self.0.get()).try_lock() } {
            Some(f(state))
        } else {
            None
        }
    }

    pub fn try_update<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&mut State) -> T,
    {
        if let Some(ref mut state) = unsafe { (*self.0.get()).try_lock() } {
            Some(f(state))
        } else {
            None
        }
    }

    pub fn fetch<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&State) -> T,
    {
        self.try_fetch(f).expect("Unable to lock process manager")
    }

    pub fn update<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut State) -> T,
    {
        self.try_update(f).expect("Unable to lock process manager")
    }
}
