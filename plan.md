Dimension 7 Internals "Documentation"/"Plan"
============================================

Bootable Disk Layout
====================

Begin | Size  | Content
------|-------|--------
    0 |   200 | Stage 0 (Master boot record) / `boot_stage0`
  200 |   200 | Stage 1 / `boot_stage1`
  400 |   200 | Stage 2 / `boot_stage2`
  600 |     ? | Kernel (and file system?)


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
  A000|   4000| Disk load buffer
 10000|      ?| Kernel ELF image (Boot stage only) (size proabably around 0x40000)
 60000|   3000| Page tables (Boot stage only)
 70000|  10000| Memory bitmap 1 (currently free memory) (this allows (8*0x10000*0x1000)/1024**3 = 2GiB memory)
 80000|  10000| Memory bitmap 2 (hardware memory status)
100000|      ?| Relocated and expanded kernel from ELF image (will be huge)

Final layout
------------

Begin   | Size  | Content
--------|-------|--------
       0|   1000| IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
    1000|    100| IDTR (10 bytes used, and after that reserved)
    1100|    100| GDT (some used, and after that reserved)
   70000|  10000| Memory bitmap 1 (currently free memory) (this allows (8*0x10000*0x1000)/1024**3 = 2GiB memory)
   80000|  10000| Memory bitmap 2 (hardware memory status)
   90000|      ?| Reserved for EBDA, ROM, Video Memory and other stuff there.
  100000|      ?| Kernel (Extended memory) (Size around 0x100000)
       ?|      ?| Free memory (must be allocated using the frame allocator)
40000000|      ?| Allocator-managed memory (This is 1GiB)

TODO: Bump allocator
