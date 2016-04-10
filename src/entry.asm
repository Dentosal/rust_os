[BITS 64]

global start
global error
global breakpoint
extern rust_main

section .entry
start:
    mov edx, 0xf00d0000
    ; set up stack
    mov esp, stack_top

    mov edx, 0xf00d0001

    call rust_main

    mov edx, 0xf00d0002

    ; rust main returned, print `OS returned!`
    mov rax, 0x4f724f204f534f4f
    mov [0xb8000], rax
    mov rax, 0x4f724f754f744f65
    mov [0xb8008], rax
    mov rax, 0x4f214f644f654f6e
    mov [0xb8010], rax
    hlt
section .text:
error:
    mov rax, 0x4f214f214f214f21
    mov [0xb8000], rax
    hlt
    jmp $
breakpoint:
    mov rax, 0x4f744f704f724f62
    mov [0xb8000], rax
    hlt
    jmp $

; reserve space for stack
section .bss
stack_bottom:
    resb 4096
stack_top:
