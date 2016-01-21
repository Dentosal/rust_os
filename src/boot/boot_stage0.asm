; MASTER BOOT RECORD
; STAGE 0

[BITS 16]
[ORG 0x7c00]

boot:
    ; initialize segment registers
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    ; initialize stack
    mov sp, 0x7bfe
    ; load more code into 0x7e00 so we can jump to it later
    mov ah, 2       ; read
    mov al, 15      ; 15 sectors (15/2 = 7.5 KiB)
    mov ch, 0       ; cylinder & 0xff
    mov cl, 2       ; sector | ((cylinder >> 2) & 0xc0)
    mov dh, 0       ; head
    mov bx, 0x7e00  ; read buffer
    int 0x13
    jc error
    ; load protected mode GDT and a null IDT (we don't need interrupts)
    cli
    lgdt [gdtr]
    lidt [idtr]
    ; set protected mode bit of cr0
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    ; far jump to load CS with 32 bit segment
    jmp 0x08:protected_mode

error:
    mov si, .msg
.loop:
    lodsb
    or al, al
    jz .done
    mov ah, 0x0e
    int 0x10
    jmp .loop
.done:
    jmp $
    .msg db "could not read disk", 0

[BITS 32]
protected_mode:
    ; load all the other segments with 32 bit data segments
    mov eax, 0x10
    mov ds, eax
    mov es, eax
    mov fs, eax
    mov gs, eax
    mov ss, eax
    ; set up stack
    mov esp, 0x7bfc
    ; SCREEN: top left: "0 "
    mov dword [0xb8000], 0x2f302f20

    ; jump into stage 1
    call 0x7e00

gdtr:
    dw (gdt_end - gdt) + 1  ; size
    dd gdt                  ; offset

idtr:
    dw 0
    dd 0

gdt:
    ; null entry
    dq 0
    ; code entry
    dw 0xffff       ; limit 0:15
    dw 0x0000       ; base 0:15
    db 0x00         ; base 16:23
    db 0b10011010   ; access byte - code
    db 0x4f         ; flags/(limit 16:19). flag is set to 32 bit protected mode
    db 0x00         ; base 24:31
    ; data entry
    dw 0xffff       ; limit 0:15
    dw 0x0000       ; base 0:15
    db 0x00         ; base 16:23
    db 0b10010010   ; access byte - data
    db 0x4f         ; flags/(limit 16:19). flag is set to 32 bit protected mode
    db 0x00         ; base 24:31
gdt_end:

times (0x200 - 0x2)-($-$$) db 0
db 0x55
db 0xaa
times (0x000b4000 - 0x200) db 0 ; Fill floppy (Standard 1.44M IBM Floppy)
