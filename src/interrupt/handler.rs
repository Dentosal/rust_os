use crate::driver::keyboard;
use crate::driver::pic;
use crate::time;

use super::ExceptionStackFrame;

/// Breakpoint handler
pub(super) unsafe fn exception_bp(stack_frame: &ExceptionStackFrame) {
    rforce_unlock!();
    rprintln!(
        "Breakpoint at {:#x}\n{}",
        (*stack_frame).instruction_pointer,
        *stack_frame
    );
    bochs_magic_bp!();
}

/// Invalid Opcode handler (instruction undefined)
pub(super) unsafe fn exception_ud(stack_frame: &ExceptionStackFrame) {
    rforce_unlock!();
    rprintln!(
        "Exception: invalid opcode at {:#x}\n{}",
        (*stack_frame).instruction_pointer,
        *stack_frame
    );
    loop {}
}

/// Double Fault handler
#[allow(unused_variables)]
pub(super) unsafe fn exception_df(stack_frame: &ExceptionStackFrame, error_code: u64) {
    // error code is always zero
    panic_indicator!(0x4f664f64); // "df"
    rforce_unlock!();
    rprintln!("Exception: Double Fault\n{}", *stack_frame);
    rprintln!("exception stack frame at {:#p}", stack_frame);
    loop {}
}

/// General Protection Fault handler
pub(super) unsafe fn exception_gpf(stack_frame: &ExceptionStackFrame, error_code: u64) {
    rforce_unlock!();
    rprintln!(
        "Exception: General Protection Fault with error code {:#x}\n{}",
        error_code,
        *stack_frame
    );
    loop {}
}

bitflags! {
    /// Page Fault error codes
    struct PageFaultErrorCode: u64 {
        const PROTECTION_VIOLATION  = 1 << 0;
        const CAUSED_BY_WRITE       = 1 << 1;
        const USER_MODE             = 1 << 2;
        const MALFORMED_TABLE       = 1 << 3;
        const INSTRUCTION_FETCH     = 1 << 4;
    }
}

/// Page Fault handler
pub(super) unsafe fn exception_pf(stack_frame: &ExceptionStackFrame, error_code: u64) {
    rforce_unlock!();
    rprintln!(
        "Exception: Page Fault with error code {:?} ({:?}) at {:#x}\n{}",
        error_code,
        PageFaultErrorCode::from_bits(error_code).unwrap(),
        x86_64::registers::control::Cr2::read().as_u64(),
        *stack_frame
    );
    loop {}
}

#[derive(Debug)]
#[allow(dead_code)]
enum SegmentNotPresentTable {
    GDT,
    IDT,
    LDT,
}

/// Segment Not Present handler
pub(super) unsafe fn exception_snp(stack_frame: &ExceptionStackFrame, error_code: u64) {
    rforce_unlock!();
    rprintln!(
        "Exception: Segment Not Present with error code {:#x} (e={:b},t={:?},i={:#x})\n{}",
        error_code,
        error_code & 0b1,
        match (error_code & 0b110) >> 1 {
            0b00 => SegmentNotPresentTable::GDT,
            0b01 => SegmentNotPresentTable::IDT,
            0b10 => SegmentNotPresentTable::LDT,
            0b11 => SegmentNotPresentTable::IDT,
            _ => {
                unreachable!();
            }
        },
        (error_code & 0xFFFF) >> 3,
        *stack_frame
    );
    loop {}
}

/// PIT timer ticked
pub(super) unsafe extern "C" fn exception_irq0() -> u128 {
    let next_process = time::SYSCLOCK.tick();
    pic::PICS.lock().notify_eoi(0x20);
    if let Some(process) = next_process {
        rprintln!(
            "NEXT {:x} {:x}",
            process.page_table.as_u64(),
            process.stack_pointer.as_u64()
        );
        ((process.stack_pointer.as_u64() as u128) << 64) | (process.page_table.as_u64() as u128)
    } else {
        0
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
        // Save scratch registers, except rax and rcx
        push rdx
        push rsi
        push rdi
        push r8
        push r9
        push r10
        push r11

        // Call inner function
        mov rdi, rax
        mov rsi, rbx
        call process_interrupt_inner
    " :::: "volatile", "intel");
}

#[no_mangle]
unsafe extern "C" fn process_interrupt_inner(interrupt: u64, process_stack: u64) {
    rprintln!("Process interrupt {}", interrupt);

    asm!("jmp panic" :::: "volatile", "intel");
}
