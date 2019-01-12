OUTPUT_FORMAT(elf64-x86-64)
ENTRY(start)

SECTIONS {
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
        KEEP(*(.rodata .rodata.*))
    }

    .data : ALIGN(0x1000) {
        *(.data .data.*)
    }

    .bss : ALIGN(0x1000) {
        *(.bss .bss.*)
    }
}
