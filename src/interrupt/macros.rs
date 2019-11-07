use x86_64::PrivilegeLevel;

macro_rules! irq_handler {
    ($name:ident) => {{
        unsafe extern "x86-interrupt" fn wrapper(_: &mut InterruptStackFrame) {
            ($name)();
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, 0)
    }};
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

// IRQ handler that allows switching to next process if required
// The handler must return `(u64, u64)` (encoded as u128) in `(rax, rdx)`, per System V ABI:
// https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI
// If process switch is required, then the returned pair of u64 should be
// (p4_physical_address, stack_pointer)
// If no switch should be done then function must return `(0, 0)`.
macro_rules! irq_handler_switch {
    ($name:ident) => {{
        #[naked]
        unsafe fn wrapper(_: &mut InterruptStackFrame) -> ! {
            use crate::memory::process_common_code::COMMON_ADDRESS_VIRT;
            save_scratch_registers!();
            asm!("
                push rcx        // Save COMMON_ADDRESS_VIRT
                //sub rsp, 8    // Align the stack pointer
                call $0         // Call the exception handler
                //add rsp, 8    // Undo stack pointer alignment
                pop rcx         // Restore COMMON_ADDRESS_VIRT
                test rax, rax   // Check whether a process switch is required
                jz .noswitch    // Jump to process switch routine, if required
                mov rcx, [rcx]  // Get procedure offset
                jmp rcx         // Jump into the procedure
            .noswitch:
                "
                :
                :
                    "i"($name as unsafe extern "C" fn() -> u128),
                    "{rcx}"(COMMON_ADDRESS_VIRT as *const u8 as u64) // switch_to
                :
                : "intel"
            );
            restore_scratch_registers!();
            asm!("iretq" :::: "intel", "volatile");
            ::core::intrinsics::unreachable();
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, 0)
    }};
}

macro_rules! exception_handler {
    ($name:ident, $pl:expr, $tss_s:expr) => {{
        unsafe extern "x86-interrupt" fn wrapper(sf: &mut InterruptStackFrame) {
            ($name)(sf);
        }
        idt::Descriptor::new(true, wrapper as u64, $pl, $tss_s)
    }};
    ($name:ident) => {{ exception_handler!($name, PrivilegeLevel::Ring0, 0) }};
}

macro_rules! exception_handler_with_error_code {
    ($name:ident, $pl:expr, $tss_s:expr) => {{
        unsafe extern "x86-interrupt" fn wrapper(sf: &mut InterruptStackFrame, ec: u64) {
            ($name)(sf, ec);
        }
        idt::Descriptor::new(true, wrapper as u64, $pl, $tss_s)
    }};
    ($name:ident) => {{ exception_handler_with_error_code!($name, PrivilegeLevel::Ring0, 0) }};
}

macro_rules! simple_exception_handler {
    ($text:expr) => {{
        unsafe extern "x86-interrupt" fn wrapper(stack_frame: &mut InterruptStackFrame) {
            panic!(concat!("Exception: ", $text, "\n{:?}"), stack_frame);
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, 0)
    }};
}

macro_rules! last_resort_exception_handler {
    () => {{
        unsafe extern "x86-interrupt" fn wrapper(_stack_frame: &mut InterruptStackFrame) {
            asm!("jmp panic"::::"intel","volatile");
            ::core::hint::unreachable_unchecked();
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, 0)
    }};
}
