use spin::Once;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::instructions::segmentation::set_cs;
use x86_64::instructions::tables::load_tss;
use x86_64::PrivilegeLevel;
use x86_64::VirtualAddress;


use core::ptr;
use core::mem;
use core::fmt;

use crate::keyboard;
use crate::pic;
use crate::time;

#[macro_use]
mod macros;
mod gdt;
pub mod idt;

use memory::{self, MemoryController};

#[repr(C,packed)]
struct ExceptionStackFrame {
    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64
}
impl fmt::Display for ExceptionStackFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ip = self.instruction_pointer;
        let cs = self.code_segment;
        let fl = self.cpu_flags;
        let sp = self.stack_pointer;
        let ss = self.stack_segment;

        write!(f, "ExceptionStackFrame {{\n  rip: {:#x},\n  cs: {:#x},\n  flags: {:#x},\n  rsp: {:#x},\n  ss: {:#x}\n}}", ip, cs, fl, sp, ss)
    }
}

/// Breakpoint handler
unsafe fn exception_bp(stack_frame: &ExceptionStackFrame) {
    rforce_unlock!();
    rprintln!("Breakpoint at {:#x}\n{}", (*stack_frame).instruction_pointer, *stack_frame);
    bochs_magic_bp!();
}

/// Invalid Opcode handler (instruction undefined)
unsafe fn exception_ud(stack_frame: &ExceptionStackFrame) {
    rforce_unlock!();
    rprintln!("Exception: invalid opcode at {:#x}\n{}", (*stack_frame).instruction_pointer, *stack_frame);
    loop {}
}

/// Double Fault handler
#[allow(unused_variables)]
unsafe fn exception_df(stack_frame: &ExceptionStackFrame, error_code: u64) {
    // error code is always zero
    panic_indicator!(0x4f664f64);   // "df"
    rforce_unlock!();
    rprintln!("Exception: Double Fault\n{}", *stack_frame);
    rprintln!("exception stack frame at {:#p}", stack_frame);
    loop {}
}

/// General Protection Fault handler
unsafe fn exception_gpf(stack_frame: &ExceptionStackFrame, error_code: u64) {
    rforce_unlock!();
    rprintln!("Exception: General Protection Fault with error code {:#x}\n{}", error_code, *stack_frame);
    loop {}
}

/// Page Fault error codes
bitflags! {
    struct PageFaultErrorCode: u64 {
        const PROTECTION_VIOLATION  = 1 << 0;
        const CAUSED_BY_WRITE       = 1 << 1;
        const USER_MODE             = 1 << 2;
        const MALFORMED_TABLE       = 1 << 3;
        const INSTRUCTION_FETCH     = 1 << 4;
    }
}

/// Page Fault handler
unsafe fn exception_pf(stack_frame: &ExceptionStackFrame, error_code: u64) {
    rforce_unlock!();
    rprintln!("Exception: Page Fault with error code {:?} ({:?}) at {:#x}\n{}", error_code, PageFaultErrorCode::from_bits(error_code).unwrap(), register!(cr2), *stack_frame);
    loop {}
}

#[derive(Debug)]
#[allow(dead_code)]
enum SegmentNotPresentTable {
    GDT,
    IDT,
    LDT
}

/// Segment Not Present handler
unsafe fn exception_snp(stack_frame: &ExceptionStackFrame, error_code: u64) {
    rforce_unlock!();
    rprintln!("Exception: Segment Not Present with error code {:#x} (e={:b},t={:?},i={:#x})\n{}",
        error_code,
        error_code & 0b1,
        match (error_code & 0b110) >> 1 {
            0b00 => SegmentNotPresentTable::GDT,
            0b01 => SegmentNotPresentTable::IDT,
            0b10 => SegmentNotPresentTable::LDT,
            0b11 => SegmentNotPresentTable::IDT,
            _ => {unreachable!();}
        },
        (error_code & 0xFFFF) >> 4, // FIXME: 3 ?
        *stack_frame
    );
    loop {}
}

/// PIT timer ticked
unsafe fn exception_irq0() {
    time::SYSCLOCK.force_unlock();
    time::SYSCLOCK.lock().tick();
    pic::PICS.lock().notify_eoi(0x20);
}


/// First ps/2 device, keyboard, sent data
unsafe fn exception_irq1()  {
    rforce_unlock!();
    keyboard::KEYBOARD.force_unlock();
    let mut kbd = keyboard::KEYBOARD.lock();
    if kbd.is_enabled() {
        kbd.notify();
    }
    pic::PICS.lock().notify_eoi(0x21);
}


/// First ATA device is ready for data transfer
pub unsafe fn exception_irq14() {
    // Since we are polling the drive, just ignore the IRQ
    pic::PICS.lock().notify_eoi(0x2e);
}


/// System calls
#[naked]
pub unsafe extern "C" fn syscall() {
    use super::syscall::{SyscallResult, call};

    let routine: u64;
    let arg0: u64;
    let arg1: u64;
    let arg2: u64;
    let arg3: u64;

    asm!("" :
        "={rax}"(routine),
        "={rdi}"(arg0),
        "={rsi}"(arg1),
        "={rdx}"(arg2),
        "={rcx}"(arg3)
        :::
        "intel"
    );

    let result = call(routine, (arg0, arg1, arg2, arg3));
    register!(rax, result.success);
    register!(rdx, result.result);
}


static TSS: Once<TaskStateSegment> = Once::new();
static GDT: Once<gdt::Gdt> = Once::new();

pub fn init() {
    // Initialize TSS
    let double_fault_stack = memory::configure(|mem_ctrl: &mut MemoryController| {
        mem_ctrl.alloc_stack(1).expect("could not allocate double fault stack")
    });

    let mut code_selector   = SegmentSelector::new(0, PrivilegeLevel::Ring0);
    let mut tss_selector    = SegmentSelector::new(1, PrivilegeLevel::Ring0);

    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[gdt::DOUBLE_FAULT_IST_INDEX] = VirtualAddress(double_fault_stack.top);
        tss
    });

    let gdt = GDT.call_once(|| {
        let mut gdt = gdt::Gdt::new();
        code_selector = gdt.add_entry(gdt::Descriptor::kernel_code_segment());
        tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
        gdt
    });

    unsafe {
        // load GDT
        gdt.load();
        // reload code segment register
        set_cs(code_selector);
        // load TSS
        load_tss(tss_selector);
    }

    let mut handlers: [idt::Descriptor; idt::ENTRY_COUNT] = [
        idt::Descriptor::new(false, 0, PrivilegeLevel::Ring0, 0); idt::ENTRY_COUNT
    ];

    // Bind exception handlers
    handlers[0x00] = simple_exception_handler!("Divide-by-zero Error");
    handlers[0x03] = exception_handler!(exception_bp);
    handlers[0x06] = exception_handler!(exception_ud);
    handlers[0x08] = exception_handler_with_error_code!(exception_df, PrivilegeLevel::Ring0, 5);
    handlers[0x0b] = exception_handler_with_error_code!(exception_snp);
    handlers[0x0d] = exception_handler_with_error_code!(exception_gpf);
    handlers[0x0e] = exception_handler_with_error_code!(exception_pf);
    handlers[0x20] = irq_handler!(exception_irq0);
    handlers[0x21] = irq_handler!(exception_irq1);
    handlers[0x2e] = irq_handler!(exception_irq14);
    handlers[0xd7] = syscall_handler!(syscall);

    for index in 0..=(idt::ENTRY_COUNT-1) {
        unsafe {
            ptr::write_volatile((idt::ADDRESS + index * mem::size_of::<idt::Descriptor>()) as *mut _, handlers[index]);
        }
    }

    unsafe {
        idt::Reference::new().write();
    }

    rprintln!("Enabling interrupt handler...");

    unsafe {
        asm!("lidt [$0]" :: "r"(idt::R_ADDRESS) : "memory" : "volatile", "intel");
        asm!("sti" :::: "volatile", "intel");
    }

    rprintln!("Enabled.");
}
