use core::arch::asm;
use x86_64::PrivilegeLevel;

macro_rules! irq_handler {
    ($name:ident, $ist:expr $(, $arg:literal)?) => {{
        unsafe extern "x86-interrupt" fn wrapper(_: &mut InterruptStackFrame) {
            ($name)($($arg)?);
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, $ist)
    }};
}

macro_rules! asm_save_scratch_registers {
    () => {
        "
        push rax
        push rcx
        push rdx
        push rsi
        push rdi
        push r8
        push r9
        push r10
        push r11
        "
    };
}

macro_rules! asm_restore_scratch_registers {
    () => {
        "
        pop r11
        pop r10
        pop r9
        pop r8
        pop rdi
        pop rsi
        pop rdx
        pop rcx
        pop rax
        "
    };
}

// IRQ handler that allows switching to next process if required
// The handler must return `(u64, u64)` (encoded as u128) in `(rax, rdx)`, per System V ABI:
// https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI
// If process switch is required, then the returned pair of u64 should be
// (p4_physical_address, stack_pointer)
// If no switch should be done then function must return `(0, 0)`.
macro_rules! irq_handler_switch {
    ($name:ident, $ist:expr, $arg:expr) => {{
        use crate::memory::process_common_code::COMMON_ADDRESS_VIRT;

        #[naked]
        unsafe extern "sysv64" fn wrapper(_: &mut InterruptStackFrame) {
            ::core::arch::asm!(concat!(
                    asm_save_scratch_registers!(), "
                    push rcx                // Save COMMON_ADDRESS_VIRT
                    push rdi                // Save rdi
                    sub rsp, 8              // Align the stack pointer
                    mov rdi, {arg}          // Pass the argument to the handler
                    call {handler}          // Call the exception handler
                    add rsp, 8              // Undo stack pointer alignment
                    pop rdi                 // Restore rdi
                    pop rcx                 // Restore COMMON_ADDRESS_VIRT
                    test rax, rax           // Check whether a process switch is required
                    jz .noswitch_", $arg, " // Jump to process switch routine, if required
                    mov rcx, [{cav}]        // Get procedure offset
                    jmp rcx                 // Jump into the procedure
                    .noswitch_", $arg, ":",
                    asm_restore_scratch_registers!(),
                    "iretq"
                ),
                handler = sym $name,
                arg = const $arg,
                cav = const COMMON_ADDRESS_VIRT,
                options(noreturn)
            );
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, $ist)
    }};
}

macro_rules! exception_handler {
    ($name:ident, $pl:expr, $ist:expr) => {{
        unsafe extern "x86-interrupt" fn wrapper(sf: &mut InterruptStackFrame) {
            ($name)(sf);
        }
        idt::Descriptor::new(true, wrapper as u64, $pl, $ist)
    }};
    ($name:ident) => {{ exception_handler!($name, PrivilegeLevel::Ring0, None) }};
}

macro_rules! exception_handler_with_error_code {
    ($name:ident, $pl:expr, $ist:expr) => {{
        unsafe extern "x86-interrupt" fn wrapper(sf: &mut InterruptStackFrame, ec: u64) {
            ($name)(sf, ec);
        }
        idt::Descriptor::new(true, wrapper as u64, $pl, $ist)
    }};
    ($name:ident) => {{ exception_handler_with_error_code!($name, PrivilegeLevel::Ring0, None) }};
}

macro_rules! simple_exception_handler {
    ($text:expr, $ist:expr) => {{
        unsafe extern "x86-interrupt" fn wrapper(stack_frame: &mut InterruptStackFrame) {
            panic!(
                concat!("Exception: ", $text, " (cpu {})\n{:?}"),
                crate::smp::current_processor_id(),
                stack_frame
            );
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, $ist)
    }};
}

macro_rules! last_resort_exception_handler {
    () => {{
        unsafe extern "x86-interrupt" fn wrapper(_stack_frame: &mut InterruptStackFrame) -> ! {
            asm!("jmp panic", options(noreturn));
            ::core::hint::unreachable_unchecked();
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, None)
    }};
}
