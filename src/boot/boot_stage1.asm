; RUSTOS LOADER
; STAGE 1
[BITS 32]
[ORG 0x7e00]


stage1:
    ; SCREEN: top left: "01"
    mov dword [0xb8000], 0x2f312f30
    ; jump into rust
    ;call 0x7e00
    jmp $

times (0x200 - 0x2)-($-$$) db 0
db 0x55
db 0xaa
times (0x000b4000 - 0x200) db 0 ; Fill floppy (Standard 1.44M IBM Floppy)
