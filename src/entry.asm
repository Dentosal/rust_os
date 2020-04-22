[BITS 64]

%include "src/asm_routines/constants.asm"

global start
global endlabel
extern rust_main

section .entry
start:
    ; clear segment registers
    xor ax, ax
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    ; set up stack
    mov rsp, stack_top

    ; jump to kernel
    jmp rust_main

; reserve space for stack
section .bss
stack_bottom:
    resb (4096*1000)
    ; I have had a couple of overflows with just 4096-sized stack.
    ; Might be a good idea to increase this even more.
    ; Might require even 4096*0x10000, however this will make zeroing out .bss very slow.
stack_top:
