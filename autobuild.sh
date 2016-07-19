#!/bin/bash
set -e

# give -v to open in VirtualBox
# give -u to run "vagrant up"
# give -s to use "qemu -s" for gdb in port 1234
# give -d to use additional debug options

flag_vbox=0
flag_debug=0
flag_vagrant=0
flag_qemu_s=0

while getopts 'abf:uvsd' flag; do
  case "${flag}" in
    u) flag_vagrant=1 ;;
    v) flag_vbox=1 ;;
    s) flag_qemu_s=1 ;;
    d) flag_debug=1 ;;
    *) error "Unexpected option ${flag}" ;;
  esac
done

if [ $flag_vagrant -eq 1 ]
then
    vagrant up
fi

vagrant ssh -c "cd /vagrant/ && ./build.sh"

if [ $flag_vbox -eq 1 ]
then
    if [ $flag_debug -eq 1 ]
    then
        VirtualBox --startvm "RustOS" --debug
    else
        VBoxManage startvm "RustOS"
    fi
else
    if [ $flag_qemu_s -eq 1 ]
    then
        qemu-system-x86_64 -d int -m 4096 -no-reboot build/disk.img -monitor stdio -s -S
    else
        if [ $flag_debug -eq 1 ]
        then
            qemu-system-x86_64 -d int,in_asm -m 4096 -no-reboot build/disk.img -monitor stdio
        else
            qemu-system-x86_64 -d int -m 4096 -no-reboot build/disk.img -monitor stdio
        fi
    fi
fi
