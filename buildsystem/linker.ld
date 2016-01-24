OUTPUT_FORMAT("elf64-x86-64") /* FLAT BINARY DOES NOT WORK, ELF WORKS */
ENTRY(start)

SECTIONS {
  . = 0x0000000000010000;

  .entry :
  {
    /* ensure that the kernel entry code is at the beginning */
    *(.entry)
  }
  .rodata : ALIGN(0x1000)
  {
    *(.rodata .rodata.*)
  }
  .text : ALIGN(0x1000)
  {
    *(.text .text.*)
  }

  .data : ALIGN(0x1000)
  {
    *(.data .data.*)
  }

  .data.rel.ro : ALIGN(0x1000) {
    *(.data.rel.ro.local*) *(.data.rel.ro .data.rel.ro.*)
  }
}
