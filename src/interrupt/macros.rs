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
        " ::: "memory" : "intel", "volatile");
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
        " ::: "memory" : "intel", "volatile");
    }
}

macro_rules! syscall_save_scratch_registers {
    () => {
        asm!("push rcx
              push rsi
              push rdi
              push r8
              push r9
              push r10
              push r11
        " ::: "memory" : "intel", "volatile");
    }
}

macro_rules! syscall_restore_scratch_registers {
    () => {
        asm!("pop r11
              pop r10
              pop r9
              pop r8
              pop rdi
              pop rsi
              pop rcx
        " ::: "memory" : "intel", "volatile");
    }
}


macro_rules! irq_handler {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                save_scratch_registers!();
                asm!("call $0" :: "i"($name as extern "C" fn()) : "memory" : "intel", "volatile");
                restore_scratch_registers!();
                asm!("iretq":::: "intel", "volatile");
                ::core::intrinsics::unreachable();
            }
        }
        wrapper as *const fn()
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
                    add rdi, 9*8  // calculate exception stack frame pointer
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
        wrapper as *const fn()
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
                asm!("xchg ax, ax" :::: "intel");
                ::core::intrinsics::unreachable();
            }
        }
        wrapper as *const fn()
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
