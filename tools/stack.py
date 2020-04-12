# Annotate stacktrace
# Use `print-stack 100` in Bochs
# Use `x /100gx $esp` in Qemu

import re
import sys
from pathlib import Path
from subprocess import Popen, PIPE

elf_file = Path(sys.argv[1]).resolve(strict=True)

return_addr_names = {} # return_addr -> function_name

# Read objdump address to assembly mappings
with Popen(["objdump", "-d", "-M", "intel", str(elf_file)], stdout=PIPE) as p:
    store_next_addr_as = None
    for line in p.stdout:
        line = line.strip(b" \t")
        columns = line.count(b"\t")

        if columns == 0:  # Skip useless lines
            continue

        if columns == 1:  # Skip second (overflown) line of raw bytes
            continue

        addr, _bytes, code = line.split(b"\t")
        addr = int(addr[:-1], 16)

        if name := store_next_addr_as:
            return_addr_names[addr] = name
            store_next_addr_as = False

        if m := re.match(br"call\s+[0-9a-f]+\s+<(.+)>", code):
            store_next_addr_as = m.group(1).decode()


addrs = []
for line in sys.stdin:
    line = line.strip()
    if not line:
        break

    # Qemu
    if m := re.match(r"[0-9a-f]{16}: 0x([0-9a-f]{16}) 0x([0-9a-f]{16})", line):
        addrs.append(int(m.group(1), 16))
        addrs.append(int(m.group(2), 16))

    # Bochs
    elif m := re.match(
        r"\| STACK 0x[0-9a-f]{16} \[0x([0-9a-f]{8}):0x([0-9a-f]{8})\]", line
    ):
        addrs.append(int(m.group(1) + m.group(2), 16))

if not addrs:
    exit("No addresses read")

for addr in addrs:
    if name := return_addr_names.get(addr):
        print(f"{hex(addr)[2:]:16} {name}")
