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

;# Description
; Process an interrupt from the user code.
; 1. Switch to kernel page tables and stack
; 2. Call the process interrupt handler
; 3. Switch back to process page tables and stack
process_interrupt:
.table_start:
    %rep 0x100
    call .common    ; Each call is 10 bytes
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

    push rax                ; Save original rax
    push rbx                ; Save original rbx
    push rcx                ; Save original rcx

    mov rbx, rsp            ; Get the process stack pointer (after the pushes here)

    ; Retrieve procedure entry address
    mov rax, [rsp + 8*3]
    ; Remove base and instruction, so rax is just the offset, between 0 and 255 * 10
    sub rax, (.table_start + 5)
    ; Divide by 5, the size of a call instruction here
    ; Asm div5 trick: https://godbolt.org/z/JyOTEr
    imul eax, 52429
    shr eax, 18

    ; Switch to kernel page table
    mov rcx, page_table_physaddr
    mov cr3, rcx

    ; Switch to kernel stack
    mov rsp, kernel_syscall_stack

    ; Jump to kernel interrupt handler
    xchg bx, bx
    jmp [interrupt_handler_ptr_addr]

.return_syscall:
    ; The kernel will jump here when returning from a system call
    ; TODO

.return_page_fault:
    ; The kernel will jump here when returning from a page fault,
    ; i.e. after loading a swapped-out page from disk
    ; TODO
