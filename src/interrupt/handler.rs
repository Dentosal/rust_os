use x86_64::structures::idt::{InterruptStackFrame, InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::{PhysAddr, VirtAddr};

use crate::driver::keyboard;
use crate::driver::pic;
use crate::multitasking::{process, ProcessId, ProcessSwitch, SCHEDULER};
use crate::time;

/// Breakpoint handler
pub(super) unsafe fn exception_bp(stack_frame: &InterruptStackFrame) {
    rforce_unlock!();
    rprintln!(
        "Breakpoint at {:?}\n{:?}",
        (*stack_frame).instruction_pointer,
        *stack_frame
    );
    bochs_magic_bp!();
}

/// Invalid Opcode handler (instruction undefined)
pub(super) unsafe fn exception_ud(stack_frame: &InterruptStackFrame) {
    panic!(
        "Exception: invalid opcode at {:?}\n{:?}",
        (*stack_frame).instruction_pointer,
        *stack_frame
    );
}

/// Double Fault handler
#[allow(unused_variables)]
pub(super) unsafe fn exception_df(stack_frame: &InterruptStackFrame, error_code: u64) {
    // error code is always zero
    panic_indicator!(0x4f664f64); // "df"
    rforce_unlock!();
    rprintln!("Exception: Double Fault\n{:?}", *stack_frame);
    rprintln!("exception stack frame at {:#p}", stack_frame);
    loop {}
}

/// General Protection Fault handler
pub(super) unsafe fn exception_gpf(stack_frame: &InterruptStackFrame, error_code: u64) {
    panic!(
        "Exception: General Protection Fault with error code {:#x}\n{:#?}",
        error_code, *stack_frame
    );
}

/// Page Fault handler
pub(super) unsafe fn exception_pf(stack_frame: &InterruptStackFrame, error_code: u64) {
    panic!(
        "Exception: Page Fault with error code {:?} ({:?}) at {:#x}\n{:#?}",
        error_code,
        PageFaultErrorCode::from_bits(error_code).expect("#PF code invalid"),
        x86_64::registers::control::Cr2::read().as_u64(),
        *stack_frame
    );
}

#[derive(Debug)]
#[allow(dead_code)]
enum SegmentNotPresentTable {
    GDT,
    IDT,
    LDT,
}

/// Segment Not Present handler
pub(super) unsafe fn exception_snp(stack_frame: &InterruptStackFrame, error_code: u64) {
    panic!(
        "Exception: Segment Not Present with error code {:#x} (e={:b},t={:?},i={:#x})\n{:?}",
        error_code,
        error_code & 0b1,
        match (error_code & 0b110) >> 1 {
            0b00 => SegmentNotPresentTable::GDT,
            0b01 => SegmentNotPresentTable::IDT,
            0b10 => SegmentNotPresentTable::LDT,
            0b11 => SegmentNotPresentTable::IDT,
            _ => {
                unreachable!();
            },
        },
        (error_code & 0xFFFF) >> 3,
        *stack_frame
    );
}

/// PIT timer ticked while the kernel was running
/// This occurs in two cases:
/// 1. Before scheduler has taken over (early)
/// 2. When the kernel is in idle state (no process is running)
/// In both cases idle and continue are equivalent
pub(super) unsafe extern "C" fn exception_irq0() -> u128 {
    let next_process = time::SYSCLOCK.tick();
    pic::PICS.lock().notify_eoi(0x20);
    match next_process {
        ProcessSwitch::Switch(process) => {
            ((process.stack_pointer.as_u64() as u128) << 64)
                | (process.page_table.p4_addr().as_u64() as u128)
        },
        ProcessSwitch::Continue => 0,
        ProcessSwitch::Idle => 0,
    }
}

/// First ps/2 device, keyboard, sent data
pub(super) unsafe fn exception_irq1() {
    rforce_unlock!();
    keyboard::KEYBOARD.force_unlock();
    let mut kbd = keyboard::KEYBOARD.lock();
    if kbd.is_enabled() {
        kbd.notify();
    }
    pic::PICS.lock().notify_eoi(0x21);
}

/// First ATA device is ready for data transfer
pub(super) unsafe fn exception_irq14() {
    // Since we are polling the drive, just ignore the IRQ
    pic::PICS.lock().notify_eoi(0x2e);
}

/// (Possibly) spurious interrupt for the primary PIC
/// https://wiki.osdev.org/8259_PIC#Handling_Spurious_IRQs
pub(super) unsafe fn exception_irq7() {
    let mut pics = pic::PICS.lock();
    // Check if this is a real IRQ
    let is_real = pics.read_isr() & (1 << 7) != 0;
    if is_real {
        pic::PICS.lock().notify_eoi(0x27);
    }
    // Ignore spurious interrupts
}

/// (Possibly) spurious interrupt for the secondary PIC
/// https://wiki.osdev.org/8259_PIC#Handling_Spurious_IRQs
pub(super) unsafe fn exception_irq15() {
    let mut pics = pic::PICS.lock();
    // Check if this is a real IRQ
    let is_real = pics.read_isr() & (1 << 15) != 0;
    if is_real {
        pic::PICS.lock().notify_eoi(0x2f);
    } else {
        // Inform primary PIC about spurious interrupt
        pic::PICS.lock().notify_eoi_primary();
    }
}

/// Interrupt from a process
/// Called from `src/asm_misc/process_common.asm`, process_interrupt
/// Input registers:
/// * `rax` Interrupt vector number
/// * `rbx` Process stack pointer
///
/// Process registers `rax`, `rbx` and `rcx` are already stored in its stack.
#[naked]
pub(super) unsafe fn process_interrupt() {
    asm!("
        // Recreate interrupt stack frame from r10..=r14
        push r14
        push r13
        push r12
        push r11
        push r10

        // Call inner function
        mov rdi, rax // interrupt
        mov rsi, rbx // process_stack
        mov rdx, rbp // page_table
        mov rcx, rsp // stack_frame_ptr
        mov r8 , r15 // error_code
        call process_interrupt_inner

        // Remove interrupt stack
        add rsp, 5 * 8

        // Set values for returning to process
        mov rbp, rax
        mov rbx, rdx

        // Return to trampoline
        ret
    " :::: "volatile", "intel");
}

#[no_mangle]
unsafe extern "C" fn process_interrupt_inner(
    interrupt: u8, process_stack: u64, page_table: u64,
    stack_frame_ptr: *const InterruptStackFrameValue, error_code: u32,
) -> u128
{
    use x86_64::registers::control::Cr2;

    let pid = SCHEDULER
        .try_lock()
        .unwrap()
        .get_running_pid()
        .expect("No process running?");

    let stack_frame: InterruptStackFrameValue = (*stack_frame_ptr).clone();
    let page_table = PhysAddr::new_unchecked(page_table);
    let process_stack = VirtAddr::new_unchecked(process_stack);

    match interrupt {
        0xd7 => {
            use crate::syscall::{handle_syscall, SyscallResult, SyscallResultAction};
            match handle_syscall(pid, page_table, process_stack, stack_frame) {
                SyscallResultAction::Terminate(status) => terminate(status),
                SyscallResultAction::Continue => {},
                SyscallResultAction::Switch(schedule) => {
                    // get the next process
                    let next_process = {
                        let mut sched = SCHEDULER.try_lock().unwrap();
                        sched.switch(Some(schedule))
                    };
                    match next_process {
                        ProcessSwitch::Switch(process) => {
                            // Store old process details
                            {
                                let mut sched = SCHEDULER.try_lock().unwrap();
                                sched.store_state(pid, page_table, process_stack);
                            }
                            // Switch to other process after returning
                            return ((process.stack_pointer.as_u64() as u128) << 64)
                                | (process.page_table.p4_addr().as_u64() as u128);
                        },
                        ProcessSwitch::Continue => {},
                        ProcessSwitch::Idle => {
                            // Store old process details
                            {
                                let mut sched = SCHEDULER.try_lock().unwrap();
                                sched.store_state(pid, page_table, process_stack);
                            }
                            // Switch to idle state
                            idle();
                        },
                    }
                },
            }
        },
        0x20 => {
            // PIT timer ticked
            let next_process = time::SYSCLOCK.tick();
            pic::PICS.lock().notify_eoi(interrupt);
            match next_process {
                ProcessSwitch::Continue => {},
                ProcessSwitch::Idle => {
                    SCHEDULER
                        .try_lock()
                        .unwrap()
                        .store_state(pid, page_table, process_stack);
                    idle();
                },
                ProcessSwitch::Switch(process) => {
                    // Store old process details
                    SCHEDULER
                        .try_lock()
                        .unwrap()
                        .store_state(pid, page_table, process_stack);
                    // Switch to other process after returning
                    return ((process.stack_pointer.as_u64() as u128) << 64)
                        | (process.page_table.p4_addr().as_u64() as u128);
                },
            }
        },
        0x21..=0x2f => {
            // TODO: Handle keyboard input
            // TODO: Handle (ignore) ata interrupts
            // TODO: Handle spurious interrupts
            // pic::PICS.lock().notify_eoi(interrupt);
            panic!("Unhandled interrupt: {}", interrupt);
        },
        0x00 => fail(process::Error::DivideByZero(stack_frame)),
        0x0e => {
            // TODO:
            // * Error code, if any, must be removed from the stack before returning
            fail(process::Error::PageFault(
                stack_frame,
                Cr2::read(),
                PageFaultErrorCode::from_bits(error_code as u64)
                    .expect("Invalid page fault error code"),
            ))
        },
        0x08 | 0x0a | 0x0b | 0x0c | 0x0d | 0x11 | 0x1e => fail(process::Error::InterruptWithCode(
            interrupt,
            stack_frame,
            error_code,
        )),
        _ => fail(process::Error::Interrupt(interrupt, stack_frame)),
    }

    // Continue current process
    ((process_stack.as_u64() as u128) << 64) | (page_table.as_u64() as u128)
}

fn fail(error: process::Error) -> ! {
    terminate(process::ProcessResult::Failed(error))
}

fn terminate(result: process::ProcessResult) -> ! {
    use crate::memory::process_common_code::COMMON_ADDRESS_VIRT;

    // Terminate current process
    let next_process = unsafe {
        SCHEDULER
            .try_lock()
            .expect("Sched unlock")
            .terminate_current(result)
    };

    match next_process {
        ProcessSwitch::Continue => unreachable!(),
        ProcessSwitch::Idle => {
            idle();
        },
        ProcessSwitch::Switch(process) => {
            // Jump to next process immediately
            unsafe {
                asm!("
                        mov rcx, [rcx]  // Get procedure offset
                        jmp rcx         // Jump into the procedure
                        "
                    :
                    :
                        "{rcx}"(COMMON_ADDRESS_VIRT as *const u8 as u64), // switch_to
                        "{rdx}"(process.stack_pointer.as_u64()),
                        "{rax}"(process.page_table.p4_addr().as_u64())
                    :
                    : "intel"
                );
                ::core::hint::unreachable_unchecked();
            }
        },
    }
}

fn idle() -> ! {
    use crate::memory::process_common_code::COMMON_ADDRESS_VIRT;

    // Jump into the idle state
    unsafe {
        asm!("
            mov rcx, [rcx + 2 * 8]  // Offset: idle
            jmp rcx                 // Jump into the procedure
            "
            :: "{rcx}"((COMMON_ADDRESS_VIRT) as *const u8 as u64)
            :: "intel"
        );
        ::core::hint::unreachable_unchecked();
    }
}
