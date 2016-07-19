[BITS 64]

%include "src/asm_routines/constants.asm"

global start
extern rust_main

section .entry
start:
    ; update segments
    mov dx, gdt_selector_data   ; data selector
    mov ss, dx  ; stack segment
    mov ds, dx  ; data segment
    mov es, dx  ; extra segment
    mov fs, dx  ; f-segment
    mov gs, dx  ; g-segment

    ; set up stack
    mov rsp, stack_top

    ; get to kernel
    call rust_main

    ; rust main returned, print `OS returned!`
    mov rax, 0x4f724f204f534f4f
    mov [0xb8000], rax
    mov rax, 0x4f724f754f744f65
    mov [0xb8008], rax
    mov rax, 0x4f214f644f654f6e
    mov [0xb8010], rax
    hlt

; reserve space for stack
section .bss
stack_bottom:
    resb 4096
stack_top:
