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
# compile kernel (with full optimizations)
cargo rustc --target x86_64-unknown-linux-gnu --release -- #-Z no-landing-pads # no-landing-pads disabled, moved to panic=abort, see Cargo.toml

echo "* kernel assembly routines"
for fpath in src/asm_routines/*.asm
do
    filename=$(basename "$fpath")   # remove path
    base="${filename%.*}"           # get basename
    nasm -f elf64 "$fpath" -o "build/asm_routines/$base.o"
done

echo "Linking objects..."

# link
ld -n --gc-sections -T buildsystem/linker.ld -o build/kernel.bin build/entry.o target/x86_64-unknown-linux-gnu/release/librust_os.a build/asm_routines/*.o

echo "Cheking boundries..."

toobig=$(wc -c build/kernel.bin | python2 -c 'print int(int(raw_input().split(" ",1)[0])/512>79)')
if [ $toobig -eq 1 ]
then
    echo "Kernel image seems to be too large."
    exit 1
fi

echo "Creating disk image..."

# floppify :] ( or maybe imagify, isofy or harddiskify)
echo "* create file"
echo "* bootsector"
cp build/boot_stage0.bin build/disk.img    # create image (boot.bin should be same size as actual floppy)
dd "if=build/boot_stage1.bin" "of=build/disk.img" "bs=512" "seek=1" "count=1" "conv=notrunc"

echo "* kernel"
dd "if=build/kernel.bin" "of=build/disk.img" "bs=512" "seek=2" "conv=notrunc"


# TODO? clean?


echo "Done"
