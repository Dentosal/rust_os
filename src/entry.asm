[BITS 64]

%include "build/constants.asm"

global _start
global endlabel
extern rust_main
extern rust_ap_get_stack
extern rust_ap_main

section .entry
_start:
    ; clear segment registers
    xor ax, ax
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    test rcx, rcx
    jnz .ap_cpu

    ; set up stack
    mov rsp, stack_top

    ; jump to kernel
    jmp rust_main

.ap_cpu:
    ; Reuse old kernel stack in case the Rust code needs it.
    ; Shouldn't cause any issues, as kernel stack isn't ever freed,
    ; and multiple processor cores should always write same values
    ; here, as the execution is identical.
    mov rsp, ap_tmp_stack_top

    ; Get a new available stack, top at rax
    call rust_ap_get_stack
    mov rsp, rax ; Switch stacks

    jmp rust_ap_main

section .bss

; reserve space for BSP stack
stack_bottom:
    resb (4096*1000)
    ; I have had a couple of overflows with just 4096-sized stack.
    ; Might be a good idea to increase this even more.
    ; Might require even 4096*0x10000, however this will make zeroing out .bss very slow.
stack_top:

; reserve space for tmp AP stack
ap_tmp_stack_bottom:
    resb 4096*10
ap_tmp_stack_top:
