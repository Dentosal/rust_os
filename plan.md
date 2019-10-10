Dimension 7 Internals "Documentation"/"Plan"
============================================

Bootable Disk Layout
====================

Begin | Size  | Content
------|-------|--------
    0 |   200 | Stage 0 (Master boot record) / `boot_stage0`
  200 |   200 | Stage 1 / `boot_stage1`
  400 |   200 | Stage 2 / `boot_stage2`
  600 |     ? | Kernel
    ? |     ? | Filesystem


Kernel Memory Layout
====================

Boot / Intermediate
-------------------

Begin | Size  | Content
------|-------|--------
     0|   1000| IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
  1000|    100| IDTR (10 bytes used, and after that reserved)
  1100|    100| GDT (some used, and after that reserved)
  2000|   1000| Boot stage memory map from BIOS (some used, and after that reserved)
  4000|   1000| Bootloader ELF decompression tables (about 0x500 used, and after that reserved)
  8000|    400| Stage 2 bootloader (two sectors atm)
  A000|   4000| Disk load buffer
 10000|      ?| Kernel ELF image (Boot stage only) (size proabably around 0x40000)
 60000|   3000| Page tables (Boot stage only)
100000|      ?| Relocated and expanded kernel from ELF image (will be huge)

Final layout
------------

Begin   | Size  | Content
--------|-------|--------
       0|   1000| IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
    1000|    100| IDTR (10 bytes used, and after that reserved)
    1100|    100| GDT (some used, and after that reserved)
   20000|  50000| DMA / VirtIO memory buffers (requires "low" memory)
   70000|  20000| *reserved*
   90000|      ?| Reserved for EBDA, ROM, Video Memory and other stuff there.
  100000|      ?| Kernel (Extended memory) (Size around 0x200000)
       ?|      ?| Free memory (must be allocated using the frame allocator)
40000000|      ?| Allocator-managed memory (This is 1GiB)

TODO: Bump allocator

Virtual address space
----------------------

TODO: Higher half kernel
TODO: Proper virtual memory map

Begin   | Size  | Content
--------|-------|---------
10000000|      ?| Page tables


# Interrupts

Numbers     | Description
------------|-------------
0x00..=0x1f | Standard intel interrupts
0x20..=0x2f | PIC interrupts