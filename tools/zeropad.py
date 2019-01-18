#!/usr/bin/env python3
import os
import sys
import ast

try:
    file_name, size_bytes = sys.argv[1:]
except:
    print("Usage: ./{} file size_bytes".format(sys.argv[0]))
    sys.exit(1)

size_bytes = int(ast.literal_eval(size_bytes))

file_size = os.path.getsize(file_name)
assert 0 <= file_size <= size_bytes

with open(file_name, mode="ba") as f:
    f.write(bytes([0]*(size_bytes - file_size)))
