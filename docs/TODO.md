# Backlog

* Reduce allow() lints in the kernel
* Never map anything to virtual address zero, for processes at least (nullptr)
* SMP support (multiple cores):
    * Move kernel to use new static mappings for physical memory access as much as possible
    * Scheduler rewrite
    * TLB Shootdown support
* Userland for applications
    * Drivers as well, as much as possible, setup IO bitmaps in TSS to do this
* Convert system calls from (len, ptr) to (ptr, len).
* `exec` arguments
* `fork` and friends
* System call and IPC topic access control
    * See `capabilities.md`
* Version check `d7abi` and `libd7` on process startup (include check in `libd7`)
    * As the programs are statically linked, they must be version-checked against the kernel
* Proper, graphics-mode GUI
* Support small pages for better memory control (requires lots of rewriting)
* Filesystems
    * Virtual filesystem
    * https://github.com/pi-pi3/ext2-rs
    * https://github.com/omerbenamram/mft
* Porting rustc
    * https://www.reddit.com/r/rust/comments/5ag60z/how_do_i_bootstrap_rust_to_crosscompile_for_a_new/d9gdjwf/
* Reimplement virtualbox support (create hard drive images)
* Look into https://github.com/minexew/Shrine/blob/master/HwSupp/Pci.HC
