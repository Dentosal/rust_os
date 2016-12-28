#!/usr/bin/env python3
import sys
import ast

try:
    file_name, location, new_byte = sys.argv[1:]
except:
    print("Usage: ./{} file location newbyte".format(sys.argv[0]))
    sys.exit(1)

addr = int(ast.literal_eval(location))
byte = int(ast.literal_eval(new_byte))

assert 0 <= addr
assert 0 <= byte < 2**8

with open(file_name, mode="br+") as f:
    f.seek(addr)
    f.write(bytes([byte]))
