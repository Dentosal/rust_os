; Debug and system panic routines
[BITS 64]

global panic
global breakpoint

section .text:

panic:
    cli
    mov rax, 0x4f214f214f214f21 ; !!!!
    mov [0xb8000], rax
    hlt
    jmp $

breakpoint:
    mov rax, 0x4f744f704f724f62 ; brpt
    mov [0xb8000], rax
    hlt
    jmp $
