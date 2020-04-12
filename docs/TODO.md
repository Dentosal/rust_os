* Move/copy disk drivers to own modules
    * All must be moved in one step
* Implement a initrd
    * Ramdisk builder tool (from staticfs)
    * Bootloader support
    * Kernel support
* Implement proper logging in `libd7`
* Implement (named) pipes
    * `/dev/pipe`
* Provide `/dev/event`
    * Gives new scheduler event ids when reading
    * Activates events when writing
    * I.e. is is general-purpose event trigger system
* Filesystems
    * https://github.com/rafalh/rust-fatfs
    * https://github.com/pi-pi3/ext2-rs
    * https://github.com/omerbenamram/mft
* Porting rustc
    * https://www.reddit.com/r/rust/comments/5ag60z/how_do_i_bootstrap_rust_to_crosscompile_for_a_new/d9gdjwf/

* Reimplement virtualbox support (create hard drive images)
* Look into https://github.com/minexew/Shrine/blob/master/HwSupp/Pci.HC
