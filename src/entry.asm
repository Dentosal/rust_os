[BITS 64]

global start
extern rust_main

section .entry
start:
    mov ecx, 0xBEEFDEAD
    mov rbx, 0xbeefbeefbeefbeef
    mov qword [0xb8000], rbx
    call rust_main

    mov rax, 0x4f724f204f534f4f
    mov [0xb8000], rax
    mov rax, 0x4f724f754f744f65
    mov [0xb8008], rax
    mov rax, 0x4f214f644f654f6e
    mov [0xb8010], rax

    hlt
