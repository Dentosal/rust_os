; SMP AP startup trampoline
; This is where other cores start their execution.
; Stack must not be used here.

%include "build/constants.asm"

%define CODE_SEG     0x0008
%define DATA_SEG     0x0010

[BITS 16]
[ORG 0x2000] ; todo: constantify?

ap_startup:
    jmp 0x0000:.flush_cs   ; reload CS to zero
.flush_cs:

    ; initialize segment registers
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax


    lidt [IDT]                        ; Load a zero length IDT

    ; Enter long mode.
    mov eax, 10100000b                ; Set the PAE and PGE bit.
    mov cr4, eax

    mov edx, PAGE_TABLES_LOCATION     ; Point CR3 at the PML4.
    mov cr3, edx

    mov ecx, 0xC0000080               ; Read from the EFER MSR.
    rdmsr
    or eax, 0x00000900                ; Set the LME & NXE bit.
    wrmsr

    mov ebx, cr0                      ; Activate long mode by enabling
    or ebx,0x80000001                 ; paging and protection simultaneously
    mov cr0, ebx

    lgdt [GDT.Pointer]                ; Load GDT.Pointer defined below.

    jmp CODE_SEG:long_mode             ; Load CS with 64 bit segment and flush the instruction cache


    ; Global Descriptor Table
ALIGN 4
GDT:
.Null:
    dq 0x0000000000000000             ; Null Descriptor - should be present.

.Code:
    dq 0x00209A0000000000             ; 64-bit code descriptor (exec/read).
    dq 0x0000920000000000             ; 64-bit data descriptor (read/write).

ALIGN 4
    dw 0                              ; Padding to make the "address of the GDT" field aligned on a 4-byte boundary

.Pointer:
    dw $ - GDT - 1                    ; 16-bit Size (Limit) of GDT.
    dd GDT                            ; 32-bit Base Address of GDT. (CPU will zero extend to 64-bit)


ALIGN 4
IDT:
    .Length       dw 0
    .Base         dd 0


[BITS 64]
long_mode:
    mov ax, DATA_SEG
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    mov rcx, 1                  ; set rcx = 1 to mark AP cpu
    jmp 0x100_0000              ; TODO: constantify