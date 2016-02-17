import sys
print "".join(map(lambda q: chr(int(q, 16)), " ".join(sys.argv[1:]).split()))
