#!/bin/bash
set -e

# give -v to open in VirtualBox

vboxflag=0

while getopts 'abf:v' flag; do
  case "${flag}" in
    v) vboxflag=1 ;;
    *) error "Unexpected option ${flag}" ;;
  esac
done

vagrant up
vagrant ssh -c "cd /vagrant/ && ./build.sh"
if [ $vboxflag -eq 1 ]
then
    VBoxManage startvm "RustOS"
else
    qemu-system-x86_64 -d int -no-reboot build/disk.img
fi
