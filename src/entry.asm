[BITS 64]

%include "src/asm_routines/constants.asm"

global start
global endlabel
extern rust_main

section .entry
start:
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
    resb (4096*80) ; I have had a couple of overflows with just 4096-sized stack. Might be a good idea to increase this even more. We might require even 4096*0x10000, however this will make zeroing out .bss very slow.
stack_top:
