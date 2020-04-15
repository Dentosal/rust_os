OUTPUT_FORMAT(elf64-x86-64)
ENTRY(start)

SECTIONS {
    . = 0x1000000;

    /* All sections are 0x200000 = 0x200_000 = 2MiB aligned,
    ** as that is the correct huge-page boundary alignment.
    */

    .text : ALIGN(0x200000) {
        /* ensure that the kernel entry code is at the beginning */
        KEEP(*(.entry))
        *(.text .text.*)
    }

    .rodata : ALIGN(0x200000) {
        KEEP(*(.rodata .rodata.*))
    }

    /* BSS contains stack, so in case of overflow (growing downwards) it
    ** tries to write to .rodata section, which is conveniently read-only.
    */
    .bss : ALIGN(0x200000) {
        *(.bss .bss.*)
    }

    .data : ALIGN(0x200000) {
        *(.data .data.*)
    }
}
