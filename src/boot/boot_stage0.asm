; MASTER BOOT RECORD
; STAGE 0

%define loadpoint 0x8000
; locate kernel at 0x8000->
%define bootdrive 0x7b00
; bootdrive location (1 byte)

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

    ; save boot drive
    mov [bootdrive], dl

    ; get memory map
    mov di, 0x1000  ; buffer 0x1000->
    mov si, mmap_error_msg
    call get_memory_map
    jc print_error   ; carry flag set on error

    ; reset ds and es, bios probably changed the values
    xor ax, ax
    mov ds, ax
    mov es, ax

    ; load more code into 0x7e00 so we can jump to it later
    mov ah, 2       ; read
    mov al, 40      ; 40 sectors (kernel max size is now 39 sectors = 39*512 bytes)
    mov ch, 0       ; cylinder & 0xff
    mov cl, 2       ; sector | ((cylinder >> 2) & 0xc0)
    mov dh, 0       ; head
    mov dl, [bootdrive] ; drive number
    mov bx, (loadpoint - 0x200)  ; read buffer (es:bx) (now next stage is located at (loadpoint - 0x200) and kernel just after that)
    mov si, disk_error_msg
    int 0x13
    jc print_error

    ; hide cursor
    mov bh, 0
    mov ah, 2
    mov dl, 100
    mov dh, 100
    int 10h

    ; load protected mode GDT and a null IDT (we don't need interrupts)
    cli
    lgdt [gdtr32]
    lidt [idtr32]
    ; set protected mode bit of cr0
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    ; far jump to load CS with 32 bit segment
    jmp 0x08:protected_mode


; in: si = pointer to string
print_error:
.loop:
    lodsb
    or al, al
    jz .done
    mov ah, 0x0e
    int 0x10
    jmp .loop
.done:
    jmp $

disk_error_msg db "E: disk", 0
mmap_error_msg db "E: mmap", 0

; use the INT 0x15, eax= 0xE820 BIOS function to get a memory map
; http://wiki.osdev.org/Detecting_Memory_(x86)#BIOS_Function:_INT_0x15.2C_EAX_.3D_0xE820
; inputs: es:di -> destination buffer for 24 byte entries
; outputs: bp = entry count, trashes all registers except esi
get_memory_map:
	xor ebx, ebx               ; ebx must be 0 to start
	xor bp, bp                 ; keep an entry count in bp
	mov edx, 0x0534D4150       ; Place "SMAP" into edx
	mov eax, 0xe820
	mov [es:di + 20], dword 1  ; force a valid ACPI 3.X entry
	mov ecx, 24                ; ask for 24 bytes
	int 0x15
	jc short .failed           ; carry set on first call means "unsupported function"
	mov edx, 0x0534D4150       ; Some BIOSes apparently trash this register?
	cmp eax, edx               ; on success, eax must have been reset to "SMAP"
	jne short .failed
	test ebx, ebx              ; ebx = 0 implies list is only 1 entry long (worthless)
	je short .failed
	jmp short .jmpin
.e820lp:
	mov eax, 0xe820            ; eax, ecx get trashed on every int 0x15 call
	mov [es:di + 20], dword 1  ; force a valid ACPI 3.X entry
	mov ecx, 24                ; ask for 24 bytes again
	int 0x15
	jc short .e820f            ; carry set means "end of list already reached"
	mov edx, 0x0534D4150       ; repair potentially trashed register
.jmpin:
	jcxz .skipent              ; skip any 0 length entries
	cmp cl, 20                 ; got a 24 byte ACPI 3.X response?
	jbe short .notext
	test byte [es:di + 20], 1  ; if so: is the "ignore this data" bit clear?
	je short .skipent
.notext:
	mov ecx, [es:di + 8]       ; get lower uint32_t of memory region length
	or ecx, [es:di + 12]       ; "or" it with upper uint32_t to test for zero
	jz .skipent                ; if length uint64_t is 0, skip entry
	inc bp                     ; got a good entry: ++count, move to next storage spot
	add di, 24
.skipent:
	test ebx, ebx              ; if ebx resets to 0, list is complete
	jne short .e820lp
.e820f:
	mov [0x1000-2], bp         ; store the entry count just below the array
	clc                        ; there is "jc" on end of list to this point, so the carry must be cleared
	ret
.failed:
	stc	                       ; "function unsupported" error exit, set carry
	ret


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
    mov esp, 0x7c00 ; stack grows downwards

    ; SCREEN: top left: "00"
    mov dword [0xb8000], 0x2f302f30

    call enable_A20
    call check_long_mode
    call set_up_SSE


    ; SCREEN: top left: "01"
    mov dword [0xb8000], 0x2f302f31


    ; jump into stage 1
    jmp 0x7e00


; http://wiki.osdev.org/A20_Line
; Using only "Fast A20" gate
; Might be a bit unreliable, but it is small :]
enable_A20:
    in al, 0x92
    test al, 2
    jnz .done
    or al, 2
    and al, 0xFE
    out 0x92, al
.done:
    ret


; Check for SSE and enable it.
; http://os.phil-opp.com/set-up-rust.html#enabling-sse
; http://wiki.osdev.org/SSE
set_up_SSE:
    ; check for SSE
    mov eax, 0x1
    cpuid
    test edx, 1<<25
    jz .SSE_missing

    ; enable SSE
    mov eax, cr0
    and ax, 0xFFFB      ; clear coprocessor emulation CR0.EM
    or ax, 0x2          ; set coprocessor monitoring  CR0.MP
    mov cr0, eax
    mov eax, cr4
    or ax, 3 << 9       ; set CR4.OSFXSR and CR4.OSXMMEXCPT at the same time
    mov cr4, eax

    ret
.SSE_missing:
    ; error: no SSE: "!S"
    mov al, '!'
    mov ah, 'S'
    jmp error

; http://wiki.osdev.org/Setting_Up_Long_Mode#x86_or_x86-64
; Just assumes that cpuid is available (processor is released after 1993)
check_long_mode:
    mov eax, 0x80000000    ; Set the A-register to 0x80000000.
    cpuid                  ; CPU identification.
    cmp eax, 0x80000001    ; Compare the A-register with 0x80000001.
    jb .no_long_mode       ; It is less, there is no long mode.
    mov eax, 0x80000001    ; Set the A-register to 0x80000001.
    cpuid                  ; CPU identification.
    test edx, 1 << 29      ; Test if the LM-bit is set in the D-register.
    jz .no_long_mode       ; They aren't, there is no long mode.
    ret
.no_long_mode:
    ; error: no long mode: "!L"
    mov al, '!'
    mov ah, 'L'
    jmp error


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


; Constant data

gdtr32:
    dw (gdt32.end - gdt32.begin) + 1    ; size
    dd gdt32                            ; offset

idtr32:
    dw 0
    dd 0

gdt32:
.begin:
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
.end:

times (0x200 - 0x2)-($-$$) db 0
db 0x55
db 0xaa
times (0x10000 - 0x0200) db 0 ; Smaller image
;times (0x000b4000 - 0x200) db 0 ; Fill floppy (Standard 1.44M IBM Floppy)
