OUTPUT_FORMAT(elf64-x86-64)
ENTRY(main)

SECTIONS {
    . = 0;

    /*
    All sections are 0x1000 = 4096 = 4KiB aligned, because that is the correct page boundary alignment.
    */

    .text : ALIGN(0x8) {
        *(.text .text.*)
    }

    .rodata : ALIGN(0x8) {
        KEEP(*(.rodata .rodata.*))
    }

    .data : ALIGN(0x8) {
        *(.data .data.*)
    }

    .bss : ALIGN(0x8) {
        *(.bss .bss.*)
    }
}
