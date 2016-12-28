; MASTER BOOT RECORD
; STAGE 0

%include "src/asm_routines/constants.asm"

%define stage1_loadpoint 0x7e00
; locate stage1 at 0x7e00->
%define kernel_loadpoint 0xA000
%define kernel_size_sectors 200
; locate kernel at 0xA000->
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

    ; clear interrupts
    cli

    ; save boot drive
    mov [bootdrive], dl

    ; get memory map
    mov al, 'M'
    call get_memory_map
    jc print_error   ; carry flag set on error

    ; reset ds and es, bios probably changed the values
    xor ax, ax
    mov ds, ax
    mov es, ax

    ; http://wiki.osdev.org/ATA_in_x86_RealMode_(BIOS)#LBA_in_Extended_Mode

    ; test that extended lba reading is enabled
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, 0x80
    int 0x13
    mov al, 'R'
    jc print_error


    ; load sectors
    ; stage 1
    mov dword [da_packet.lba_low],  1
    mov dword [da_packet.lba_high], 0
    mov  word [da_packet.count],    2
    mov  word [da_packet.address],  stage1_loadpoint
    mov  word [da_packet.segment],  0

    mov ah, 0x42
    mov si, da_packet
    mov dl, 0x80        ; FIXME: actual boot device?
    int 0x13
    mov al, 'D'
    jc print_error

    ; kernel
    mov dword [da_packet.lba_low],  3
    mov dword [da_packet.lba_high], 0
    mov  word [da_packet.count],    0x50-3
    mov  word [da_packet.address],  kernel_loadpoint
    mov  word [da_packet.segment],  0
    mov ah, 0x42
    mov si, da_packet
    mov dl, 0x80        ; FIXME: actual boot device?
    int 0x13
    mov al, 'D'
    jc print_error

    mov ecx, ((kernel_size_sectors-(kernel_size_sectors-(0x50-3)))+0x50-1)/0x50+1; ceil((kernel_size_sectors-(0x50-3))/0x50)+1
    mov eax, 0x50   ; note: limited so that last 4 bits are not in use
.load_kernel_loop:
    push cx
        mov dword [da_packet.lba_low],  eax
        mov dword [da_packet.lba_high], 0
        mov  word [da_packet.count],    0x50
        push eax
            ; eax = (eax * 0x200) / 0x10 = eax * 0x20
            imul eax, 0x20
            mov word [da_packet.address], kernel_loadpoint
            mov word [da_packet.segment], ax
        pop eax
        push eax
            mov ah, 0x42
            mov si, da_packet
            mov dl, 0x80        ; FIXME: actual boot device?
            int 0x13
            mov al, 'D'
            jc print_error
        pop eax
        add eax, 0x50
    pop cx
    loop .load_kernel_loop

    ; hide cursor by moving it out of the screen
    mov bh, 0
    mov ah, 2
    mov dl, 100
    mov dh, 100
    int 10h

    ; load protected mode GDT and a null IDT
    cli
    lgdt [gdtr32]
    lidt [idtr32]
    ; set protected mode bit of cr0
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    ; far jump to load CS with 32 bit segment
    jmp 0x08:0x7e00

print_error:    ; prints E and one letter from al and terminates, (error in boot sector 0)
    push ax
        push ax
            mov al, 'E'
            mov ah, 0x0e
            int 0x10
        pop ax
        mov ah, 0x0e
        int 0x10
    pop ax
    jmp $

; disk address packet
ALIGN 2
da_packet:
    db 16               ; size of this packet (constant)
    db 0                ; reserved (always zero)
.count:
    dw 100              ; count (how many sectors) (127 might be a limit here)
.address:
    dw stage1_loadpoint ; offset (where)
.segment:
    dw 0x0000           ; segment
.lba_low:
    dq 1                ; lba low (position on disk)
.lba_high:
    dq 0                ; lba high

; use the INT 0x15, eax= 0xE820 BIOS function to get a memory map
; http://wiki.osdev.org/Detecting_Memory_(x86)#BIOS_Function:_INT_0x15.2C_EAX_.3D_0xE820
; output: bp = entry count, trashes all registers except esi
get_memory_map:
    mov di, (boot_tmp_mmap_buffer+2)
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
	mov [boot_tmp_mmap_buffer], bp ; store the entry count just below the array
	clc                        ; there is "jc" on end of list to this point, so the carry must be cleared
	ret
.failed:
	stc	                       ; "function unsupported" error exit, set carry
	ret


; Constant data

gdtr32:
    dw (gdt32.end - gdt32.begin) + 1    ; size
    dd gdt32                            ; offset

idtr32:
    dw 0
    dd 0

gdt32:  ; from AMD64 system programming manual, page 132
.begin:
    ; null entry
    dq 0
    ; code entry
    dw 0xffff       ; limit 0:15
    dw 0x0000       ; base 0:15
    db 0x00         ; base 16:23
    db 0b10011010   ; access P=1, DPL=00 (ring 0), S=1, TYPE=1010 (code, C=0, R=1 (readable), A=0)
    db 0b01001111   ; flags G=0, D/B=1, RESERVED=0, AVL=0 and limit 16:19 = 0b1111
    db 0x00         ; base 24:31
    ; data entry
    dw 0xffff       ; limit 0:15
    dw 0x0000       ; base 0:15
    db 0x00         ; base 16:23
    db 0b10010010   ; access P=1, DPL=00 (ring 0), S=1, TYPE=0010 (data, E=0, W=1 (writable), A=0)
    db 0b11001111   ; flags G=1 (limit marks 4 KiB blocks instead of 1 Byte), D/B=1, RESERVED=0, AVL=0 and limit 16:19 = 0b1111
    db 0x00         ; base 24:31
.end:



times (0x200 - 0x2)-($-$$) db 0
db 0x55
db 0xaa
