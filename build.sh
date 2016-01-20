#!/bin/bash
set -e

nasm src/boot/boot.asm -f bin -o ../start.img
# cargo rustc --target x86_64-unknown-linux-gnu -- -Z no-landing-pads
