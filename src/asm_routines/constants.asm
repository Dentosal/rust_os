; IMPORTANT: this file MUST contain only define-constants and macros
; putting any concrete data here WILL cause everything (including bootloader)
; to break

; These constants MUST match the ones in plan.md
; If a constant defined here doesn't exists in that file, then it shpuld be fine too

; Kernel elf executable initial load point
%define loadpoint 0x10000

; GDT
%define gdt 0x1100
%define gdt_selector_zero 0x00
%define gdt_selector_code 0x08
%define gdt_selector_data 0x10

; IDT
%define idt 0x0
%define idt_size 0x1000
%define idtr (idt+idt_size)

; Temporary memory map
%define boot_tmp_mmap_buffer 0x2000

; Page tables
%define page_table_section_start    0x00060000
%define page_table_p4               page_table_section_start
%define page_table_p3               page_table_section_start + 0x1000
%define page_table_p2               page_table_section_start + 0x2000
%define page_table_section_end      page_table_section_start + 0x2000
