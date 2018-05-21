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

use keyboard;
use pic;
use time;

#[macro_use]
mod macros;
mod gdt;
pub mod idt;

use memory::MemoryController;

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
extern "C" fn exception_bp(stack_frame: *const ExceptionStackFrame) {
    unsafe {
        rforce_unlock!();
        rprintln!("Breakpoint at {:#x}\n{}", (*stack_frame).instruction_pointer, *stack_frame);
        bochs_magic_bp!();
    }
}

/// Invalid Opcode handler (instruction undefined)
extern "C" fn exception_ud(stack_frame: *const ExceptionStackFrame) {
    unsafe {
        rforce_unlock!();
        rprintln!("Exception: invalid opcode at {:#x}\n{}", (*stack_frame).instruction_pointer, *stack_frame);
    }
    loop {}
}

/// Double Fault handler
#[allow(unused_variables)]
extern "C" fn exception_df(stack_frame: *const ExceptionStackFrame, error_code: u64) {
    // error code is always zero
    unsafe {
        bochs_magic_bp!();
        panic_indicator!(0x4f664f64);   // "df"
        rforce_unlock!();
        rprintln!("Exception: Double Fault\n{}", *stack_frame);
        rprintln!("exception stack frame at {:#p}", stack_frame);
    }
    loop {}
}

/// General Protection Fault handler
extern "C" fn exception_gpf(stack_frame: *const ExceptionStackFrame, error_code: u64) {
    unsafe {
        rforce_unlock!();
        rprintln!("Exception: General Protection Fault with error code {:#x}\n{}", error_code, *stack_frame);
    }
    loop {}
}

/// Page Fault error codes
bitflags! {
    flags PageFaultErrorCode: u64 {
        const PROTECTION_VIOLATION  = 1 << 0,
        const CAUSED_BY_WRITE       = 1 << 1,
        const USER_MODE             = 1 << 2,
        const MALFORMED_TABLE       = 1 << 3,
        const INSTRUCTION_FETCH     = 1 << 4,
    }
}

/// Page Fault handler
extern "C" fn exception_pf(stack_frame: *const ExceptionStackFrame, error_code: u64) {
    unsafe {
        bochs_magic_bp!();
        rforce_unlock!();
        rprintln!("Exception: Page Fault with error code {:?} ({:?}) at {:#x}\n{}", error_code, PageFaultErrorCode::from_bits(error_code).unwrap(), register!(cr2), *stack_frame);
    }
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
extern "C" fn exception_snp(stack_frame: *const ExceptionStackFrame, error_code: u64) {
    unsafe {
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
    }
    loop {}
}

/// PIT timer ticked
extern "C" fn exception_irq0() {
    unsafe {
        time::SYSCLOCK.force_unlock();
        time::SYSCLOCK.lock().tick();
        pic::PICS.lock().notify_eoi(0x20);
    }
}


/// First ps/2 device, keyboard, sent data
pub extern "C" fn exception_irq1() {
    unsafe {
        rforce_unlock!();
        let mut kbd = keyboard::KEYBOARD.lock();
        if kbd.is_enabled() {
            kbd.notify();
        }
        pic::PICS.lock().notify_eoi(0x21);
    }
}


/// First ATA device is ready for data transfer
pub extern "C" fn exception_irq14() {
    // Since we are polling the drive, just ignore the IRQ
    unsafe {
        pic::PICS.lock().notify_eoi(0x2e);
    }
}


/// System calls
#[naked]
pub extern "C" fn syscall() -> ! {
    use super::syscall::{SyscallResult, call};

    unsafe {
        syscall_save_scratch_registers!();
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

        syscall_restore_scratch_registers!();
        asm!("iretq" :::: "intel", "volatile");
        ::core::intrinsics::unreachable();
    }
}

#[derive(Clone,Copy)]
struct HandlerInfo {
    function_ptr: *const fn(),
    privilege_level: PrivilegeLevel,
    tss_selector: u8
}
impl HandlerInfo {
    pub const fn new(f: *const fn(), pl: PrivilegeLevel, tss_s: u8) -> HandlerInfo {
        HandlerInfo { function_ptr: f, privilege_level: pl, tss_selector: tss_s }
    }
}

static TSS: Once<TaskStateSegment> = Once::new();
static GDT: Once<gdt::Gdt> = Once::new();

pub fn init(memory_controller: &mut MemoryController) {
    let mut handlers: [Option<HandlerInfo>; idt::ENTRY_COUNT] = [None; idt::ENTRY_COUNT];

    // Initialize TSS
    let double_fault_stack = memory_controller.alloc_stack(1).expect("could not allocate double fault stack");

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

    // Bind exception handlers
    handlers[0x00] = Some(HandlerInfo::new(simple_exception!("Divide-by-zero Error"), PrivilegeLevel::Ring0, 0));
    handlers[0x03] = Some(HandlerInfo::new(exception_handler!(exception_bp), PrivilegeLevel::Ring0, 0));
    handlers[0x06] = Some(HandlerInfo::new(exception_handler!(exception_ud), PrivilegeLevel::Ring0, 0));
    handlers[0x08] = Some(HandlerInfo::new(exception_handler_with_error_code!(exception_df), PrivilegeLevel::Ring0, 5));
    handlers[0x0b] = Some(HandlerInfo::new(exception_handler_with_error_code!(exception_snp), PrivilegeLevel::Ring0, 0));
    handlers[0x0d] = Some(HandlerInfo::new(exception_handler_with_error_code!(exception_gpf), PrivilegeLevel::Ring0, 0));
    handlers[0x0e] = Some(HandlerInfo::new(exception_handler_with_error_code!(exception_pf), PrivilegeLevel::Ring0, 0));
    handlers[0x20] = Some(HandlerInfo::new(irq_handler!(exception_irq0), PrivilegeLevel::Ring0, 0));
    handlers[0x21] = Some(HandlerInfo::new(irq_handler!(exception_irq1), PrivilegeLevel::Ring0, 0));
    handlers[0x2e] = Some(HandlerInfo::new(irq_handler!(exception_irq14), PrivilegeLevel::Ring0, 0));
    handlers[0xd7] = Some(HandlerInfo::new(syscall as *const fn(), PrivilegeLevel::Ring0, 0));

    for index in 0..=(idt::ENTRY_COUNT-1) {
        let descriptor = match handlers[index] {
            None        => {idt::Descriptor::new(false, 0, PrivilegeLevel::Ring0, 0)},
            Some(info)  => {idt::Descriptor::new(true, info.function_ptr as u64, info.privilege_level, info.tss_selector)}
        };
        unsafe {
            ptr::write_volatile((idt::ADDRESS + index * mem::size_of::<idt::Descriptor>()) as *mut _, descriptor);
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
