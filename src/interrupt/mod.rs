use core::ptr;
use core::mem;

use vga_buffer;
use keyboard;
use pic;

// These constants MUST have defined with same values as those in src/asm_routines/constants.asm
// They also MUST match the ones in plan.md
// If a constant defined here doesn't exists in that file, then it's also fine
const GDT_SELECTOR_CODE: u16 = 0x08;
const IDT_ADDRESS: usize = 0x0;
const IDTR_ADDRESS: usize = 0x1000;
const IDT_ENTRY_COUNT: usize = 0x100;


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
    pub fn write(&self) {
        unsafe {
            ptr::write(IDTR_ADDRESS as *mut Self, *self);
        }
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


#[derive(Debug)]
#[repr(C)]
struct ExceptionStackFrame {
    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
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
                vga_buffer::panic_output(format_args!(concat!("Exception: ", $text, "\n{:#?}"), *stack_frame));
            };
            loop {}
        }
        exception_handler!(exception)
    }}
}


/// Breakpoint handler
extern "C" fn exception_bp(stack_frame: *const ExceptionStackFrame) {
    unsafe {
        // vga_buffer::panic_output(format_args!("EXCEPTION: Breakpoint at {:#x}\n{:#?}", (*stack_frame).instruction_pointer, *stack_frame));
        rprintln!("EXCEPTION: Breakpoint at {:#x}\n{:#?}", (*stack_frame).instruction_pointer, *stack_frame);
    }
}

/// Invalid Opcode handler (instruction undefined)
extern "C" fn exception_ud(stack_frame: *const ExceptionStackFrame) {
    unsafe {
        vga_buffer::panic_output(format_args!("Exception: invalid opcode at {:#x}\n{:#?}", (*stack_frame).instruction_pointer, *stack_frame));
    }
    loop {}
}

/// Double Fault handler
#[naked]
unsafe extern "C" fn exception_df() -> ! {
    // it has double faulted, so no more risks, just deliver the panic indicator
    unsafe {
        panic_indicator!(0x4f664f64);   // "df"
    }
    loop {}
}


/// General Protection Fault handler
extern "C" fn exception_gpf(stack_frame: *const ExceptionStackFrame, error_code: u64) {
    unsafe {
        vga_buffer::panic_output(format_args!("Exception: General Protection Fault with error code at {:#x}\n{:#?}", error_code, *stack_frame));
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
        vga_buffer::panic_output(format_args!("Exception: Page Fault with error code {:?} ({:?}) at {:#x}\n{:#?}", error_code, PageFaultErrorCode::from_bits(error_code).unwrap(), register!(cr2), *stack_frame));
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
        vga_buffer::panic_output(format_args!("Exception: Segment Not Present with error code {:#x} (e={:b},t={:?},i={:#x})\n{:#?}",
            error_code,
            error_code & 0b1,
            match (error_code & 0b110) >> 1 {
                0b00 => SegmentNotPresentTable::GDT,
                0b01 => SegmentNotPresentTable::IDT,
                0b10 => SegmentNotPresentTable::LDT,
                0b11 => SegmentNotPresentTable::IDT,
                _ => {unreachable!();}
            },
            (error_code & 0xFFFF) >> 4, // 3 ?
            *stack_frame
        ));
    }
    loop {}
}

extern "C" fn exception_irq0() {
    // just ignore it (use later for timer?)
    unsafe {
        pic::PICS.lock().notify_eoi(0x20);
    }
}


/// keyboard_event: first ps/2 device sent data
/// we just trust that it is a keyboard
/// ^^this should change when we properly initialize the ps/2 controller
pub extern "C" fn exception_irq1() {
    unsafe {
        keyboard::KEYBOARD.lock().notify();
        pic::PICS.lock().notify_eoi(0x21);
    }
}


pub fn init() {
    rprintln!("!2");

    let mut exception_handlers: [Option<*const fn()>; IDT_ENTRY_COUNT] = [None; IDT_ENTRY_COUNT];

    //exception_handlers[0x00] = Some(exception_de_wrapper as *const fn());
    exception_handlers[0x00] = Some(simple_exception!("Divide-by-zero Error") as *const fn());
    rprintln!("!3");
    exception_handlers[0x03] = Some(exception_handler!(exception_bp) as *const fn());
    rprintln!("!5"); loop {}
    exception_handlers[0x06] = Some(exception_handler!(exception_ud) as *const fn());
    exception_handlers[0x08] = Some(exception_df as *const fn());
    exception_handlers[0x0b] = Some(exception_handler_with_error_code!(exception_snp) as *const fn());
    exception_handlers[0x0d] = Some(exception_handler_with_error_code!(exception_gpf) as *const fn());
    exception_handlers[0x0e] = Some(exception_handler_with_error_code!(exception_pf) as *const fn());
    exception_handlers[0x20] = Some(irq_handler!(exception_irq0) as *const fn());
    exception_handlers[0x21] = Some(irq_handler!(exception_irq1) as *const fn());

    rprintln!("..."); loop {}

    for index in 0...(IDT_ENTRY_COUNT-1) {
        let descriptor = match exception_handlers[index] {
            None            => {IDTDescriptor::new(false, 0, 0)},
            Some(pointer)   => {IDTDescriptor::new(true, pointer as u64, 0)} // TODO: currenly all are ring 0b00
        };
        unsafe {
            ptr::write_volatile((IDT_ADDRESS + index * mem::size_of::<IDTDescriptor>()) as *mut _, descriptor);
        }
    }
    IDTReference::new().write();


    unsafe {
        asm!("lidt [$0]" :: "r"(IDTR_ADDRESS) : "memory" : "volatile", "intel");
        asm!("sti" :::: "volatile", "intel");
    }
}
