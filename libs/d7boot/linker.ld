OUTPUT_FORMAT(elf64-x86-64)
ENTRY(start)

SECTIONS {
    . = 0x8000;

    /* ensure that the bootloader entry code is at the beginning */
    .entry : ALIGN(0x8) {
        KEEP(*(.entry))
    }

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
