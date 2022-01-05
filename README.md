# D7 - An operating system

D7 is an operating system written in Rust. It aims to be an event-driven quasi-microkernel. By design, it runs only on moder x86-64 processors. The system is in fairly early stage, and is developed by fearlessly breaking things, trying new stuff before older stubs are even working and most importantly experimenting with weird ideas.

## Current features:
* Multitasking: tickless event-driven round-robin scheduler
* Executable programs, in ELF format
* IPC: PubSub messaging and named pipes
* Keyboard input
* Virtual TTYs
* Disk IO:
    * ATA PIO (Read only)
    * VirtIO-blk (Read only)
* Networking:
    * Drivers for NE2000 and RTL8139
    * IPv4 stack, supporting TCP, UDP, DHCP, ARP
* Services
    * Serviced - startup and service status queries
    * Netd - Manages network interfaces and sockets

## Planned in the near future:
* Shell
* Writing to disk, and a real filesystem

## Not-in-so-near future features:
* Networking
    * Intel E1000 driver
    * VirtIO-net driver
* Automated tests
* Proper filesystem drivers, maybe FAT32, or ext2/3
* Shell and utilities
* Device drivers for USB and Audio devices


# Development

This is a learning project. Currently code contributions are not accepted, as I'd like to learn to fix the problems myself. Forking the project is of course possible, if you'd like to develop something based on this.
Feel free to submit issues on GitHub if you find any bugs.

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

Vagrant isn't actually required: on systems with apt, like Debian or Ubuntu, it should be reasonably easy to just install the dependencies by hand. The install script can be found from `Vagrantfile`.

You will also need a virtual machine. Qemu is suggested, but Bochs should work as well. VirtualBox can also be used, but the project isn't actively tested with it. Moreover, you must run it yourself.

## Actually running

With Qemu and Vagrant installed, run `./autobuild.sh -u`. With Bochs: `./autobuild.sh -ub`. To use VirtualBox, run `./autobuild.sh -uv`.

## Local development

```bash
cargo fmt && factory && ./autobuild.sh -n
```

# License
This project is licensed under the MIT license, which can be found in the file called LICENSE.
