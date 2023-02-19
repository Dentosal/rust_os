; Process trampoline, available from both kernel and process page tables

%include "build/constants.asm"

[BITS 64]
[ORG 0x200000]

; Push and pop macros. RFLAGS not pushed, as it is part of the IST
; System calls rely on the order of registers in the stack.
%define stack_stored_registers 15
%macro push_all 0
    push rbp
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax
%endmacro
%macro pop_all 0
    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rsi
    pop rdi
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15
    pop rbp
%endmacro

; Offset lookup header
header:
    dq switch_to
    dq process_interrupt.table_start
    dq idle

;# Description
; Switch to another process.
; 1. Switch back to process page tables
; 2. Restore process stack and registers
; 3. Jump back into the process
;
;# Inputs
; rax: P4 page table physical address
; rdx: rsp for the process
;# Outputs
; none
switch_to:
    ; Save rax
    push rax
    ; Load GDT
        push qword 0x1000
        push word 8*2-1
        mov rax, rsp
        lgdt [rax]
        add rsp, 10
    ; Load IDT
        push qword 0
        push word 0x100*16-1
        mov rax, rsp
        lidt [rax]
        add rsp, 10
    ; Restore rax
    pop rax
    ; Switch page tables
    mov cr3, rax
    ; Set stack pointer
    mov rsp, rdx
    ; Prepare stack
    pop_all
    add rsp, 8 ; Discard tmpvar
    ; Return to process
    iretq

; Kernel addresses
%define interrupt_handler_ptr_addr 0xa000
%define page_table_physaddr 0x10_000_000
%define kernel_syscall_stack 0x11_000_000
%define kernel_syscall_stack_size 0x200_00
%define kernel_syscall_stack_end (kernel_syscall_stack + kernel_syscall_stack_size)


;# Description
; Process an interrupt from the user code.
; 1. Switch to kernel page tables and stack
; 2. Call the process interrupt handler
; 3. Switch back to process page tables and stack
process_interrupt:
.table_start:       ; Keep in sync with write_process_dts
    %rep 0x100
    call .common    ; Each call is 5 bytes
    %endrep
.common:
    ; As the interrupt enters here by `call .common`,
    ; the topmost item in the process stack is now
    ; the address from which the subroutine was entered.
    ; It will be used to determine the interrupt number.

    ; Save registers to stack
    ; * rax: Stores interrupt vector number
    ; * rbx: Stores process stack pointer
    ; * rcx: Used for misc operations
    ; * r10-r14: Stores the interrupt frame
    ; * r15: Stores the exception error code, if any
    ; Other registers are also preserved for process switching
    push_all

    ; Get the process stack pointer (after the pushes here)
    ; RBX is a scratch register, so it will not be overwritten by Rust code
    mov rbx, rsp

    ; Save process page table address
    ; RBP is a scratch register, so it will not be overwritten by Rust code
    mov rbp, cr3

    ; Retrieve procedure entry address
    mov rax, [rsp + stack_stored_registers * 8] ; offset for the pushes above
    ; Remove base and instruction, so rax is just the offset, between 0 and 255 * 10
    sub rax, (.table_start + 5)
    ; Divide by 5, the size of a call instruction here
    ; Asm div5 trick: https://godbolt.org/z/JyOTEr
    imul eax, 52429
    shr eax, 18

    ; Get error code, if any (See https://wiki.osdev.org/Exceptions for a list)
    ; Required if vector number is any of 0x08, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x11, 0x1e
    cmp rax, 0x08
    jl .no_error_code
    je .error_code
    cmp rax, 0x0a
    jl .no_error_code
    je .error_code
    cmp rax, 0x0e
    jle .error_code
    cmp rax, 0x11
    je .error_code
    cmp rax, 0x1e
    je .error_code

.no_error_code:
    ; No error code to get
    mov r15, 0

    ; Get interrupt stack frame (offset: pushes above + entry address)
    mov r10, [rsp + (stack_stored_registers + 1 + 0) * 8]
    mov r11, [rsp + (stack_stored_registers + 1 + 1) * 8]
    mov r12, [rsp + (stack_stored_registers + 1 + 2) * 8]
    mov r13, [rsp + (stack_stored_registers + 1 + 3) * 8]
    mov r14, [rsp + (stack_stored_registers + 1 + 4) * 8]

    jmp .after_error_code

.error_code:
    ; Get error code (offset: pushes above + entry address)
    mov r15, [rsp + (stack_stored_registers + 1) * 8]

    ; Get interrupt stack frame (offset: pushes above + entry address + error code)
    mov r10, [rsp + (stack_stored_registers + 1 + 1 + 0) * 8]
    mov r11, [rsp + (stack_stored_registers + 1 + 1 + 1) * 8]
    mov r12, [rsp + (stack_stored_registers + 1 + 1 + 2) * 8]
    mov r13, [rsp + (stack_stored_registers + 1 + 1 + 3) * 8]
    mov r14, [rsp + (stack_stored_registers + 1 + 1 + 4) * 8]

.after_error_code:

    ; Switch to kernel page table
    mov rcx, page_table_physaddr
    mov cr3, rcx

    ; Switch to kernel stack
    mov rsp, kernel_syscall_stack_end

    ; Switch to kernel interrupt handlers
    push qword IDT_ADDR
    push word 0x100 * 16 - 1
    mov rcx, rsp
    lidt [rcx]
    add rsp, 10

    ; Switch to kernel GDT
    push qword GDT_ADDR
    push word 4 * 8 - 1 ; TODO: named constant?
    mov rcx, rsp
    lgdt [rcx]
    add rsp, 10

    ; Invoke the kernel interrupt handler
    call [interrupt_handler_ptr_addr]

    ; TODO: Check if this should be a special return?

.return_normal:
    ; rax and rdx correct from kernel interrupt handler
    jmp switch_to


;# Description
; Idle loop to run when all processes are sleeping
; Does not trash any registers, memory, or stack
; Uses iretq to return from interrupt state.
; The looped hlt instruction is interrupted by
; the timer periodically.
idle:
    ; Fabricate suitable iretq structure
    push qword 0x0      ; Stack segment
    push qword rsp      ; Stack pointer
    push qword 0x0202   ; RFLAGS: Interrupt flag only (0x2 is reserved)
    push qword 0x8      ; Code segment
    push qword .loop    ; RIP: Address of .loop
    ; "Return" into the .loop below
    iretq
.loop:
    hlt
    jmp .loop