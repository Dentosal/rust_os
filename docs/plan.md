Dimension 7 Internals "Documentation"/"Plan"
============================================

# Bootable Disk Layout

Begin | Size  | Content
------|-------|--------
    0 |   200 | Stage 0 (Master boot record) / `boot_stage0`
  200 |   200 | Stage 1 / `boot_stage1`
  400 |   400 | Stage 2 / `d7boot`
  800 |     ? | Kernel
    ? |     ? | InitRD


# Kernel Memory Layout

## Boot / Intermediate

Begin    | Size  | Content
---------|-------|--------
        0|   1000| IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
     1000|    100| GDT (some used, and after that reserved)
     2000|   1000| Boot stage memory map from BIOS (some used, and after that reserved)
     3000|      4| Kernel/InitRD split sector number
     7bfe|      ?| Stack (grows downwards)
     8000|    400| Stage 2 bootloader (two sectors atm)
   1_0000|   3000| Page tables (Boot stage only)
  10_0000|      ?| Kernel ELF image + InitRD (Boot stage only) (size probably around 0x10_0000)
 100_0000|      ?| Relocated and expanded kernel from ELF image (will be huge)

## Final layout

Using 2MiB pages here.

All rwx flags are on for addresses < 0x20_0000, as the AP trampoline requires.

Begin      | Size     |rwx| Content
-----------|----------|---|--------
          0|      1000|rwx| IDT Descriptors (all used) (0x100 entries * 16 bytes per entry)
       2000|      1000|rwx| SMP AP startup trampoline
       4000| 40*ncores|rwx| GDTs (0x40 =  64 bytes per cpu core)
       6000| 68*ncores|rwx| TSSs (0x68 = 104 bytes per cpu core)
       a000|       100|rwx| Pointer to function that handles in-process interrupts
     4_0000|    4_0000|rw-| DMA / VirtIO memory buffers (requires "low" memory)
     8_0000|         ?|---| Reserved for EBDA, ROM, Video Memory and other stuff there.
   100_0000|         ?|+++| Kernel + InitRD (Size around 0x800_0000, each section is page_aligned)
  1000_0000|  100_0000|rw-| Page tables, (0x200_000 used and after that reserved)
          ?|         ?|   | Free memory (must be allocated using the frame allocator)
  4000_0000|         ?|rw?| Heap allocator managed memory (This is 1GiB)

TODO: Bump allocator

## Virtual address space

Only for the kernel, of course. Processes have a separate layout.

Begin       | Size    |rwx| Content
------------|---------|---|---------
           0| 20_0000 |r--| IDT, GDT, Global pointers, DMA buffers
     20_0000| 20_0000 |r-x| Common code for process switching
    100_0000|       ? |+++| Kernel (ELF image)
   1000_0000| 20_0000 |rw-| Kernel page tables, identity mapped
   1100_0000| 20_0000 |rw-| System call kernel stack (grows downwards)
 1_0000_0000|       ? |???| Allocated virtual memory for processes
HIGHER_HALF | ?       |rw-| Physical memory mapped here for fast and convenient access

## The first page

Begin  | Size | Content
--------------|---------|---------
      0| 1000 | IDT
   1000|    ? | GDT
   a000|    8 | Ptr to the process interrupt handler


# Interrupts

Numbers     | Description
------------|-------------
0x00..=0x1f | Standard intel interrupts
0x20..=0x2f | PIC interrupts
0xd7        | System call
0xd8        | LAPIC timer
0xdd        | System panic (IPI)
0xff        | IOAPIC spurious interrupt

# Process Virtual Memory Layout

Begin         | Size    |rwx| Content
--------------|---------|---|---------
             0| 20_0000 |r--| IDT, GDT, static kernel data
       20_0000| 20_0000 |r-x| Common code for process switching
       40_0000| 40_0000 |rw-| Process stack
      100_0000|       ? |+++| Process elf image
 100_0000_0000|*dynamic*|rw-| Process heap (At 1 TiB)

## The first page

Begin         | Size    | Content
--------------|---------|---------
             0|    1000 | IDT
          1000|    ? 10 | GDT
          8000|       ? | Per-processor info table


IDT, GDT, static kernel data


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