#!/bin/bash
set -e

nasm src/boot/boot.asm -f bin -o ../start.img
cargo build # rustc -- -C no-split-stack -Z no-landig-pads
