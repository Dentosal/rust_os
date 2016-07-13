; IMPORTANT: this file MUST contain only define-constants and macros
; putting any concrete data here WILL cause everything (including bootloader)
; to break

%define gdt 0x5000
%define gdt_selector_zero 0x00
%define gdt_selector_code 0x08
%define gdt_selector_data 0x10


%define idt 0x0
%define idt_size 0x4000
%define idtr 0x4000
