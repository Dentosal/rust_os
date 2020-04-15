import sys
print("".join(map(lambda q: hex(ord(q))[2:], " ".join(sys.argv[1:]).strip())))
