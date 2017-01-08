Dimension 7 Internals "Documentation"/"Plan"
============================================

Bootable Disk Layout
====================

Begin   | Size  | Content
--------|-------|--------
0x0     |0x200  | Stage 1 (Master boot record) / `boot_stage0`
0x200   |0x400  | Stage 2 / `boot_stage1`
0x600   |???????| Kernel (and file system?)


Kernel Memory Layout
====================

Boot / Intermediate
-------------------

Begin   | Size  | Content
--------|-------|--------
0x0     |0x1000 | IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
0x1000  |0x100  | IDTR (10 bytes used, and after that reserved)
0x1100  |0x100  | GDT (some used, and after that reserved)
0x2000  |0x1000 | Boot stage memory map from BIOS (some used, and after that reserved)
0xA000  |???????| Kernel ELF image (Boot stage only) (size proabably around 0x20000)
0x60000 |0x3000 | Page tables (Boot stage only)
0x70000 |0x10000| Memory bitmap 1 (currently free memory) (this allows (8*0x10000*0x1000)/1024**3 = 2GiB memory to be used, I think)
0x80000 |0x10000| Memory bitmap 2 (hardware memory status)
0x100000|???????| Relocated kernel (will be huge)

Final layout
------------

Begin   | Size  | Content
--------|-------|--------
0x0     |0x1000 | IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
0x1000  |0x100  | IDTR (10 bytes used, and after that reserved)
0x1100  |0x100  | GDT (some used, and after that reserved)
0x70000 |0x10000| Memory bitmap 1 (currently free memory) (this allows (8*0x10000*0x1000)/1024**3 = 2GiB memory to be used, I think)
0x80000 |0x10000| Memory bitmap 2 (hardware memory status)
0x90000 |???????| Reserved for EBDA, ROM, Video Memory and other stuff there.
0x100000|???????| Kernel (Extended memory) (Size around 0x100000)
????????|???????| Free memory (must be allocated using the frame allocator)
