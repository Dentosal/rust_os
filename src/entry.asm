[BITS 64]

global start

section .entry
start:

    mov ecx, 0xBEEF0001
    mov al, '?'
    mov byte [0xb8000], al
    jmp start

    mov ecx, 0xBEEF0002
    extern rust_main
    extern test_main
    mov rsi, test_main
    mov rdi, [test_main]
    mov eax, start
    mov ebx, [start]
    call test_main
    mov ecx, 0xBEEF0003


    mov rax, 0x4f724f204f534f4f
    mov [0xb8000], rax
    mov rax, 0x4f724f754f744f65
    mov [0xb8008], rax
    mov rax, 0x4f214f644f654f6e
    mov [0xb8010], rax

    hlt
