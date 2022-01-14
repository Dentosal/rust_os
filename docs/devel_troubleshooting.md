Troubleshooting help tips:

* Check stack size at `src/entry.asm`
* Check the amount of sectors loaded from the disk at `src/boot/boot_stage0.asm`
* Check that `plan.md` is in sync with bootloader constants, FrameAllocator, and others


## Frequent problems with solutions

### IRQ is not firing?

Check that PIC is not masking it


# Emulators

## Bochs
* Run in bochs with `trace on`
* Stacktrace with `print-stack 100`
* Page tables: `info tab` and `page 0x1234`

##
* Stacktrace with `x /100gx $esp`