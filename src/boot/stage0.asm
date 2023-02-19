; MASTER BOOT RECORD
; STAGE 0

%include "build/constants.asm"

; locate stage1 at 0x7e00->
%define stage1_loadpoint 0x7e00

; 0x7f is max for most platforms, including Qemu
%define sectors_per_operation 1
;0x20

; disk load buffer for kernel
%define disk_load_buffer 0xa000
%define disk_load_buffer_size (sectors_per_operation * BOOT_DISK_SECTOR_SIZE)

; bootdrive location (1 byte)
%define bootdrive 0x7b00

; checks
%if (disk_load_buffer + disk_load_buffer_size) > BOOT_KERNEL_LOADPOINT
%define qq (disk_load_buffer + disk_load_buffer_size)
%fatal "Disk load buffer overlaps with kernel load point"
%endif

[BITS 16]
[ORG 0x7c00]

boot:
    ; clear interrupts
    cli

    ; initialize segment registers
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax

    ; initialize stack
    mov sp, 0x7c00

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

    ; save kernel/initrd split and end sectors
    mov eax, [initrd_split]
    mov [BOOT_TMP_KERNEL_SPLIT_ADDR], eax
    mov eax, [initrd_end]
    mov [BOOT_TMP_KERNEL_END_ADDR], eax

    ; Test that extended lba reading is enabled
    ; http://wiki.osdev.org/ATA_in_x86_RealMode_(BIOS)#LBA_in_Extended_Mode
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, 0x80
    clc
    int 0x13
    mov al, 'R'
    jc print_error

    ; Enable A20
    call enable_A20


    ; Enter Big Unreal Mode
    ; https://wiki.osdev.org/Unreal_mode#Big_Unreal_Mode
    push ds ; Save real mode
    lgdt [gdtr_unreal]

    ; Switch to protected mode
    mov  eax, cr0
    or al, 1
    mov  cr0, eax

    jmp $+2                ; Tell 386/486 to not crash

    mov  bx, 0x08          ; Select descriptor 1
    mov  ds, bx

    ; Switch back to real mode
    and al, 0xFE
    mov cr0, eax
    pop ds ; Restore old segment

    ; Load sectors
    ; Rest of the bootloader (da_packet already set up)
    mov ah, 0x42
    mov si, da_packet
    mov dl, [bootdrive]
    int 0x13
    mov al, 'D'
    jc print_error

    jmp .afterkernel

    ; Load the kernel
    mov ecx, BOOTLOADER_SECTOR_COUNT ; LBA, first sector

.load_loop:

    push ecx
        ; Load from disk
        mov dword [da_packet.lba_low],  ecx
        ; mov dword [da_packet.lba_high], 0 ; These are already true
        mov  word [da_packet.count],    sectors_per_operation
        mov  word [da_packet.address],  disk_load_buffer
        ; mov  word [da_packet.segment],  0 ; These are already true

        mov ah, 0x42
        mov si, da_packet
        mov dl, [bootdrive]
        int 0x13
        mov al, '*'
        jc print_error

        ; Copy to correct position
        ; BOOT_KERNEL_LOADPOINT + (ecx - BOOTLOADER_SECTOR_COUNT) * sector_size
        mov edi, ecx
        sub edi, BOOTLOADER_SECTOR_COUNT
        shl edi, 9 ; multiply by sector size (2**9 = 512 = 0x200)
        add edi, BOOT_KERNEL_LOADPOINT

        mov esi, disk_load_buffer
        mov ecx, disk_load_buffer_size
        .copyloop:
            mov al, [esi]
            mov [edi], al
            inc esi
            inc edi
            loop .copyloop, ecx

    pop ecx

    ; Test if all loaded
    add ecx, sectors_per_operation
    mov edx, [initrd_end]
    add edx, BOOTLOADER_SECTOR_COUNT


    cmp ecx, edx
    jle .load_loop
.afterkernel:

    ; hide cursor by moving it out of the screen
    mov bh, 0
    mov ah, 2
    mov dx, 0xFFFF
    int 0x10

    ; load protected mode GDT and a null IDT
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

; disk address packet, set up for the first transfer (rest of the bootloacder)
ALIGN 4
da_packet:
    db 16               ; size of this packet (constant)
    db 0                ; reserved (always zero)
.count:
    dw (BOOTLOADER_SECTOR_COUNT - 1)    ; count (how many sectors)
.address:                               ; ^ (127 might be a limit here, still 0xFF on most BIOSes)
    dw stage1_loadpoint ; offset (where)
.segment:
    dw 0                ; segment
.lba_low:
    dq 1                ; lba low (position on disk)
.lba_high:
    dq 0                ; lba high

; use the INT 0x15, eax= 0xE820 BIOS function to get a memory map
; http://wiki.osdev.org/Detecting_Memory_(x86)#BIOS_Function:_INT_0x15.2C_EAX_.3D_0xE820
; output: bp = entry count, trashes all registers except esi
get_memory_map:
    mov di, (BOOT_TMP_MMAP_BUFFER+2)
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
	mov [BOOT_TMP_MMAP_BUFFER], bp ; store the entry count just below the array
	clc                        ; there is "jc" on end of list to this point, so the carry must be cleared
	ret
.failed:
	stc	                       ; "function unsupported" error exit, set carry
	ret


; Constant data

; GDT for protected mode
gdtr32:
    dw gdt32.end - gdt32.begin - 1  ; size
    dd gdt32.begin                  ; offset

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

; GDT for big unreal mode
gdtr_unreal:
   dw gdt_unreal.end - gdt_unreal.begin - 1
   dd gdt_unreal.begin

gdt_unreal:
.begin:     dd 0, 0
.flatdesc:  db 0xff, 0xff, 0, 0, 0, 10010010b, 11001111b, 0
.end:


times (0x200 - 10) - ($ - $$) db 0
initrd_split: dd 0xd7cafed7 ; placeholder: d7initrd start
initrd_end:   dd 0xd7cafed7 ; placeholder: d7initrd end
dw 0xaa55 ; Boot signature
