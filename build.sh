#!/bin/bash
set -e

# prepare for build
mkdir -p build

# compile bootloader
nasm src/boot/boot.asm -f bin -o boot.bin

# compile kernel
cargo rustc --target x86_64-unknown-linux-gnu -- -Z no-landing-pads

# link
ld -n --gc-sections -T buildsystem/linker.ld -o kernel.bin target/x86_64-unknown-linux-gnu/debug/libblog_os.a


# floppify :] ( or maybe imagify, isofy or harddiskify)
