#!/bin/bash
set -e

# give -u to run "vagrant up"
# give -v to open in VirtualBox
# give -b to open in Bochs
# give -s to use "qemu -s" for gdb in port 1234
# give -d to use additional debug options
# give -c to compile only
# give -r to run only

flag_vagrant=0
flag_vbox=0
flag_bochs=0
flag_qemu_s=0
flag_debug=0
flag_build_only=0
flag_run_only=0

while getopts 'abf:uvbsdcr' flag; do
  case "${flag}" in
    u) flag_vagrant=1 ;;
    v) flag_vbox=1 ;;
    b) flag_bochs=1 ;;
    s) flag_qemu_s=1 ;;
    d) flag_debug=1 ;;
    c) flag_build_only=1 ;;
    r) flag_run_only=1 ;;
    *) error "Unexpected option ${flag}" ;;
  esac
done

if [ $flag_run_only -ne 1 ]
then
    if [ $flag_vagrant -eq 1 ]
    then
        vagrant up
    fi
    vagrant ssh -c "cd /vagrant/ && ./build.sh"
fi

if [ $flag_build_only -ne 1 ]
then
    if [ $flag_vbox -eq 1 ]
    then
        if [ $flag_debug -eq 1 ]
        then
            VirtualBox startvm "RustOS" --debug
        else
            VBoxManage startvm "RustOS"
        fi
    elif [ $flag_bochs -eq 1 ]
    then
        bochs -q -f bochs_config
    else
        if [ $flag_qemu_s -eq 1 ]
        then
            qemu-system-x86_64 -d int -m 4096 -no-reboot build/disk.img -monitor stdio -s -S
        else
            if [ $flag_debug -eq 1 ]
            then
                qemu-system-x86_64 -d int,in_asm -m 4096 -no-reboot build/disk.img -monitor stdio
            else
                # qemu-system-x86_64 -d int -m 4096 -no-reboot build/disk.img -monitor stdio
                qemu-system-x86_64 -m 4096 -no-reboot build/disk.img -monitor stdio
            fi
        fi
    fi
fi
