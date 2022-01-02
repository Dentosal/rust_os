d7abi - Data definitions for D7 system calls
============================================

This crate is shared between the user processes and the kernel,
and contains data definitions for structures passed through
syscalls and kernel-process IPC.

Some useful traits are implemented for the data types, and
some necessary and useful methods are included.

In addition to Rust source code it also contains:
* Linker script for creating ELF files
* Json target definition file for `cargo xbuild`
