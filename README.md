# Dimension 7 - An operating system
Dimension 7 is a simple x86-64 operating system written in Rust. It is in fairly early stage, and is developed by fearlessly breaking things, trying new stuff before older stubs are even working and most importantly.

## Development

This is a learning project. Currently code contributions are not accepted, as I'd like to learn to fix the problems myself. Feel free to submit issues on GitHub if you find any bugs.

Currently everything is subject to quick changes. Until I feel safe to even partially stabilize any modules, all development is done in main branch, and not being able to boot the version on main branch is more like rule than an exception.

## Current features:
* Long mode with Rust
* Text mode terminal
* Physical memory manager
* Paging
* Basic interrupt support
* Keyboard input

## Planned in near future:
* Virtual TTYs
* Disk IO
* Automated tests

## Not-in-so-near future features:
* A proper filesystem, maybe SFS, FAT32, or ext3
* Networking
* Executable programs, probably in ELF format
* Shell and utilities
* Multitasking
* Device drivers for USB/Audio/NICs

# Running
The project is using Vagrant to virtualize the building environment. While being a little slower, this means that building the system on any supported platform should Just Work™.

## Dependencies

Building with default automated build system required that Vagrant is installed. I use VirtualBox as my Vagrant provider, but [other providers](https://www.vagrantup.com/docs/providers/) should work as well.

Vagrant isn't actually required: on Linux it should be reasonably easy to just install the dependencies by hand:

    sudo apt-get update
    sudo apt-get install nasm -y
    sudo apt-get install git -y
    sduo apt-get install texinfo flex bison python-dev ncurses-dev -y
    sudo apt-get install cmake libssl-dev -y
    curl -sSf https://static.rust-lang.org/rustup.sh | sh -s -- --channel=nightly -y
    cargo install xargo

You will also need a virtual machine. Qemu is suggested, but Bochs should work as well. VirtualBox can also be used, but the project isn't actively tested with it. Moreover, you must run it yourself.

## Actually running

With Qemu and Vagrant installed, run `./autobuild.sh -u`. With Bochs: `./autobuild.sh -ub`. To use VirtualBox, run `./autobuild.sh -c`, and then convert raw binary image `build/disk.img` to VirtualBox format.


# License
This project is licensed under MIT license, that can be found in the file called LICENSE.

The paging module, under [/src/paging/](/src/paging/), is partially a rewrite of [Philipp Oppermann's Blog OS](https://github.com/phil-opp/blog_os), and is really similar to his module. The code is licensed under MIT license. The hole list allocator is also from the Blog OS project, and the source file contains a license notice.
