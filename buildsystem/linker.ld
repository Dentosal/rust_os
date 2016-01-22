ENTRY(start)

SECTIONS {
  . = 1M;

  .entry :
  {
    /* ensure that the kernel entry code is at the beginning */
    *(.entry)
  }

  .rodata : ALIGN(4K)
  {
    *(.rodata .rodata.*)
  }

  .text : ALIGN(4K)
  {
    *(.text .text.*)
  }

  .data : ALIGN(4K)
  {
    *(.data .data.*)
  }

  .data.rel.ro : ALIGN(4K) {
    *(.data.rel.ro.local*) *(.data.rel.ro .data.rel.ro.*)
  }
}
