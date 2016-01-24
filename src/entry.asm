[BITS 64]

global start
extern rust_main
extern test_main

section .entry
start:
    mov al, '?'
    mov byte [0xb8000], al

    mov ecx, 0xBEEF0002
    mov rax, test_main
    mov rbx, [test_main]
    mov rdx, 0xf00d

    push ax
    mov al, '*'
    mov byte [0xb8000], al
    pop ax

    jmp $

    mov ecx, 0xBEEF0003
    call test_main
    mov ecx, 0xBEEF0004
    jmp $

    mov rax, 0x4f724f204f534f4f
    mov [0xb8000], rax
    mov rax, 0x4f724f754f744f65
    mov [0xb8008], rax
    mov rax, 0x4f214f644f654f6e
    mov [0xb8010], rax

    hlt
