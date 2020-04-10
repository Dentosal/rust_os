Troubleshooting help tips:

* Check stack size at `src/entry.asm`
* Check stack amount of sectors loaded from disk at `src/boot/boot_stage0.asm`
* Check that `plan.md`is in sync with bootloader constants, FrameAllocator, and others
* Run in bochs with `trace on`


## Frequent problems with solutions

### IRQ is not firing?

Check that PIC is not masking it
