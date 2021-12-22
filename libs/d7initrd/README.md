D7_StaticFS
===========

Minimal read-optimized filesystem. Just a static file allocation table on disk. Writing is slow and unpleasant, but possible. All values are little-endian.

## Disk Layout

MBR contains a 32bit LBA sector number. It's located just before the boot signature, at offset `0x1fa`. It is the first sector after kernel section. File table is located there. After the file table, there are files.

## File Table

The file table begins with a simple 16-byte header.

Offset | Size | Content
-------|------|--------
0      |    4 | Magic number 0xd7cafed7
4      |    4 | Length of file list in bytes
8      |    8 | Length of the whole initrd in bytes

The header is followed by an array of file entries.
