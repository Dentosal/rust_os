; Process trampoline, available from both kernel and process page tables

[BITS 64]
[ORG 0x200000]

; Push and pop macros
%macro push_all 0
    pushfq
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    push rbp
%endmacro

%macro pop_all 0
    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    popfq
%endmacro

header:
    dq switch_to
    dq process_interrupt.table_start

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
        push qword 16*0x100
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
    ; Reload CS
    push qword 0x8
    lea rax, [rel .new_cs] ; to ._new_cs
    push rax
    o64 retf
.new_cs:
    pop_all
    ret

; Kernel addresses
%define interrupt_handler_ptr_addr 0x2000
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
.table_start:
    %rep 0x100
    call .common    ; Each call is 5 bytes
    %endrep
.common:
    ; As the interrupt enters here by `call .common`,
    ; the topmost item in the process stack is now
    ; the address from which the subroutine was entered.
    ; It will be used to determine the interrupt number.

    ; Save some registers to stack
    ; * rax: Stores interrupt vector number
    ; * rbx: Stores process stack pointer
    ; * rcx: Used for misc operations
    ; * r10-r14: Stores the interrupt frame
    ; * r15: Stores the exception error code, if any
    push rax
    push rbx
    push rcx
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    ; Get the process stack pointer (after the pushes here)
    mov rbx, rsp

    ; Retrieve procedure entry address
    mov rax, [rsp + 9 * 8] ; 9 marks number of pushes above
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

    ; Get interrupt stack frame (10 for (pushes above + entry address))
    mov r10, [rsp + (10 + 0) * 8]
    mov r11, [rsp + (10 + 1) * 8]
    mov r12, [rsp + (10 + 2) * 8]
    mov r13, [rsp + (10 + 3) * 8]
    mov r14, [rsp + (10 + 4) * 8]

    jmp .after_error_code

.error_code:
    ; Get error code (10 for (pushes above + entry address))
    mov r10, [rsp + 10 * 8]

    ; Get interrupt stack frame (11 for (pushes above + entry address + error code))
    mov r10, [rsp + (11 + 0) * 8]
    mov r11, [rsp + (11 + 1) * 8]
    mov r12, [rsp + (11 + 2) * 8]
    mov r13, [rsp + (11 + 3) * 8]
    mov r14, [rsp + (11 + 4) * 8]

.after_error_code:

    ; Switch to kernel page table
    mov rcx, page_table_physaddr
    mov cr3, rcx

    ; Switch to kernel stack
    mov rsp, kernel_syscall_stack_end

    ; Switch to kernel interrupt handlers
    push qword 0x0
    push word 0x100 * 16 - 1
    mov rcx, rsp
    lidt [rcx]
    add rsp, 10

    ; Switch to kernel GDT
    push qword 0x1000
    push word 4 * 8 - 1
    mov rcx, rsp
    lgdt [rcx]
    add rsp, 10

    ; Jump to kernel interrupt handler
    jmp [interrupt_handler_ptr_addr]

.return_syscall:
    ; The kernel will jump here when returning from a system call
    ; TODO

.return_page_fault:
    ; The kernel will jump here when returning from a page fault,
    ; i.e. after loading a swapped-out page from disk
    ; TODO
