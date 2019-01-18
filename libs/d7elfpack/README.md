D7 ElfPack
==========

Simple but efficient ELF file compression based on Huffman coding.
For ELF image of this kernel, the size was reduced to about 40% of original.

Only preserver useful sections:
* ELF header
* Program header
* Actual program sections

## Compression details

Standard [Huffman encoding](https://en.wikipedia.org/wiki/Huffman_coding) is used.
Only sections pointed by program headers are compressed; this is still a valid ELF file.
After compression, all unused details are stripped from the file.

## Decompression tables

The decompression table is two-part. First, it contains the values of leaf nodes in fixed-size
256-length array of bytes. Then the tree is embedded as bitstream.

The size of the tree is constant, `2n-1 = 2*256-1 = 511 bits`.
This means 64 bytes and one extra zero for padding.

Therefore, the whole decompression table takes just