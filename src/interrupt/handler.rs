use core::arch::asm;
use core::sync::atomic::Ordering;
use x86_64::structures::idt::{InterruptStackFrame, InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::{PhysAddr, VirtAddr};

use crate::driver::pic;
use crate::multitasking::process::ProcessSwitchInfo;
use crate::multitasking::{
    process, Process, ProcessId, ProcessSwitch, SCHEDULER, SCHEDULER_ENABLED,
};
use crate::smp;
use crate::syscall::RawSyscall;

/// Breakpoint handler
pub(super) unsafe fn exception_bp(stack_frame: &InterruptStackFrame) {
    rforce_unlock!();
    log::warn!(
        "Breakpoint at {:?} (cpu {})\n{:?}",
        (*stack_frame).instruction_pointer,
        smp::current_processor_id(),
        *stack_frame
    );
    bochs_magic_bp!();
}

/// Invalid Opcode handler (instruction undefined)
pub(super) unsafe fn exception_ud(stack_frame: &InterruptStackFrame) {
    panic!(
        "Exception: invalid opcode at {:?} (cpu {})\n{:?}",
        (*stack_frame).instruction_pointer,
        smp::current_processor_id(),
        *stack_frame
    );
}

/// Double Fault handler
#[allow(unused_variables)]
pub(super) unsafe fn exception_df(stack_frame: &InterruptStackFrame, error_code: u64) {
    // error code is always zero
    panic_indicator!(0x4f664f64); // "df"
    rforce_unlock!();
    log::error!("Exception: Double Fault\n{:?}", *stack_frame);
    log::error!("exception stack frame at {:#p}", stack_frame);
    loop {}
}

/// General Protection Fault handler
pub(super) unsafe fn exception_gpf(stack_frame: &InterruptStackFrame, error_code: u64) {
    log::trace!(
        "Exception: General Protection Fault with error code {:#x}\n{:#?}",
        error_code,
        *stack_frame
    );
    panic!(
        "Exception: General Protection Fault with error code {:#x}\n{:#?}",
        error_code, *stack_frame
    );
}

/// Page Fault handler
pub(super) unsafe fn exception_pf(stack_frame: &InterruptStackFrame, error_code: u64) {
    panic!(
        "Exception: Page Fault with error code {:?} ({:?}) at {:#x} (cpu {})\n{:#?}",
        error_code,
        PageFaultErrorCode::from_bits(error_code).expect("#PF code invalid"),
        x86_64::registers::control::Cr2::read().as_u64(),
        smp::current_processor_id(),
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
        "Exception: Segment Not Present with error code {:#x} (e={:b},t={:?},i={:#x}) (cpu {})\n{:?}",
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
        smp::current_processor_id(),
        *stack_frame
    );
}

/// LAPIC TSC-deadline timer ticked
pub(super) unsafe extern "sysv64" fn exception_tsc_deadline(_: u64) -> u128 {
    // Interrupt timing
    crate::random::insert_entropy(0);

    log::trace!("Deadline");
    crate::driver::ioapic::lapic::write_eoi();

    if crate::smp::is_bsp() && SCHEDULER_ENABLED.load(Ordering::SeqCst) {
        let next_process = {
            let mut sched = SCHEDULER.try_lock().expect("SCHEDUELR LOCKED");
            let target = sched.tick();
            if let Some(deadline) = sched.next_tick() {
                crate::smp::sleep::set_deadline(deadline).expect("TODO: Deadline too soon");
            }
            target
        };

        match next_process {
            ProcessSwitch::Switch(p) => return_process(p),
            ProcessSwitch::RepeatSyscall(p) => {
                if let Some(rp) = handle_repeat_syscall(p) {
                    return_process(rp)
                } else {
                    0
                }
            },
            ProcessSwitch::Continue => 0,
            ProcessSwitch::Idle => 0,
        }
    } else {
        0
    }
}

/// PIT timer ticked while the kernel was running
pub(super) unsafe fn exception_irq0() {
    if !pic::is_enabled() {
        panic!("Stop! reached 0");
    }
    crate::driver::pit::callback();
    pic::PICS.try_lock().unwrap().notify_eoi(0x20);
}

/// First ps/2 device, keyboard, sent data.
/// Read the byte and then send it to the keyboard driver.
pub(super) unsafe fn exception_irq1() {
    if !pic::is_enabled() {
        log::debug!("KBDINPUT when pic is disabled");
    }
    let mut port_ps2_data = cpuio::UnsafePort::<u8>::new(0x60);
    let mut port_ps2_status = cpuio::UnsafePort::<u8>::new(0x64);

    // Wait until ready
    loop {
        if (port_ps2_status.read() & 0x1) != 0 {
            break;
        }
    }

    // Read byte
    let byte = port_ps2_data.read();

    // Send to driver
    let mut sched = SCHEDULER.try_lock().unwrap();
    crate::ipc::kernel_publish(&mut sched, "irq/keyboard", &byte);

    // Interrupt over
    pic::PICS.lock().notify_eoi(0x21);
}

/// First ATA device is ready for data transfer
pub(super) unsafe fn exception_irq14() {
    if !pic::is_enabled() {
        panic!("Stop! reached 14");
    }
    // Since we are polling the drive, just ignore the IRQ
    pic::PICS.lock().notify_eoi(0x2e);
}

/// (Possibly) spurious interrupt for the primary PIC
/// https://wiki.osdev.org/8259_PIC#Handling_Spurious_IRQs
pub(super) unsafe fn exception_irq7() {
    if !pic::is_enabled() {
        panic!("Stop! reached 7");
    }
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
    if !pic::is_enabled() {
        panic!("Stop! reached 15");
    }
    let mut pics = pic::PICS.try_lock().unwrap();
    // Check if this is a real IRQ
    let is_real = pics.read_isr() & (1 << 15) != 0;
    if is_real {
        pics.notify_eoi(0x2f);
    } else {
        // Inform primary PIC about spurious interrupt
        pics.notify_eoi_primary();
    }
}

pub(super) unsafe extern "sysv64" fn irq_dynamic(interrupt: u64) -> u128 {
    let interrupt = interrupt as u8;

    let next_process = {
        let mut sched = SCHEDULER.try_lock().unwrap();
        let irq = interrupt - 0x30;
        crate::ipc::kernel_publish(&mut sched, &format!("irq/{}", irq), &());
        crate::driver::ioapic::lapic::write_eoi();

        sched.switch_current_or_next()
    };

    log::trace!("irq_dynamic: Switching to {:?}", next_process);
    match next_process {
        ProcessSwitch::Continue | ProcessSwitch::Idle => 0,
        ProcessSwitch::Switch(process) => return_process(process),
        ProcessSwitch::RepeatSyscall(process) => {
            if let Some(inner_process) = handle_repeat_syscall(process) {
                return_process(inner_process)
            } else {
                0
            }
        },
    }
}

/// Some other core paniced, stopping the system
pub(super) unsafe fn ipi_panic(_: &InterruptStackFrame) {
    crate::driver::ioapic::lapic::write_eoi();
    log::error!("Kernel panic IPI received, halting core");
    loop {
        asm!("cli; hlt");
    }
}

/// Interrupt while a process tables were active
/// Called from `src/asm_misc/process_common.asm`, process_interrupt
/// Input registers:
/// * `rax` Interrupt vector number
/// * `rbx` Process stack pointer
///
/// Process registers `rax`, `rbx` and `rcx` are already stored in its stack.
#[naked]
pub(super) unsafe extern "C" fn process_interrupt() {
    asm!(
        "
        // Recreate interrupt stack frame from r10..=r14
        push r14
        push r13
        push r12
        push r11
        push r10

        // Call inner function
        mov rdi, rax // interrupt
        mov rsi, rbx // process_rsp
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
    ",
        options(noreturn)
    );
}

#[no_mangle]
unsafe extern "C" fn process_interrupt_inner(
    interrupt: u8, process_rsp: u64, page_table: u64,
    stack_frame_ptr: *const InterruptStackFrameValue, error_code: u32,
) -> u128 {
    use x86_64::registers::control::Cr2;

    let stack_frame: InterruptStackFrameValue = (*stack_frame_ptr).clone();
    let page_table = PhysAddr::new_unchecked(page_table);
    let process_rsp = VirtAddr::new_unsafe(process_rsp);

    let pid = {
        let mut sched = SCHEDULER.try_lock().unwrap();
        let pid = sched.get_running_pid().expect("No process running?");
        sched.store_state(pid, page_table, process_rsp);
        pid
    };

    // Interrupt timing and number
    crate::random::insert_entropy(interrupt as u64);

    macro_rules! handle_switch {
        ($next_process:expr) => {{
            log::trace!("Switching to {:?}", $next_process);
            match $next_process {
                ProcessSwitch::Continue => {},
                ProcessSwitch::Idle => {
                    idle();
                },
                ProcessSwitch::Switch(process) => {
                    return return_process(process);
                },
                ProcessSwitch::RepeatSyscall(process) => {
                    if let Some(inner_process) = handle_repeat_syscall(process) {
                        return return_process(inner_process);
                    }
                },
            }
        }};
    }

    if interrupt != 0xd7 && interrupt != 0xd8 && interrupt != 0x3e {
        log::debug!("Handling interrupt {:#02x} while in process", interrupt);
    }
    log::trace!("Interrupt {:#02x}", interrupt);

    match interrupt {
        0xd7 => {
            use crate::syscall::{handle_syscall, SyscallResultAction};
            match handle_syscall(pid) {
                SyscallResultAction::Terminate(status) => terminate(pid, status),
                SyscallResultAction::Continue => {},
                SyscallResultAction::Switch(schedule) => {
                    // get the next process
                    let next_process = {
                        let mut sched = SCHEDULER.try_lock().unwrap();
                        let target = sched.switch(Some(schedule));
                        if let Some(deadline) = sched.next_tick() {
                            crate::smp::sleep::set_deadline(deadline)
                                .expect("TODO: Deadline too soon");
                        }
                        target
                    };
                    handle_switch!(next_process);
                },
            }
        },
        0xd8 => {
            // TSC deadline

            // log::trace!("TSC_DEADLINE");
            crate::driver::ioapic::lapic::write_eoi();

            assert!(SCHEDULER_ENABLED.load(Ordering::SeqCst)); // TODO: remove
            if crate::smp::is_bsp() {
                let switch_target = {
                    let mut sched = SCHEDULER.try_lock().expect("SCHEDUELR LOCKED");
                    let target = sched.tick();
                    log::trace!("TSC_DEADLINE tick => {target:?}");
                    if let Some(deadline) = sched.next_tick() {
                        crate::smp::sleep::set_deadline(deadline).expect("TODO: Deadline too soon");
                    }
                    target
                };
                handle_switch!(switch_target);
            } else {
                handle_switch!(ProcessSwitch::Idle);
            }
        },
        0x20 => {
            // PIT timer ticked
            panic!("PIT ticked while in process");
        },
        0x21 => exception_irq1(),
        0x27 => exception_irq7(),
        0x22..=0x26 | 0x28 | 0x2c..=0x2d => {
            // pic::PICS.lock().notify_eoi(interrupt);
            panic!("Unhandled interrupt: {:02x}", interrupt);
        },
        0x2e => {
            // Handle (ignore) ata interrupts
            exception_irq14();
        },
        0x2f => {
            // Handle spurious interrupts
            exception_irq15();
        },
        0x30..=0x9f => {
            // Dynamic range
            let mut sched = SCHEDULER.try_lock().unwrap();
            let irq = interrupt - 0x30;
            crate::ipc::kernel_publish(&mut sched, &format!("irq/{}", irq), &());
            crate::driver::ioapic::lapic::write_eoi();
            handle_switch!(sched.switch_current_or_next());
        },
        0x00 => fail(pid, process::Error::DivideByZero(stack_frame)),
        0x0e => {
            // TODO:
            // * Error code, if any, must be removed from the stack before returning
            fail(
                pid,
                process::Error::PageFault(
                    stack_frame,
                    Cr2::read(),
                    PageFaultErrorCode::from_bits(error_code as u64)
                        .expect("Invalid page fault error code"),
                ),
            )
        },
        0x08 | 0x0a | 0x0b | 0x0c | 0x0d | 0x11 | 0x1e => fail(
            pid,
            process::Error::InterruptWithCode(interrupt, stack_frame, error_code),
        ),
        _ => fail(pid, process::Error::Interrupt(interrupt, stack_frame)),
    }

    // Continue current process
    process_pair_to_u128(process_rsp, page_table)
}

fn fail(pid: ProcessId, error: process::Error) -> ! {
    terminate(pid, process::ProcessResult::Failed(error))
}

/// Terminate the give process and switch to the next one
fn terminate(pid: ProcessId, result: process::ProcessResult) -> ! {
    let next_process = unsafe {
        let mut sched = SCHEDULER.try_lock().expect("Sched unlock");
        sched.terminate_and_switch(pid, result)
    };

    log::debug!(
        "Switching to {:?} after {} did terminate",
        next_process,
        pid
    );

    match next_process {
        ProcessSwitch::Continue => unreachable!(),
        ProcessSwitch::Idle => {
            idle();
        },
        ProcessSwitch::Switch(p) => immediate_switch_to(p),
        ProcessSwitch::RepeatSyscall(p) => {
            if let Some(rp) = unsafe { handle_repeat_syscall(p) } {
                immediate_switch_to(rp)
            } else {
                idle()
            }
        },
    }
}

fn idle() -> ! {
    use crate::memory::process_common_code::COMMON_ADDRESS_VIRT;
    log::trace!("Setting processor to idle state");

    // Jump into the idle state
    unsafe {
        asm!("
            mov rcx, [{cav} + 2 * 8]    // Offset: idle
            jmp rcx                     // Jump into the procedure
            ",
            cav = const COMMON_ADDRESS_VIRT,
            options(nostack, noreturn)
        );
        ::core::hint::unreachable_unchecked();
    }
}

/// Jump to next process immediately
fn immediate_switch_to(process: ProcessSwitchInfo) -> ! {
    use crate::memory::process_common_code::COMMON_ADDRESS_VIRT;

    unsafe {
        asm!("
            mov rcx, [rcx]  // Get procedure offset
            jmp rcx         // Jump into the procedure
            ",
            in("rcx") COMMON_ADDRESS_VIRT, // switch_to
            in("rdx") process.stack_pointer.as_u64(),
            in("rax") process.p4addr.as_u64(),
            options(nostack, noreturn)
        );
        ::core::hint::unreachable_unchecked();
    }
}

/// Returns process to switch to, if any.
/// On `None` the system should switch to idle state.
#[must_use]
unsafe fn handle_repeat_syscall(p: ProcessSwitchInfo) -> Option<ProcessSwitchInfo> {
    use crate::syscall::{handle_syscall, SyscallResult, SyscallResultAction};
    log::trace!("handle_repeat_syscall {p:?}");
    match handle_syscall(p.pid) {
        SyscallResultAction::Terminate(status) => terminate(p.pid, status),
        SyscallResultAction::Continue => Some(p),
        SyscallResultAction::Switch(schedule) => {
            let next_process = {
                SCHEDULER
                    .try_lock()
                    .expect("Scheduler locked")
                    .switch(Some(schedule))
            };
            match next_process {
                ProcessSwitch::Continue => None,
                ProcessSwitch::Idle => None,
                ProcessSwitch::Switch(inner_p) => Some(inner_p),
                ProcessSwitch::RepeatSyscall(inner_p) => {
                    assert!(p.pid != inner_p.pid, "handle_repeat_syscall loops");
                    handle_repeat_syscall(inner_p)
                },
            }
        },
    }
}

/// Constructs a u128 from two integers for returning
/// process stack pointer and page table.
/// When this integer is returned from `extern "C"` works like this:
///
/// `(u64, u64)` (encoded as u128) in `(rax, rdx)`, per System V ABI:
/// https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI
/// If process switch is required, then the returned pair of u64 should be
/// (p4_physical_address, stack_pointer)
/// If no switch should be done then function must return `(0, 0)`.
#[inline]
fn return_process(p: ProcessSwitchInfo) -> u128 {
    process_pair_to_u128(p.stack_pointer, p.p4addr)
}

fn process_pair_to_u128(stack_pointer: VirtAddr, page_table: PhysAddr) -> u128 {
    ((stack_pointer.as_u64() as u128) << 64) | (page_table.as_u64() as u128)
}
