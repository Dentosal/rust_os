; RUSTOS LOADER
; STAGE 2

%include "src/asm_routines/constants.asm"

[BITS 64]
[ORG 0x8000]

stage2:
    cli

    ; update segments
    mov dx, GDT_SELECTOR_DATA
    mov ss, dx  ; stack segment
    mov ds, dx  ; data segment
    mov es, dx  ; extra segment
    mov fs, dx  ; f-segment
    mov gs, dx  ; g-segment

    ; SCREEN: top left: "03"
    mov dword [0xb8000], 0x2f332f30

    ; parse and load kernel, an ELF executable "file"
    ; http://wiki.osdev.org/ELF#Loading_ELF_Binaries

    ; elf error messages begin with "E"
    mov al, 'E'

    ; magic number 0x7f+'ELF'
    ; if not elf show error message "E!"
    mov ah, '!'
    cmp dword [BOOT_KERNEL_LOADPOINT], 0x464c457f
    jne error

    ; bitness and instruction set (must be 64, so values must be 2 and 0x3e) (error code: "EB")
    mov ah, 'B'
    cmp byte [BOOT_KERNEL_LOADPOINT + 4], 0x2
    jne error
    cmp word [BOOT_KERNEL_LOADPOINT + 18], 0x3e
    jne error

    ; endianess (must be little endian, so value must be 1) (error code: "EE")
    mov ah, 'E'
    cmp byte [BOOT_KERNEL_LOADPOINT + 5], 0x1
    jne error

    ; elf version (must be 1) (error code: "EV")
    mov ah, 'V'
    cmp byte [BOOT_KERNEL_LOADPOINT + 0x0006], 0x1
    jne error

    ; Now lets trust it's actually real and valid elf file

    ; kernel entry position must be correct
    ; (error code : "Ep")
    mov ah, 'p'
    cmp qword [BOOT_KERNEL_LOADPOINT + 24], KERNEL_LOCATION
    jne error

    ; load point is correct, great. print green OK
    mov dword [0xb8000 + 80*2], 0x2f4b2f4f

    ; Parse program headers
    ; http://wiki.osdev.org/ELF#Program_header
    ; (error code : "EH")
    mov ah, 'H'

    ; We know that program header size is 56 (=0x38) bytes
    ; still, lets check it:
    cmp word [BOOT_KERNEL_LOADPOINT + 54], 0x38
    jne error


    ; program header table position
    mov rbx, qword [BOOT_KERNEL_LOADPOINT + 32]
    add rbx, BOOT_KERNEL_LOADPOINT ; now rbx points to first program header

    ; length of program header table
    mov rcx, 0
    mov cx, [BOOT_KERNEL_LOADPOINT + 56]

    mov ah, '_'
    ; loop through headers
.loop_headers:
    ; First, lets check that this segment should be loaded

    cmp dword [rbx], 1 ; load: this is important
    jne .next   ; if not important: continue

    ; load: clear p_memsz bytes at p_vaddr to 0, then copy p_filesz bytes from p_offset to p_vaddr
    push rcx

    ; esi = p_offset
    mov rsi, [rbx + 8]
    add rsi, BOOT_KERNEL_LOADPOINT  ; now points to begin of buffer we must copy

    ; rdi = p_vaddr
    mov rdi, [rbx + 16]

    ; rcx = p_memsz
    mov rcx, [rbx + 40]

    ; <1> clear p_memsz bytes at p_vaddr to 0
    push rdi
.loop_clear:
    mov byte [rdi], 0
    inc rdi
    loop .loop_clear
    pop rdi
    ; </1>

    ; rcx = p_filesz
    mov rcx, [rbx + 32]

    ; <2> copy p_filesz bytes from p_offset to p_vaddr
    ; uses: rsi, rdi, rcx
    rep movsb
    ; </2>

    pop rcx

.next:
    add rbx, 0x38   ; skip entry (0x38 is entry size)
    loop .loop_headers

    mov ah, '-'

    ; ELF relocation done
.over:

    ; looks good, going to jump to kernel entry
    ; prints green "JK" for "Jump to Kernel"
    mov dword [0xb8000 + 80*4], 0x2f6b2f6a

    jmp KERNEL_LOCATION ; jump to kernel

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


times (0x200-($-$$)) db 0 ; fill a sector
