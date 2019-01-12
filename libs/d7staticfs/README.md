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
4      |    4 | Version number (always 1)
8      |    4 | Number of file entries
12     |    4 | Reserved (always zero)

The header is followed by an array of file entries, each 16 bytes in size.

Offset | Size | Content
-------|------|--------
0      |   12 | Filename (`[a-zA-Z0-9_]+`, Zero-padded)
12     |    4 | File size in sectors (zero for empty file)

There is also a two special File entries. When filename is all zeroes:
* Skip: Size is nonzero: The file has been deleted, and the entry marks the size of the empty region on file space.
* Zero: Filename and size are both all zeros. Can be produced when merging file table entries This entry should be ignored.

## Files

First file starts immediately after the file table. Second file starts right after it. Size of each file (in sectors) is specified in the file table.

File size is always a multiple of sector size. They are zero-padded the full length if necessary.
