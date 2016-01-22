; RUSTOS LOADER
; STAGE 1


; Page tables (because cant use resb in flat binary)
; this must
%define page_table_section_start 0x00020000
%define page_table_p4 0x00020000
%define page_table_p3 0x00021000
%define page_table_p2 0x00022000
%define page_table_section_end 0x00023000


[BITS 32]
[ORG 0x7e00]


stage1:
    ; SCREEN: top left: "01"
    mov dword [0xb8000], 0x2f312f30
    mov ecx, 0xBEEF0001

    ; "high-level" code block follows
    call enable_A20
    call check_long_mode
    call set_up_page_tables
    call enable_paging
    call set_up_SSE
    ; end of "high-level" code block


    ; SCREEN: top left: "2 "
    mov dword [0xb8000], 0x2f202f32


    ; going to byte bytes mode (8*8 = 2**6 = 64 bits = Long mode)
    mov edx, [0x9000]
    mov ecx, 0xBEEF00FF

    ; load GDT
    lgdt [gdt64.pointer]

    ; update selectors
    mov ax, gdt64.data
    mov ss, ax  ; stack selector
    mov ds, ax  ; data selector
    mov es, ax  ; extra selector



    ; SCREEN: top left: "23"
    ;mov dword [0xb8000], 0x2f332f32

    mov ecx, 0xBEEF0001

    ; jump into kernel entry
    jmp gdt64.code:0x9000


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
    cmp ecx, 512                ; is the whole P2 table is mapped?
    jne .map_page_table_p2_loop ; next entry
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
