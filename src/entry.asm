[BITS 64]

global start
extern rust_main

section .entry
start:
    mov byte [0xb8000], '?'

    mov ecx, 0xBEEF0002
    mov rax, rust_main
    mov rbx, [rust_main]

    mov byte [0xb8000], '*'

    mov ecx, 0xBEEF0003
    call rust_main
    mov ecx, 0xBEEF0004

    ; rust main returned, print `OS returned!`
    mov rax, 0x4f724f204f534f4f
    mov [0xb8000], rax
    mov rax, 0x4f724f754f744f65
    mov [0xb8008], rax
    mov rax, 0x4f214f644f654f6e
    mov [0xb8010], rax
    hlt
