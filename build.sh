#!/bin/bash
set -e

# prepare for build
mkdir -p build

# compile bootloader
nasm src/boot/boot.asm -f bin -o build/boot.bin

# compile kernel entry point
nasm -f elf64 src/entry.asm -o build/entry.o

# compile kernel
cargo rustc --target x86_64-unknown-linux-gnu -- -Z no-landing-pads

# link
ld -n --gc-sections -T buildsystem/linker.ld -o build/kernel.bin build/entry.o target/x86_64-unknown-linux-gnu/debug/librust_os.a


# floppify :] ( or maybe imagify, isofy or harddiskify)
