; Debug and system panic routines
[BITS 64]

global panic_stop

section .text

panic_stop:
    mov rax, 0x4f214f214f214f21 ; !!!!
    mov [0xb8000], rax
    cli
    hlt
    jmp $
