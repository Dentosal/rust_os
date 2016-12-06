; IMPORTANT: this file MUST contain only define-constants and macros
; putting any concrete data here WILL cause everything (including bootloader)
; to break

; These constants MUST match the ones in plan.md
; If a constant defined here doesn't exists in that file, then it's also fine

%define gdt 0x1100
%define gdt_selector_zero 0x00
%define gdt_selector_code 0x08
%define gdt_selector_data 0x10

%define idt 0x0
%define idt_size 0x1000
%define idtr (idt+idt_size)

%define boot_tmp_mmap_buffer 0x2000

; Page tables
%define page_table_section_start 0x00020000
%define page_table_p4 0x00020000
%define page_table_p3 0x00021000
%define page_table_p2 0x00022000
%define page_table_section_end 0x00023000
