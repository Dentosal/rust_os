#!/bin/bash
set -e

vagrant up
vagrant ssh -c "cd /vagrant/ && ./build.sh"
qemu-system-x86_64 build/disk.img
