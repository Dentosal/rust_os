; Debug and system panic routines
[BITS 64]

global panic
global breakpoint

section .text

panic:
    mov rax, 0x4f214f214f214f21 ; !!!!
    mov [0xb8000], rax
    cli
    hlt
    jmp $
