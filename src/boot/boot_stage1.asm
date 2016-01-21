; RUSTOS LOADER
; STAGE 1

stage1:
    ; SCREEN: top left: "01"
    mov dword [0xb8000], 0x4f524f45
    mov dword [0xb8004], 0x4f3a4f52
    mov dword [0xb8008], 0x4f204f20
    ; jump into rust
    ;call 0x7e00
    jmp $

times (0x200 - 0x2)-($-$$) db 0
db 0x55
db 0xaa
times (0x000b4000 - 0x200) db 0 ; Fill floppy (Standard 1.44M IBM Floppy)
