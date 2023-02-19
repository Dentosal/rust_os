; RUSTOS LOADER
; STAGE 1

%include "build/constants.asm"

[BITS 32]
[ORG 0x7e00]

stage1:
    ; load all the other segments than cs (it's already set by jumping) with 32 bit data segments
    mov eax, 0x10
    mov ds, eax
    mov es, eax
    mov fs, eax
    mov gs, eax
    mov ss, eax

    ; set up stack
    mov esp, 0x7c00 ; stack grows downwards

    ; SCREEN: clear screen
    mov ecx, (25 * 80 * 2) ; / 4
.clear_screen_lp:
    mov eax,  ecx
    shl eax, 2 ; multiply by 4
    add eax, 0xb8000
    mov dword [eax], 0x00200020
    loop .clear_screen_lp

    call check_long_mode
    call set_up_SSE

    ; SCREEN: top left: "01"
    mov dword [0xb8000], 0x2f312f30

    ; paging
    call set_up_page_tables
    call enable_paging

    ; SCREEN: top left: "02"
    mov dword [0xb8000], 0x2f322f30

    ; relocate GDT
    mov esi, tmp_gdt64  ; from
    mov edi, GDT_ADDR   ; to
    mov ecx, 8*3+12     ; size (no pointer)
    rep movsb           ; copy

    ; load the new GDT
    lgdt [GDT_ADDR + 8*3]

    ; update selectors
    mov ax, GDT_SELECTOR_DATA
    mov ss, ax
    mov ds, ax
    mov es, ax

    ; jump into stage 2, and activate long mode
    jmp GDT_SELECTOR_CODE:0x8000

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


; set up paging
; http://os.phil-opp.com/entering-longmode.html#set-up-identity-paging
; http://wiki.osdev.org/Paging
; http://pages.cs.wisc.edu/~remzi/OSTEP/vm-paging.pdf
; Identity map first 1GiB (0x200000 * 0x200)
; using 2MiB pages
set_up_page_tables:
    ; map first P4 entry to P3 table
    mov eax, BOOT_PAGE_TABLE_P3
    or eax, 0b11 ; present & writable
    mov [BOOT_PAGE_TABLE_P4], eax

    ; map first P3 entry to P2 table
    mov eax, BOOT_PAGE_TABLE_P2
    or eax, 0b11 ; present & writable
    mov [BOOT_PAGE_TABLE_P3], eax

    ; map each P2 entry to a huge 2MiB page
    mov ecx, 0         ; counter

.map_page_table_p2_loop:
    ; map ecx-th P2 entry to a huge page that starts at address 2MiB*ecx
    mov eax, 0x200000                       ; 2MiB
    mul ecx                                 ; page[ecx] start address
    or eax, 0b10000011                      ; present & writable & huge
    mov [BOOT_PAGE_TABLE_P2 + ecx * 8], eax ; map entry

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
    mov eax, BOOT_PAGE_TABLE_P4
    mov cr3, eax

    ; enable PAE-flag in cr4 (Physical Address Extension)
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; set the long mode bit in the EFER MSR (model specific register)
    mov ecx, 0xC0000080 ; EFER
    rdmsr               ; read
    or eax, 1 << 8      ; set bit
    wrmsr               ; write

    ; enable paging in the cr0 register
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    ret

; constant data

; GDT (Global Descriptor Table)
tmp_gdt64:
    dq 0 ; zero entry
    dq (1<<44) | (1<<47) | (1<<41) | (1<<43) | (1<<53) ; code segment
    dq (1<<44) | (1<<47) | (1<<41) ; data segment
.pointer:       ; GDTR
    dw 8*3      ; size
    dq GDT_ADDR ; pointer

times (0x200-($-$$)) db 0 ; fill a sector
