OUTPUT_FORMAT(elf64-x86-64)
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
    .entry : ALIGN(0x1000) {
        KEEP(*(.entry))
    }

    .text : ALIGN(0x1000) {
        *(.text .text.*)
    }

    .rodata : ALIGN(0x1000) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(0x1000) {
        *(.data .data.*)
    }

    .bss : ALIGN(0x1000) {
        KEEP(*(.bss .bss.*))
    }

    .got : ALIGN(0x1000) {
        *(.got)
    }

    .got.plt : ALIGN(0x1000) {
        *(.got.plt)
    }

    .data.rel.ro : ALIGN(0x1000) {
        *(.data.rel.ro.local*) *(.data.rel.ro .data.rel.ro.*)
    }

    .gcc_except_table : ALIGN(0x1000) {
        *(.gcc_except_table)
    }
}
