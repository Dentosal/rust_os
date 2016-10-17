Dimension 7 Internals "Documentation"
=====================================

Kernel Memory Layout
====================

Boot / Intermediate
-------------------

Begin  | Size  | Content
-------|-------|--------
0x0    |0x1000 | IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
0x1000 |0x100  | IDTR (10 bytes used, and after that reserved)
0x1100 |0x100  | GDT (some used, and after that reserved)
0x2000 |0x1000 | Boot stage memory map (some used, and after that reserved)
0x8000 |???????| Kernel ELF image (Boot stage only)
0x10000|???????| Relocated kernel (size proabably 0x10000)
0x20000|0x3000 | Page tables (Boot stage only)
0x30000|0x10000| Memory bitmap 1 (currently free memory) (this allows (8*0x10000*0x1000)/1024**3 = 2GiB memory to be used, I think)
0x40000|0x10000| Memory bitmap 2 (hardware memory status)


Final layout
------------

Begin  | Size  | Content
-------|-------|--------
0x0    |0x1000 | IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
0x1000 |0x100  | IDTR (10 bytes used, and after that reserved)
0x1100 |0x100  | GDT (some used, and after that reserved)
0x10000|???????| Kernel
0x30000|0x10000| Memory bitmap 1 (currently free memory) (this allows (8*0x10000*0x1000)/1024**3 = 2GiB memory to be used, I think)
0x40000|0x10000| Memory bitmap 2 (hardware memory status)
0x50000|???????| Free memory (must be allocated using the frame allocator)
