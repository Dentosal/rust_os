#!/bin/bash
set -e

# give -v to open in VirtualBox
# give -u to run vagrant up

vboxflag=0
vagrantflag=0

while getopts 'abf:v' flag; do
  case "${flag}" in
    v) vboxflag=1 ;;
    u) vagrantflag=1 ;;
    *) error "Unexpected option ${flag}" ;;
  esac
done

if [ $vagrantflag -eq 1 ]
then
    vagrant up
fi

vagrant ssh -c "cd /vagrant/ && ./build.sh"

if [ $vboxflag -eq 1 ]
then
    VBoxManage startvm "RustOS"
else
    qemu-system-x86_64 -d int -no-reboot build/disk.img
fi
