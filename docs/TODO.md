# Backlog

* spin::Once seems to have some strange bug in v 0.7
* Never map anything to virtual address zero
* SMP support (multiple cores):
    * Move kernel to use new static mappings for physical memory access
    * Scheduler rewrite
    * TLB Shootdown support
* Convert system calls from (len, ptr) to (ptr, len).
* System call and IPC topic access control
* Move/copy disk drivers to own modules
    * All must be moved in one step
* Implement proper logging in `libd7`
* Provide process-accessable event system
    * Gives new scheduler event ids when reading
    * Activates events when writing
    * I.e. is is general-purpose event trigger system
* Version check `d7abi` and `libd7` on process startup (include check in `libd7`)
    * As the programs are statically linked, they must be version-checked against the kernel
* Proper, graphics-mode GUI
* Support small pages for better memory control (requires lots of rewriting)
* Filesystems
    * https://github.com/rafalh/rust-fatfs
    * https://github.com/pi-pi3/ext2-rs
    * https://github.com/omerbenamram/mft
* Porting rustc
    * https://www.reddit.com/r/rust/comments/5ag60z/how_do_i_bootstrap_rust_to_crosscompile_for_a_new/d9gdjwf/
* Reimplement virtualbox support (create hard drive images)
* Look into https://github.com/minexew/Shrine/blob/master/HwSupp/Pci.HC
