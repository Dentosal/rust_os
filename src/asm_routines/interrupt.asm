[BITS 64]
; Interrupt Descriptor Table setup and modification routines
; http://wiki.osdev.org/IDT

%include "src/asm_routines/constants.asm"

global idt_setup
global interrupt_unknown_exception

extern panic
extern panic_unknown_exception


section .text:

interrupt_unknown_exception:
    xor ax, ax
    mov gs, ax
    call panic_unknown_exception
    hlt
    jmp $

install_isr:
; in:       al = interrupt number, rbp = address of isr
; out:      None
; consumes: rax, rbp

    push rbp
    and rax, 0xFF
    shl rax, 4
    add rax, idt
    mov rdi, rax
    pop rbp

    mov [rdi], bp
    shr rbp, 16
    mov [rdi+6], bp
    shr rbp, 16
    mov [rdi+8], ebp

    ret

idt_setup:  ; set all interrupt routines to system panic
    ; save registers
    push rax
    push rbx
    push rcx
    push rdx

    ; set idt entries
    mov rcx, 0x100  ; 0x100 = 256 entries
    mov rbx, 0x0    ; idt starts from 0x0
.foreach:
    ; store offset (pointer to irq routine) to rax
    mov rax, panic_unknown_exception    ; default to panic with message about unknown exception

    mov [rbx+ 0], ax                ; offset_low
    mov [rbx+ 2], word gdt_selector_code    ; code segment selector
    mov [rbx+ 4], byte 0            ; zero
    mov [rbx+ 5], byte 0b10001110   ; type_attr: in use => 1, ring 0 => 0, interrupt gate => 0, interrupt gate => 1110,
    shr rax, 16                     ; get middle offset
    mov [rbx+ 6], ax                ; offset_middle
    shr rax, 32                     ; get high offset
    mov [rbx+ 8], eax               ; offset_high
    mov [rbx+12], dword 0           ; zero
    ; mov rax, rbx
    ; mov [0], rax

    add rbx, 64
    loop .foreach
    ; entries set

    ; write idtr
    ; idt starts from 0x0 and is therefore page-aligned
    ; limit: rbx
    mov rax, rbx
    mov [idtr], rax    ; set limit
    ; offset 0
    xor rax, rax
    mov [idtr+2], rax

    ; tell cpu where the idt is
    lidt [idtr]

    ; return
    pop rdx
    pop rcx
    pop rbx
    pop rax
    ret


; ; STRUCTURE
; idt_descriptor:
;     dw 0x0  ; u16 offset_low
;     dw 0x0  ; u16 selector
;     db 0x0  ; u8  zero
;     db 0x0  ; u8  type and attributes
;     dw 0x0  ; u16 offset_middle
;     dd 0x0  ; u32 offset_high
;     dd 0x0  ; u32 zero
;
;
; idtr:   ; idt starts from 0x0 and is therefore page-aligned
;     dw 0x0  ; limit 0
;     dq 0x0  ; offset 0
