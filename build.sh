#!/bin/bash
set -euo pipefail
# set -x # turn on command printing

# Config
TARGET="d7os"

# some constants
PATH=$PATH:$HOME/.cargo/bin
export RUST_BACKTRACE=1

# Prepare for build
mkdir -p build

echo "Compiling source files..."

echo "* Bootloader"
# Compile bootloader
nasm src/boot/stage0.asm -f bin -o build/stage0.bin
nasm src/boot/stage1.asm -f bin -o build/stage1.bin
# nasm src/boot/stage2.asm -f bin -o build/stage2.bin

(
    cd libs/d7boot/ &&
    nasm -f elf64 src/entry.asm -o entry.o &&
    RUSTFLAGS="-g -C opt-level=z" RUST_TARGET_PATH=$(pwd)  cargo xbuild --target ../../d7os.json --release &&
    ld -z max-page-size=0x1000 --gc-sections --print-gc-sections -T linker.ld -o ../../build/stage2.bin entry.o target/d7os/release/libd7boot.a &&
    python3 ../../tools/zeropad.py ../../build/stage2.bin 0x800
)

echo "* Kernel entry point"
# compile kernel entry point
nasm -f elf64 src/entry.asm -o build/entry.o

echo "* Kernel"

# Compile kernel (with full optimizations)
# RUST_TARGET_PATH=$(pwd) xargo build --target $TARGET --release
# RUSTFLAGS=-g RUST_TARGET_PATH=$(pwd) xargo rustc --target $TARGET --release -- -C opt-level=s
RUSTFLAGS="-g -C opt-level=s" RUST_TARGET_PATH=$(pwd) cargo xbuild --target $TARGET --release

echo "* Kernel assembly routines"
mkdir -p build/asm_routines/
for fpath in src/asm_routines/*.asm
do
    filename=$(basename "$fpath")   # remove path
    base="${filename%.*}"           # get basename
    nasm -f elf64 "$fpath" -o "build/asm_routines/$base.o"
done

echo "* Rust cli tools"
( cd libs/d7staticfs/ && cargo build --release )
( cd libs/d7elfpack/  && cargo build --release )

echo "Linking objects..."

# Link (use --print-gc-sections to debug)
# ld -z max-page-size=0x1000  -T build_config/linker.ld -o build/kernel_orig.elf build/entry.o target/$TARGET/release/libd7os.a build/asm_routines/*.o
# ld -z max-page-size=0x1000 --unresolved-symbols=ignore-all -T build_config/linker.ld -o build/kernel_orig.elf build/entry.o target/$TARGET/release/libd7os.a build/asm_routines/*.o
# ld -z max-page-size=0x1000 --gc-sections --print-gc-sections  -T build_config/linker.ld -o build/kernel_orig.elf build/entry.o target/$TARGET/release/libd7os.a build/asm_routines/*.o
ld -z max-page-size=0x1000 --gc-sections -T build_config/linker.ld -o build/kernel_orig.elf build/entry.o target/$TARGET/release/libd7os.a build/asm_routines/*.o

echo "* Saving objdump..."
objdump -CShdr -M intel build/kernel_orig.elf > build/objdump.txt
echo "* Saving readelf..."
readelf -e build/kernel_orig.elf > build/readelf.txt

echo "Stripping executable..."
cp build/kernel_orig.elf build/kernel_unstripped.elf
strip build/kernel_orig.elf

echo "Compressing kernel..."
./libs/d7elfpack/target/release/d7elfpack build/kernel_orig.elf build/kernel.elf
size_o=$(wc -c build/kernel_orig.elf | xargs -n 1 | tail -n +1 | head -n 1) # https://superuser.com/a/642932/328647
size_c=$(wc -c build/kernel.elf | xargs -n 1 | tail -n +1 | head -n 1) # https://superuser.com/a/642932/328647
echo "Compressed to $[ ($size_c * 100) / $size_o ]% of original"

echo "Cheking boundries..."

# Image size check
imgsize=$(wc -c build/kernel.elf | xargs -n 1 | tail -n +1 | head -n 1) # https://superuser.com/a/642932/328647
echo "imgsize: $imgsize"
maxsize=400 # size in blocks
if [ $[ imgsize / 0x200 > 400] -eq 1 ]
then
    echo "Kernel image seems to be too large."
    exit 1
fi

echo "Creating disk image..."
DISK_SIZE_BYTES=$(python3 -c 'print(0x200*0x800)') # a disk of 0x800=2048 0x200-byte sectors, 2**20 bytes, one mebibyte
DISK_SIZE_SECTORS=$(python3 -c "print($DISK_SIZE_BYTES // 0x200)")

# Create disk
echo "* create disk"
dd "if=/dev/zero" "of=build/disk.img" "bs=512" "count=$DISK_SIZE_SECTORS" "conv=notrunc"

echo "* copy boot stages"
dd "if=build/stage0.bin" "of=build/disk.img" "bs=512" "seek=0" "count=1" "conv=notrunc"
dd "if=build/stage1.bin" "of=build/disk.img" "bs=512" "seek=1" "count=1" "conv=notrunc"
dd "if=build/stage2.bin" "of=build/disk.img" "bs=512" "seek=2" "count=4" "conv=notrunc"

echo "* copy kernel"
dd "if=build/kernel.elf" "of=build/disk.img" "bs=512" "seek=6" "conv=notrunc"

echo "* write filesystem"
./libs/d7staticfs/target/release/mkimg build/disk.img $(($imgsize/0x200+8)) $(< build_config/staticfs_files.txt)

# TODO? Clean?

echo "Done"
