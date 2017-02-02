# Dimension 7 - An operating system
A simple x86-64 operating system written in Rust and nasm.

## Current features:
* Long mode with Rust
* Text output terminal
* Physical memory manager
* Paging
* Basic interrupt support
* Keyboard input

## Planned in near future:
* Virtual TTYs
* Disk IO

## Not-in-so-near future features:
* Filesystem
* Networking
* Executable programs
* Shell & Utils & Editor
* Multitasking
* Device drivers for USB/Audio/NICs

# Running
Have Qemu and Vagrant installed, and run `./autobuild.sh -u`.
