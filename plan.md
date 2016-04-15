Dimension 7 "Documentation"
===========================

Kernel Memory Layout
====================

Begin  | Size  | Content
-------|-------|--------
0x0    |0x100  | IDT (all used)
0x100  |0x100  | GDT (some used, and after that reserved)
0x8000 |0x9E00 | Kernel (ELF image)
0x11e00|?????? | Reserved for kernel image
0x20000|0x3000 | Page tables (Boot stage)
0x30000|0x10000| Memory bitmap 1 (currently free memory) (this allows (8*0x10000*0x1000)/1024**3 = 2GiB memory to be used, I think)
0x40000|0x10000| Memory bitmap 2 (hardware memory status)
0x50000|???????| Free memory (must be allocated using the frame allocator)