use core::ptr;
use core::mem;
use core::fmt;

use keyboard;
use pic;
use time;

// These constants MUST have defined with same values as those in src/asm_routines/constants.asm
// They also MUST match the ones in plan.md
// If a constant defined here doesn't exists in that file, then it's also fine
const GDT_SELECTOR_CODE: u16 = 0x08;
pub const IDT_ADDRESS: usize = 0x0;
pub const IDTR_ADDRESS: usize = 0x1000;
pub const IDT_ENTRY_COUNT: usize = 0x100;


#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct IDTReference {
    limit: u16,
    offset: u64
}
impl IDTReference {
    pub fn new() -> IDTReference {
        IDTReference {
            limit: ((IDT_ENTRY_COUNT-1)*(mem::size_of::<IDTDescriptor>())) as u16,
            offset: IDT_ADDRESS as u64
        }
    }
    pub unsafe fn write(&self) {
        ptr::write_volatile(IDTR_ADDRESS as *mut Self, *self);
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IDTDescriptor {
    pointer_low: u16,
    gdt_selector: u16,
    zero: u8,
    options: u8,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32
}

impl IDTDescriptor {
    pub fn new(present: bool, pointer: u64, ring: u8) -> IDTDescriptor {
        assert!(ring < 4);
        assert!(present || (pointer == 0 && ring == 0)); // pointer and ring must be 0 if not present
        // example options: present => 1, ring 0 => 00, interrupt gate => 0, interrupt gate => 1110,
        let options: u8 = 0b0_00_0_1110 | (ring << 5) | ((if present {1} else {0}) << 7);

        IDTDescriptor {
            pointer_low: (pointer & 0xffff) as u16,
            gdt_selector: GDT_SELECTOR_CODE,
            zero: 0,
            options: options,
            pointer_middle: ((pointer & 0xffff_0000) >> 16) as u16,
            pointer_high: ((pointer & 0xffff_ffff_0000_0000) >> 32) as u32,
            reserved: 0,
        }
    }
}


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
        write!(f, "ExceptionStackFrame {{\n  rip: {:#x},\n  cs: {:#x},\n  flags: {:#x},\n  rsp: {:#x},\n  ss: {:#x}\n}}", self.instruction_pointer, self.code_segment, self.cpu_flags, self.stack_pointer, self.stack_segment)
    }
}

macro_rules! save_scratch_registers {
    () => {
        asm!("push rax
              push rcx
              push rdx
              push rsi
              push rdi
              push r8
              push r9
              push r10
              push r11
        " :::: "intel", "volatile");
    }
}

macro_rules! restore_scratch_registers {
    () => {
        asm!("pop r11
              pop r10
              pop r9
              pop r8
              pop rdi
              pop rsi
              pop rdx
              pop rcx
              pop rax
            " :::: "intel", "volatile");
    }
}

macro_rules! irq_handler {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                save_scratch_registers!();
                asm!("call $0" :: "i"($name as extern "C" fn()) :: "intel", "volatile");
                restore_scratch_registers!();
                asm!("iretq":::: "intel", "volatile");
                ::core::intrinsics::unreachable();
            }
        }
        wrapper
    }}
}

macro_rules! exception_handler {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                save_scratch_registers!();
                asm!("
                    mov rdi, rsp  // pointer to stack as first argument
                    add rdi, 9*8 // calculate exception stack frame pointer
                    call $0       // call handler
                "   :
                    : "i"($name as extern "C" fn(*const ExceptionStackFrame))
                    : "rdi"
                    : "intel"
                );
                restore_scratch_registers!();
                asm!("iretq":::: "intel", "volatile");
                ::core::intrinsics::unreachable();
            }
        }
        wrapper
    }}
}

macro_rules! exception_handler_with_error_code {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                save_scratch_registers!();
                asm!("
                    mov rsi, [rsp+9*8]  // load error code into rsi
                    mov rdi, rsp        // pointer to stack as first argument
                    add rdi, 10*8       // calculate exception stack frame pointer
                    sub rsp, 8          // align the stack pointer
                    call $0             // call handler
                    add rsp, 8          // undo stack pointer alignment
                "   :
                    : "i"($name as extern "C" fn(*const ExceptionStackFrame, u64))
                    : "rdi","rsi"
                    : "intel"
                );
                restore_scratch_registers!();
                asm!("
                    add rsp, 8  // drop error code
                    iretq       // return from exception
                "   :::: "intel", "volatile");
                ::core::intrinsics::unreachable();
            }
        }
        wrapper
    }}
}

macro_rules! simple_exception {
    ($text:expr) =>  {{
        extern "C" fn exception(stack_frame: *const ExceptionStackFrame) {
            unsafe {
                rforce_unlock!();
                rprintln!(concat!("Exception: ", $text, "\n{}"), *stack_frame);
            };
            loop {}
        }
        exception_handler!(exception)
    }}
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


pub fn init() {
    let mut exception_handlers: [Option<*const fn()>; IDT_ENTRY_COUNT] = [None; IDT_ENTRY_COUNT];

    //exception_handlers[0x00] = Some(exception_de_wrapper as *const fn());
    exception_handlers[0x00] = Some(simple_exception!("Divide-by-zero Error") as *const fn());
    exception_handlers[0x03] = Some(exception_handler!(exception_bp) as *const fn());
    exception_handlers[0x06] = Some(exception_handler!(exception_ud) as *const fn());
    exception_handlers[0x08] = Some(exception_handler_with_error_code!(exception_df) as *const fn());
    exception_handlers[0x0b] = Some(exception_handler_with_error_code!(exception_snp) as *const fn());
    exception_handlers[0x0d] = Some(exception_handler_with_error_code!(exception_gpf) as *const fn());
    exception_handlers[0x0e] = Some(exception_handler_with_error_code!(exception_pf) as *const fn());
    exception_handlers[0x20] = Some(irq_handler!(exception_irq0) as *const fn());
    exception_handlers[0x21] = Some(irq_handler!(exception_irq1) as *const fn());

    for index in 0...(IDT_ENTRY_COUNT-1) {
        let descriptor = match exception_handlers[index] {
            None            => {IDTDescriptor::new(false, 0, 0)},
            Some(pointer)   => {IDTDescriptor::new(true, pointer as u64, 0)} // TODO: currenly all are ring 0b00
        };
        unsafe {
            ptr::write_volatile((IDT_ADDRESS + index * mem::size_of::<IDTDescriptor>()) as *mut _, descriptor);
        }
    }

    unsafe {
        IDTReference::new().write();
    }

    rprintln!("Enabling interrupt handler...");

    unsafe {
        asm!("lidt [$0]" :: "r"(IDTR_ADDRESS) : "memory" : "volatile", "intel");
        asm!("sti" :::: "volatile", "intel");
    }

    rprintln!("Enabled.");
}
