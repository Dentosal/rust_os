#!/bin/bash
set -e
# set -x # turn on command printing

# config
TARGET="rust_os"

# some constants
PATH=$PATH:$HOME/.cargo/bin

# prepare for build
mkdir -p build

echo "Compiling source files..."

echo "* bootloader"
# compile bootloader
nasm src/boot/alt0.asm -f bin -o build/alt0.bin
nasm src/boot/stage0.asm -f bin -o build/stage0.bin
nasm src/boot/stage1.asm -f bin -o build/stage1.bin
nasm src/boot/stage2.asm -f bin -o build/stage2.bin

echo "* kernel entry point"
# compile kernel entry point
nasm -f elf64 src/entry.asm -o build/entry.o

echo "* kernel"

# compile kernel (with full optimizations)
xargo build --target $TARGET --release

echo "* kernel assembly routines"
for fpath in src/asm_routines/*.asm
do
    filename=$(basename "$fpath")   # remove path
    base="${filename%.*}"           # get basename
    nasm -f elf64 "$fpath" -o "build/asm_routines/$base.o"
done

echo "Linking objects..."

# link
ld -z max-page-size=0x1000 --gc-sections -T buildsystem/linker.ld -o build/kernel.bin build/entry.o target/$TARGET/release/librust_os.a build/asm_routines/*.o
# ld -n --gc-sections -T buildsystem/linker.ld -o build/kernel.bin build/entry.o target/$TARGET/release/librust_os.a build/asm_routines/*.o

echo "Cheking boundries..."

# image size check
# toobig=$(wc -c build/kernel.bin | python3 -c 'print(int(int(input().strip().split(" ",1)[0])//512>197))') # where 197 is size in blocks
toobig=$(wc -c build/kernel.bin | python3 -c 'print(int(int(input().strip().split(" ",1)[0])//512>370))') # where 370 is size in blocks (absolute should be 381)
if [ $toobig -eq 1 ]
then
    echo "Kernel image seems to be too large."
    exit 1
fi

echo "Creating disk image..."
DISK_SIZE_BYTES=$(python3 -c 'print(0x200*0x800)') # a disk of 0x800=2048 0x200-byte sectors, 2**20 bytes, one mebibyte
DISK_SIZE_SECTORS=$(python3 -c "print($DISK_SIZE_BYTES // 0x200)")

# create disk
echo "* create disk"
dd "if=/dev/zero" "of=build/disk.img" "bs=512" "count=$DISK_SIZE_SECTORS" "conv=notrunc"

echo "* copy boot stages"
# dd "if=build/stage0.bin" "of=build/disk.img" "bs=512" "seek=0" "count=1" "conv=notrunc"
dd "if=build/alt0.bin" "of=build/disk.img" "bs=512" "seek=0" "count=1" "conv=notrunc"
dd "if=build/stage1.bin" "of=build/disk.img" "bs=512" "seek=1" "count=1" "conv=notrunc"
dd "if=build/stage2.bin" "of=build/disk.img" "bs=512" "seek=2" "count=1" "conv=notrunc"

echo "* copy kernel"
dd "if=build/kernel.bin" "of=build/disk.img" "bs=512" "seek=3" "conv=notrunc"

echo "Saving objdump..."
objdump -CShdr -M intel build/kernel.bin > objdump.txt
echo "Saving readelf..."
readelf -e build/kernel.bin > readelf.txt

# TODO? clean?

echo "Done"
