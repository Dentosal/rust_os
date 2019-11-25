Dimension 7 Internals "Documentation"/"Plan"
============================================

# Bootable Disk Layout

Begin | Size  | Content
------|-------|--------
    0 |   200 | Stage 0 (Master boot record) / `boot_stage0`
  200 |   200 | Stage 1 / `boot_stage1`
  400 |   200 | Stage 2 / `boot_stage2`
  600 |     ? | Kernel
    ? |     ? | Filesystem


# Kernel Memory Layout

## Boot / Intermediate

Begin    | Size  | Content
---------|-------|--------
        0|   1000| IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
     1000|    100| GDT (some used, and after that reserved)
     2000|   1000| Boot stage memory map from BIOS (some used, and after that reserved)
     4000|   1000| Bootloader ELF decompression tables (about 0x500 used, and after that reserved)
     8000|    400| Stage 2 bootloader (two sectors atm)
     A000|   4000| Disk load buffer
   1_0000|      ?| Kernel ELF image (Boot stage only) (size proabably around 0x10_0000)
  20_0000|   3000| Page tables (Boot stage only)
1_000_000|      ?| Relocated and expanded kernel from ELF image (will be huge)

## Final layout

Using 2MiB pages here.

Begin      | Size     |rwx| Content
-----------|----------|---|--------
          0|      1000|rw-| IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
      1_000|       100|rw-| GDT (some used, and after that reserved)
     20_000|     70000|rw-| DMA / VirtIO memory buffers (requires "low" memory)
     90_000|         ?|---| Reserved for EBDA, ROM, Video Memory and other stuff there.
  1_000_000|         ?|+++| Kernel (Extended memory) (Size around 0x1_000_000, as each section is page_aligned)
 10_000_000| 1_000_000|rw-| Page tables, (0x200_000 used and after that reserved)
          ?|         ?|   | Free memory (must be allocated using the frame allocator)
 40_000_000|         ?|rw?| Heap allocator managed memory (This is 1GiB)

TODO: Bump allocator

## Virtual address space

TODO: Higher half kernel
TODO: Proper virtual memory map

Begin       | Size    |rwx| Content
------------|---------|---|---------
           0| 200_000 |r--| IDT, GDT, Global pointers
     200_000| 200_000 |r-x| Common code for process switching
  10_000_000| 200_000 |rw-| Kernel page tables, identity mapped
  11_000_000| 200_000 |rw-| System call kernel stack (grows downwards)
 100_000_000|       ? |???| Allocated virtual memory for processes


# Interrupts

Numbers     | Description
------------|-------------
0x00..=0x1f | Standard intel interrupts
0x20..=0x2f | PIC interrupts
0xd7        | System call

# Process Memory Layout

Begin         | Size    |rwx| Content
--------------|---------|---|---------
             0| 20_0000 |r--| IDT, GDT
       20_0000| 20_0000 |r-x| Common code for process switching
       40_0000| 40_0000 |rw-| Process stack
      100_0000|       ? |+++| Process elf image
 100_0000_0000|*dynamic*|rw-| Process dynamic memory (At 1 TiB)



# Scheduler tick and process switch procedure

## When PIT ticks

1. Save current process registers to the current stack
  * `x86-interrupt` cc saves all registers and return pointer
2. Switch to kernel page tables
  * TODO: might need some kind of jump area
3. Advance system clock
4. Run scheduler, and change the current process if required
5. Switch back to process page tables
6. Restore process registers and jump back into the process
  * `x86-interrupt` cc restores all registers and the return address

## Process stack when not active

Index | Size | Contents
------|------|----------
0     | 5    | Interrupt stack frame
5     | 1    | Tmp value for process interrupt handler
6     | 15   | Registers