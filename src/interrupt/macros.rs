use x86_64::PrivilegeLevel;


macro_rules! irq_handler {
    ($name:ident) => {{
        unsafe extern "x86-interrupt" fn wrapper(_: &mut ExceptionStackFrame) {
            ($name)();
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, 0)
    }};
}

macro_rules! syscall_handler {
    ($name:ident) => {{
        #[naked]
        unsafe fn wrapper(_: &mut ExceptionStackFrame) {
            asm!("push rcx
                push rsi
                push rdi
                push r8
                push r9
                push r10
                push r11
            " ::: "memory" : "intel", "volatile");

            asm!("
                call $0
            "   :
                : "i"($name as unsafe extern "C" fn())
                : "rdi"
                : "intel"
            );

            asm!("pop r11
                pop r10
                pop r9
                pop r8
                pop rdi
                pop rsi
                pop rcx
            " ::: "memory" : "intel", "volatile");

            asm!("iretq" :::: "intel", "volatile");
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, 0)
    }};
}

macro_rules! exception_handler {
    ($name:ident, $pl:expr, $tss_s:expr) => {{
        unsafe extern "x86-interrupt" fn wrapper(sf: &mut ExceptionStackFrame) {
            ($name)(sf);
        }
        idt::Descriptor::new(true, wrapper as u64, $pl, $tss_s)
    }};
    ($name:ident) => {{
        exception_handler!($name, PrivilegeLevel::Ring0, 0)
    }};
}

macro_rules! exception_handler_with_error_code {
    ($name:ident, $pl:expr, $tss_s:expr) => {{
        unsafe extern "x86-interrupt" fn wrapper(sf: &mut ExceptionStackFrame, ec: u64) {
            ($name)(sf, ec);
        }
        idt::Descriptor::new(true, wrapper as u64, $pl, $tss_s)
    }};
    ($name:ident) => {{
        exception_handler_with_error_code!($name, PrivilegeLevel::Ring0, 0)
    }};
}

macro_rules! simple_exception_handler {
    ($text:expr) =>  {{
        unsafe extern "x86-interrupt" fn wrapper(stack_frame: &mut ExceptionStackFrame) {
            unsafe {
                rforce_unlock!();
                rprintln!(concat!("Exception: ", $text, "\n{}"), stack_frame);
            };
            loop {}
        }
        idt::Descriptor::new(true, wrapper as u64, PrivilegeLevel::Ring0, 0)
    }};
}
