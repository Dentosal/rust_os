import sys
if sys.version_info[0] != 3:
    exit("Py3 required.")

import ast

class MockRam(dict):
    def __missing__(self, addr):
        return None

def b2i(l):
    return sum([a*0x100**i for i,a in enumerate(l)])

def i2b(i):
    b = []
    while i:
        b.append(i%0x100)
        i //= 0x100
    return b


def main(fn, mreq):
    with open(fn, "rb") as f:
        img = f.read()

    # verify image
    print("Verifying...")
    assert img[0:4] == bytes([0x7f, 0x45, 0x4c, 0x46]), "magic"
    assert img[4] == 0x2, "bitness"
    assert img[18] == 0x3e, "instruction set"
    assert img[5] == 0x1, "endianess"
    assert img[6] == 0x1, "version"
    assert img[54] == 0x38, "program header size"
    print("Verification ok.\n")

    print("Load point {:#x}".format(b2i(img[24:24+8])))
    pht_pos = b2i(img[32:32+8])
    pht_len = b2i(img[56:56+2])
    print("Program header len={} pos={:#x}".format(pht_len, pht_pos))

    ptr = pht_pos

    ram = MockRam()

    for index in range(pht_len):
        print("Header #{}:".format(index+1))
        segment_type = img[ptr]
        if segment_type == 1:
            print("  This is a LOAD segment")

            flags    = b2i(img[(ptr+4):(ptr+4)+4])
            p_offset = b2i(img[(ptr+8):(ptr+8)+8])
            p_vaddr  = b2i(img[(ptr+16):(ptr+16)+8])
            p_filesz = b2i(img[(ptr+32):(ptr+32)+8])
            p_memsz  = b2i(img[(ptr+40):(ptr+40)+8])

            # clear
            for i in range(p_memsz):
                ram[p_vaddr+i] = 0

            # copy
            for i in range(p_filesz):
                ram[p_vaddr+i] = img[p_offset+i]

                if p_vaddr+i in mreq:
                    print("{:#x}->{:#x}: {:#x}".format(p_offset+i, p_vaddr+i, ram[p_vaddr+i]))

            print("  Flags: {} ({:#b})".format("".join([(l*(flags&(1<<i)!=0)) for i,l in enumerate("XWR")]), flags))
            print("  Clear {:#x} bytes starting at {:#x}".format(p_memsz, p_vaddr))
            print("  Copy {:#x} bytes from {:#x} to {:#x}".format(p_filesz, p_offset, p_vaddr))
            print("  Initialized: {:#x} bytes, uninitialized: {:#x} bytes".format(p_filesz, p_memsz-p_filesz))

        else:
            print("  This isn't a LOAD segment")

        ptr += 0x38

    for r in mreq:
        if ram[r] is None:
            print("{:#x}: No data".format(r))
        else:
            print("{:#x}: {:#x}".format(r, ram[r]))

if __name__ == '__main__':
    main(sys.argv[1], [int(ast.literal_eval(r)) for r in sys.argv[2:]])
