#!/bin/bash
set -e
#set -x # turn on command printing

# prepare for build
mkdir -p build

echo "Compiling source files..."

echo "* bootloader"
# compile bootloader
nasm src/boot/boot_stage0.asm -f bin -o build/boot_stage0.bin
nasm src/boot/boot_stage1.asm -f bin -o build/boot_stage1.bin

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
cp build/boot_stage0.bin build/disk.img    # create image (boot.bin should be same size as actual floppy)
dd "if=build/boot_stage1.bin" "of=build/disk.img" "bs=512" "seek=1" "count=1" "conv=notrunc"

echo "* kernel"
kernel_size=`grep "kernel_size" buildsystem/build.conf | python2 -c 'print(raw_input().split("#",1)[0].split(":",1)[1].strip())'`
kernel_offset=`grep "kernel_offset" buildsystem/build.conf | python2 -c 'print(raw_input().split("#",1)[0].split(":",1)[1].strip())'`
dd "if=build/kernel.bin" "of=build/disk.img" "bs=512" "seek=$kernel_offset" "count=$kernel_size" "conv=notrunc"


# TODO? clean?


echo "Done"
