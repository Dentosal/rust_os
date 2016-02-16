; MASTER BOOT RECORD
; STAGE 0

%define loadpoint 0x8000
; now kernel is located at range(0x8000, 0x9c00)

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
    mov al, 20      ; 20 sectors (kernel max size is now 19 sectors = 19*512 bytes)
    mov ch, 0       ; cylinder & 0xff
    mov cl, 2       ; sector | ((cylinder >> 2) & 0xc0)
    mov dh, 0       ; head
    mov bx, (loadpoint - 0x200)  ; read buffer (now next stage is located at (loadpoint - 0x200) and kernel just after that)
    int 0x13
    jc load_error
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

load_error:
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
; Page tables
%define page_table_section_start 0x00020000
%define page_table_p4 0x00020000
%define page_table_p3 0x00021000
%define page_table_p2 0x00022000
%define page_table_section_end 0x00023000



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
    call set_up_page_tables
    call enable_paging
    call set_up_SSE


    ; SCREEN: top left: "01"
    mov dword [0xb8000], 0x2f302f31


    ; jump into stage 1
    call 0x7e00


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

; set up paging
; http://os.phil-opp.com/entering-longmode.html#set-up-identity-paging
; http://wiki.osdev.org/Paging
; http://pages.cs.wisc.edu/~remzi/OSTEP/vm-paging.pdf
; Identity map first 1GiB (0x200000 * 0x200)
; using 2MiB pages
set_up_page_tables:
    ; map first P4 entry to P3 table
    mov eax, page_table_p3
    or eax, 0b11 ; present & writable
    mov [page_table_p4], eax

    ; map first P3 entry to P2 table
    mov eax, page_table_p2
    or eax, 0b11 ; present & writable
    mov [page_table_p3], eax

    ; map each P2 entry to a huge 2MiB page
    mov ecx, 0         ; counter

.map_page_table_p2_loop:
    ; map ecx-th P2 entry to a huge page that starts at address 2MiB*ecx
    mov eax, 0x200000                   ; 2MiB
    mul ecx                             ; page[ecx] start address
    or eax, 0b10000011                  ; present & writable & huge
    mov [page_table_p2 + ecx * 8], eax  ; map entry

    inc ecx
    cmp ecx, 0x200                  ; is the whole P2 table is mapped?
    jne .map_page_table_p2_loop     ; next entry
    ; done
    ret

; enable_paging
; http://os.phil-opp.com/entering-longmode.html#enable-paging
; http://wiki.osdev.org/Paging#Enabling
enable_paging:
    ; load P4 to cr3 register (cpu uses this to access the P4 table)
    mov eax, page_table_p4
    mov cr3, eax

    ; enable PAE-flag in cr4 (Physical Address Extension)
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; set the long mode bit in the EFER MSR (model specific register)
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    ; enable paging in the cr0 register
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax
    ret



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
