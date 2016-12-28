OUTPUT_FORMAT("elf64-x86-64") /* FLAT BINARY DOES NOT WORK, ELF WORKS */
ENTRY(start)

SECTIONS {
/*
ftp://ftp.gnu.org/old-gnu/Manuals/ld-2.9.1/html_chapter/ld_3.html#SEC10
The dot is the location counter.
This means we are starting from 0x100000, 1MiB.
It MUST match specifications in the plan.md.
*/

. = 0x100000;

/*
All sections are 0x1000 = 4096 = 4KiB aligned, because that is the correct page boundary alignment.

*/

    /* ensure that the kernel entry code is at the beginning */
    .entry :
    {
        *(.entry)
        . = ALIGN(0x1000);
    }

    .text :
    {
        *(.text .text.*)
        . = ALIGN(0x1000);
    }

    .rodata :
    {
        *(.rodata .rodata.*)
        . = ALIGN(0x1000);
    }

    .data :
    {
        *(.data .data.*)
        . = ALIGN(0x1000);
    }

    .bss :
    {
        *(.bss .bss.*)
        . = ALIGN(0x1000);
    }

    .got :
    {
        *(.got)
        . = ALIGN(0x1000);
    }

    .got.plt :
    {
        *(.got.plt)
        . = ALIGN(0x1000);
    }

    .data.rel.ro :
    {
        *(.data.rel.ro.local*) *(.data.rel.ro .data.rel.ro.*)
        . = ALIGN(0x1000);
    }

    .gcc_except_table : ALIGN(4K) {
        *(.gcc_except_table)
    }
}
