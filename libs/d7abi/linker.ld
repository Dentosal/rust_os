OUTPUT_FORMAT(elf64-x86-64)
ENTRY(main)

SECTIONS {
    /* Load point for programs is 0x1_000_000 */
    . = 0x1000000;

    /*
    All sections are 0x200000 = 0x200_000 = 2MiB aligned,
    as that is the correct huge-page boundary alignment.
    */

    .text : ALIGN(0x200000) {
        *(.text .text.*)
    }

    .rodata : ALIGN(0x200000) {
        KEEP(*(.rodata .rodata.*))
    }

    .data : ALIGN(0x200000) {
        *(.data .data.*)
    }

    .bss : ALIGN(0x200000) {
        *(.bss .bss.*)
    }
}
