; RUSTOS LOADER
; STAGE 1


; Kernel elf executable initial load point
%define loadpoint 0x8000


[BITS 32]
[ORG 0x7e00]


stage1:
    ; SCREEN: top left: "11"
    mov dword [0xb8000], 0x2f312f31

    ; SCREEN: top2 left: "12"
    mov dword [0xb8000 + 2*80], 0x2f312f32

    mov ecx, 0xBEEF0001

    ; parse elf header and relocate kernel
    ; http://wiki.osdev.org/ELF#Tables
    ; elf error messages begin with "E"
    mov al, 'E'

    ; magic number 0x7f+'ELF'
    ; if not elf show error message "E!"
    mov ah, '!'
    cmp dword [loadpoint + 0], 0x464c457f
    jne error

    ; bitness and instrucion set (must be 64, so values must be 2 and 0x3e) (error code: "EB")
    mov ah, 'B'
    cmp byte [loadpoint + 4], 0x2
    jne error
    cmp word [loadpoint + 18], 0x3e
    jne error

    ; endianess (must be little endian, so value must be 1) (error code: "EE")
    mov ah, 'E'
    cmp byte [loadpoint + 5], 0x1
    jne error

    ; elf version (must be 2) (error code: "EV")
    mov ah, 'V'
    cmp byte [loadpoint + 0x0006], 0x2


    ; Now lets trust it's actually real and valid elf file


    ; kernel entry position must be 0x_00000000_00010000
    ; (error code : "EP")
    mov ah, 'P'
    cmp dword [loadpoint + 24], 0x00010000
    jne error
    cmp dword [loadpoint + 28], 0x00000000
    jne error

    ; load point is correct, great. print green OK
    mov dword [0xb8000 + 80*24], 0x2f4b2f4f

    ; Relocate elf image to new position
    mov esi, loadpoint
    mov edi, 0x00010000
    cld ; copy forward
    mov ecx, (14 * 0x200)   ; image max size
    rep movsb   ; https://en.wikibooks.org/wiki/X86_Assembly/Data_Transfer#Move_String


    ; determine point to jump
    ;mov ebx, dword [0x00010000 + 32]    ; edx = Program header table position
    ; first entry in table is first section (our entry section!)
    ;mov edi, 0x00010000
    ;add edi, dword [ebx + 8]    ; p_offset

    ; going to byte bytes mode (8*8 = 2**6 = 64 bits = Long mode)

    ; load GDT
    lgdt [gdt64.pointer]

    ; Now we are in some kind of compatibility mode
    ; Don't do anything else that update selectors and jump
    ; (I think memory access will fail)

    ; update selectors
    mov dx, gdt64.data
    mov ss, dx  ; stack selector
    mov ds, dx  ; data selector
    mov es, dx  ; extra selector

    mov edx, 0xCAFE

    ; jump into kernel entry (relocated to 0x00010000)
    jmp gdt64.code:0x00011000


; Prints `ERR: ` and the given 2-character error code to screen (TL) and hangs.
; args: ax=(al,ah)=error_code (2 characters)
error:
    mov dword [0xb8000], 0x4f524f45
    mov dword [0xb8004], 0x4f3a4f52
    mov dword [0xb8008], 0x4f204f20
    mov dword [0xb800a], 0x4f204f20
    mov byte  [0xb800a], al
    mov byte  [0xb800c], ah
    hlt

; Convert dl to it's ascii hex representation and set color to black/white
;ReprHex:
;	push ax
;	push cx
;
;    mov	al, 0x0F    ; Color: black/white
;    and	al, dl
;    ; convert al to ascii hex (four instructions)
;    add	al, 0x90
;    daa
;    adc	al, 0x40
;    daa
;
;    mov dl, al
;    pop cx
;    pop ax
;    ret


; Constant data section

; GDT (Global Descriptor Table)
gdt64:
    dq 0 ; zero entry
.code: equ $ - gdt64
    dq (1<<44) | (1<<47) | (1<<41) | (1<<43) | (1<<53) ; code segment
.data: equ $ - gdt64
    dq (1<<44) | (1<<47) | (1<<41) ; data segment
.pointer:   ; pointer "struct"
    dw $ - gdt64 - 1
    dq gdt64


times (0x200-($-$$)) db 0 ; fill sector
