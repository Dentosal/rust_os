[BITS 64]
; Interrupt Descriptor Table setup and modification routines
; http://wiki.osdev.org/IDT

%include "src/asm_routines/constants.asm"

global idt_setup
extern panic


section .text:

int_handler:
    xor ax, ax
    mov gs, ax
    mov dword [gs:0xB8000],') : '
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
    mov rcx, 0x100  ; 0x100 = 255 entries
    mov rbx, 0x0    ; idt starts from 0x0
.foreach: ; IMPORTANT HERE FIXME: this does not do anything?? why??? (it is still called)
    ; store offset (pointer to irq routine) to rax
    ;mov rax, panic  ; default to panic. TODO: write a proper irq routine
    mov rax, int_handler  ; use tmp handler
    ; test if we should call something else on interrupt
    ;cmp

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

    mov edx, 0xF00D0001

    ; tell cpu where the idt is
    lidt [idtr]

    mov edx, 0xF00D0002

    mov eax, [idtr]

    int 1

    mov edx, 0xF00D0003

    jmp $

    ; return
    pop rdx
    pop rcx
    pop rbx
    pop rax
    ret

.int_1:
    ; mov rax,
    ret

; ; STRUCTURE
; idt_descriptor:
;     dw 0x0  ; offset_low (16 bits)
;     dw 0x0  ; selector
;     db 0x0  ; zero
;     db 0x0  ; type and attributes
;     dw 0x0  ; offset_middle (16 bits)
;     dd 0x0  ; offset_high (32 bits)
;     dd 0x0  ; zero
;
;
; idtr:   ; idt starts from 0x0 and is therefore page-aligned
;     dw 0x0  ; limit 0
;     dq 0x0  ; offset 0
