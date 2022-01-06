#!python3

from sys import argv
from pathlib import Path

max_size = int(argv[1])
cur_size = Path("build/kernel_stripped.elf").stat().st_size

if cur_size > max_size:
    exit(f"Error: kernel image max size exceeded ({cur_size:#x} > {max_size:#x})")
