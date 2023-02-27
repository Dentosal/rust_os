use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::intrinsics::copy_nonoverlapping;
use core::ptr;
use d7abi::{MemoryProtectionFlags, SyscallErrorCode};
use x86_64::structures::idt::{InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{align_down, align_up, PhysAddr, VirtAddr};

pub use d7abi::process::{Error, ProcessId, ProcessResult};

use crate::memory::paging::{PageMap, PAGE_MAP};
use crate::memory::phys::OutOfMemory;
use crate::memory::process_common_code as pcc;
use crate::memory::{phys, virt};
use crate::memory::{phys_to_virt, prelude::*};
use crate::memory::{PROCESS_COMMON_CODE, PROCESS_STACK};
use crate::util::elf_parser::{self, ELFHeader, ELFProgramHeader};

use super::super::ElfImage;
use super::{Process, ProcessMetadata, Status};

/// Creates a new process
/// This function:
/// * Creates a stack for the new process, and populates it for returning to the process
/// * Creates a page table for the new process, and populates it with required kernel data
/// * Loads executable from an ELF image
/// Requires that the kernel page table is active.
/// Returns ProcessId and PageMap for the process.
pub unsafe fn create_process(
    pid: ProcessId, args: &[String], elf: ElfImage,
) -> Result<Process, OutOfMemory> {
    // Allocate a stack for the process
    let stack_size_bytes = (PROCESS_STACK_SIZE_PAGES * PAGE_SIZE_BYTES) as usize;
    let mut stack = phys::allocate_zeroed(
        Layout::from_size_align(stack_size_bytes, PAGE_SIZE_BYTES as usize).unwrap(),
    )?;

    // Calculate offsets
    // Offset to leave registers zero when they are popped,
    // plus space for the return address and other iretq data
    let args_size_in_memory: usize = 8
        + 8 * args.len()
        + args
            .iter()
            .map(|a| a.len())
            .sum::<usize>()
            .next_multiple_of(8);
    let registers_popped: usize = 15; // process_common.asm : push_all
    let inthandler_tmpvar = 1;
    let iretq_structure = 5;
    let stack_items_fixed = registers_popped + inthandler_tmpvar + iretq_structure;
    let process_stack_end = PROCESS_STACK + stack_size_bytes - args_size_in_memory;
    let process_init_rsp = process_stack_end - (stack_items_fixed * 8);

    log::trace!("init rsp {:p}", process_init_rsp);

    assert!(
        args_size_in_memory + stack_items_fixed <= stack_size_bytes,
        "Attempting to have too large argv"
    );

    // Populate the process stack
    {
        let mut top: usize = stack_size_bytes;
        let stack_mem = stack.write();

        macro_rules! push_u64 {
            ($val:expr) => {
                top -= 8;
                let v: u64 = $val;
                stack_mem[top..top + 8].copy_from_slice(&v.to_ne_bytes());
            };
        }

        macro_rules! align_u64 {
            () => {
                while top % 8 != 0 {
                    push_u8!(0);
                }
            };
        }

        macro_rules! push_u8 {
            ($val:expr) => {
                top -= 1;
                stack_mem[top] = $val;
            };
        }

        // Write process arguments into it's stack
        push_u64!(args.len() as u64);
        for arg in args {
            push_u64!(arg.len() as u64);
        }

        for arg in args.iter().rev() {
            for byte in arg.as_bytes().iter().rev() {
                push_u8!(*byte);
            }
        }

        align_u64!();

        // Write fixed iretq structure
        // https://os.phil-opp.com/returning-from-exceptions/#returning-from-exceptions

        // SS
        push_u64!(0);
        // RSP
        push_u64!(process_stack_end.as_u64());
        // RFLAGFS: Interrupt flag on (https://en.wikipedia.org/wiki/FLAGS_register#FLAGS)
        push_u64!(0x0202);
        // CS
        push_u64!(0x8u64);
        // RIP
        push_u64!(elf.header.program_entry_pos);
    }

    // TODO: do processes need larger-than-one-page page tables?
    // Allocate own page table for the process
    let pt_frame = phys::allocate(PAGE_LAYOUT)?;

    // Populate the page table of the process
    let pm_addr = pt_frame.mapped_start();
    let mut pm = unsafe {
        PageMap::init(
            pm_addr,
            pt_frame.phys_start(),
            pm_addr, // TODO: is this correct?
        )
    };
    core::mem::forget(pt_frame); // Ownership moved to the page map

    // Map the required kernel structures into the process tables
    unsafe {
        // Descriptor tables
        pm.map_to(
            pm_addr,
            Page::from_start_address(VirtAddr::new_unsafe(0x0)).unwrap(),
            PhysFrame::from_start_address(PhysAddr::new(pcc::PROCESS_IDT_PHYS_ADDR)).unwrap(),
            // FIXME: GDT.ACCESSED flag should mean that CPU will not attempt to write to this?
            Flags::PRESENT | Flags::NO_EXECUTE | Flags::WRITABLE,
        )
        .ignore();

        // Common section for process switches
        pm.map_to(
            pm_addr,
            Page::from_start_address(PROCESS_COMMON_CODE).unwrap(),
            PhysFrame::from_start_address(PhysAddr::new(pcc::COMMON_ADDRESS_PHYS)).unwrap(),
            Flags::PRESENT,
        )
        .ignore();

        // TODO: Rest of the structures? Are there any?
    }

    // Map process stack its own page table
    // No guard page is needed, as the page below the stack is read-only
    for i in 0..PROCESS_STACK_SIZE_PAGES {
        unsafe {
            pm.map_to(
                pm_addr,
                Page::from_start_address(PROCESS_STACK + i * PAGE_SIZE_BYTES).unwrap(),
                PhysFrame::from_start_address(stack.phys_start() + i * PAGE_SIZE_BYTES).unwrap(),
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            )
            .ignore();
        }
    }

    // Map the executable image to its own page table
    for (ph, frames) in &elf.sections {
        assert!(ph.virtual_address >= 0x400_000);
        let start = VirtAddr::new(ph.virtual_address);

        let mut flags = Flags::PRESENT;
        if !ph.has_flag(elf_parser::ELFPermissionFlags::EXECUTABLE) {
            flags |= Flags::NO_EXECUTE;
        }
        if !ph.has_flag(elf_parser::ELFPermissionFlags::READABLE) {
            panic!("Non-readable pages are not supported (yet?)");
        }
        if ph.has_flag(elf_parser::ELFPermissionFlags::WRITABLE) {
            flags |= Flags::WRITABLE;
        }

        for (i, frame) in frames.into_iter().enumerate() {
            // TODO: assumes that frames are page-sized
            let page = Page::from_start_address(start + PAGE_SIZE_BYTES * (i as u64)).unwrap();
            unsafe {
                pm.map_to(
                    pm_addr,
                    page,
                    PhysFrame::from_start_address(frame.phys_start()).unwrap(),
                    flags,
                )
                .ignore();
            }
        }
    }

    Ok(Process {
        page_table: pm,
        stack_pointer: process_init_rsp,
        stack_memory: stack,
        dynamic_memory: Vec::new(),
        repeat_syscall: false,
        _elf_image: elf,
        metadata: ProcessMetadata {
            id: pid,
            status: Status::Running,
        },
    })
}
