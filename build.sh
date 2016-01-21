#!/bin/bash
set -e
#set -x # turn on command printing

# prepare for build
mkdir -p build

echo "Compiling source files..."

echo "* bootloader"
# compile bootloader
nasm src/boot/boot.asm -f bin -o build/boot.bin

echo "* kernel entry point"
# compile kernel entry point
nasm -f elf64 src/entry.asm -o build/entry.o

echo "* kernel"
# compile kernel
cargo rustc --target x86_64-unknown-linux-gnu -- -Z no-landing-pads

echo "Linking objects..."

# link
ld -n --gc-sections -T buildsystem/linker.ld -o build/kernel.bin build/entry.o target/x86_64-unknown-linux-gnu/debug/librust_os.a

echo "Creating disk image..."

# floppify :] ( or maybe imagify, isofy or harddiskify)
echo "* create file"
echo "* bootsector"
cp build/boot.bin build/disk.img    # create image (boot.bin should be same size as actual floppy)

echo "* kernel"
kernel_size=`grep "kernel_size" buildsystem/build.conf | python2 -c 'print(raw_input().split("#",1)[0].split(": ")[1])'`
dd "if=build/kernel.bin" "of=build/disk.img" "bs=512" "seek=1" "count=$kernel_size" "conv=notrunc"


# TODO? clean?


echo "Done"
