# Dimension 7 - An operating system
Dimension 7 is a simple x86-64 operating system written in Rust. It is in fairly early stage, and is developed by fearlessly breaking things, trying new stuff before older stubs are even working and most importantly experimenting with weird ideas.

## Development

This is a learning project. Currently code contributions are not accepted, as I'd like to learn to fix the problems myself. Feel free to submit issues on GitHub if you find any bugs.

Currently everything is subject to quick changes. Any module should be considered unstable.

### Branches

Main branch should always contain a working build, that can be compiled and it boots successfully.
Feature development is done in separate branches.

Currently following features are under development:
* Generic ATA IDE driver
* Networking: Intel E1000 NIC driver


## Current features:
* Long mode with Rust
* Text mode terminal
* Physical memory manager
* Paging
* Interrupts
* Keyboard input (Somewhat)

## Planned in near future:
* Virtual TTYs
* Disk IO
* Networking
 * Intel E1000 driver

## Not-in-so-near future features:
* Automated tests
* A proper filesystem, maybe SFS, FAT32, or ext3
* Executable programs, probably in ELF format
* Shell and utilities
* Multitasking
* Device drivers for USB/Audio/NICs

# Running
The project is using Vagrant to virtualize the building environment. While being a little slower, this means that building the system on any supported platform should Just Work™. If you have a Unix-like system, install Qemu and

```bash
git clone https://github.com/Dentosal/rust_os.git && cd rust_os && ./autobuild.sh -u
```

Sometimes shared folder feature will not work, and you get an error message about missing `/vagrant` etc. In that case installing vbguest plugin should help:

```bash
vagrant plugin install vagrant-vbguest
```


If you don't have a Unix-like system, then you should probably get one, they are pretty awesome compared to old DOS systems or [Dentosal/rust_os](https://github.com/Dentosal/rust_os/). However, building on WSL is also possible. Just install the required tools (see [Vagrantfile](Vagrantfile)), and the run `./autobuild.sh -n`

## Dependencies

Building with default automated build system required that Vagrant is installed. I use VirtualBox as my Vagrant provider, but [other providers](https://www.vagrantup.com/docs/providers/) should work as well.

Vagrant isn't actually required: on systems with apt, like Debian or Ubuntu, it should be reasonably easy to just install the dependencies by hand. The install script can be found from Vagrantfile.

You will also need a virtual machine. Qemu is suggested, but Bochs should work as well. VirtualBox can also be used, but the project isn't actively tested with it. Moreover, you must run it yourself.

## Actually running

With Qemu and Vagrant installed, run `./autobuild.sh -u`. With Bochs: `./autobuild.sh -ub`. To use VirtualBox, run `./autobuild.sh -c`, and then convert raw binary image `build/disk.img` to VirtualBox format.

# License
This project is licensed under MIT license, that can be found in the file called LICENSE.

The paging module, under [/src/paging/](/src/paging/), is partially a rewrite of [Philipp Oppermann's Blog OS](https://github.com/phil-opp/blog_os), and is really similar to his module. The code is licensed under MIT license. The hole list allocator is also from the Blog OS project, and the source file contains a license notice.
