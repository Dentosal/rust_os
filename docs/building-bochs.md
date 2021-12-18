# Notes

Hypothesis: Slow startup is caused by bochs malloc+memcpy'ing large flat disk image to memory

# Per OS installation

## Linux

### Optional tweak: pcap logging

In `iodev/network/eth_vnet.cc` set `BX_ETH_VNET_PCAP_LOGGING 1`
In `Makefile` add `-lpcap` to `LIBS`

### Configure

```
./configure --enable-smp \
            --enable-cpu-level=6 \
            --enable-all-optimizations \
            --enable-x86-64 \
            --enable-vmx \
            --enable-avx \
            --enable-pci \
            --enable-show-ips \
            --enable-debugger \
            --enable-disasm \
            --enable-debugger-gui \
            --enable-logging \
            --enable-fpu \
            --enable-3dnow \
            --enable-sb16=dummy \
            --enable-cdrom \
            --enable-x86-debugger \
            --enable-ne2000 \
            --enable-iodebug \
            --disable-plugins \
            --disable-docbook \
            --with-x --with-x11 --with-term --with-sdl2
```
