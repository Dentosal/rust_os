OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

SECTIONS {
    /* Load point for programs is 0x1_000_000 */
    . = 0x1000000;

    /*
    All sections are 0x200000 = 0x200_000 = 2MiB aligned,
    as that is the correct huge-page boundary alignment.
    */

    . = ALIGN(0x200000);
    .text : {
        *(.text .text.*)
    }

    . = ALIGN(0x200000);
    .rodata : {
        KEEP(*(.rodata .rodata.*))
    }

    . = ALIGN(0x200000);
    .bss (NOLOAD) : {
        *(.bss .bss.*)
    }

    . = ALIGN(0x200000);
    .data : {
        *(.data .data.*)
    }

}
