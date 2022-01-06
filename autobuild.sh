#!/bin/bash
set -e

# Flags:
# -c to compile only
# -r to run only
# -g to run in vargrant
# -u to run "vagrant up"
# -v to open in VirtualBox
# -b to open in Bochs
# -d to use additional debug options

flag_vagrant=0
flag_vagrant_up=0
flag_vbox=0
flag_bochs=0
flag_qemu_s=0
flag_debug=0
flag_self_test=0
flag_build_only=0
flag_run_only=0

while getopts 'abf:crguvbd' flag; do
  case "${flag}" in
    c) flag_build_only=1 ;;
    r) flag_run_only=1 ;;
    g) flag_vagrant=1 ;;
    u) flag_vagrant_up=1 ;;
    v) flag_vbox=1 ;;
    b) flag_bochs=1 ;;
    d) flag_debug=1 ;;
    *) error "Unexpected option ${flag}" ;;
  esac
done

if [ $flag_run_only -ne 1 ]
then
    if [ $flag_vagrant_up -eq 1 ]
    then
        vagrant up
    fi
    if [ $flag_vagrant_up -eq 1 ]
    then
        vagrant ssh -c "cd /vagrant/ && ./autobuild -nc"
    else
        python3 build_config/configure.py
        ninja
    fi
fi

if [ $flag_build_only -eq 1 ]
then
    exit
fi

qemu_flags=''

if [ -d "/mnt/c/Windows" ]; then
    # This is Windows subsystem for Linux
    qemucmd='qemu-system-x86_64.exe'
    vboxcmd='VBoxManage.exe'
else
    # Generic posix, assume kvm is available
    qemucmd='qemu-system-x86_64 --enable-kvm'
    vboxcmd='VirtualBox'
fi


if [ $flag_vbox -eq 1 ]
then
    rm build/disk.vdi
    $vboxcmd convertfromraw build/disk.img build/disk.vdi --format vdi --uuid "63f64532-cad0-47f1-a002-130863cf16a7"

    if [ $flag_debug -eq 1 ]
    then
        $vboxcmd startvm "RustOS" --debug
    else
        $vboxcmd startvm "RustOS"
    fi
elif [ $flag_bochs -eq 1 ]
then
    if [ $flag_debug -eq 1 ]
    then
        bochs -q -f dbgenv_config/bochs_debug
    else
        bochs -q -f dbgenv_config/bochs_normal
    fi
else
    # More qemu flags
    # -nic user,model=virtio
    # -nic user,model=virtio,id=u
    # -object filter-dump,id=f1,netdev=u1,file=dump.dat
    # -drive file=build/disk.img,format=raw,if=virtio
    # -cpu qemu64,+invtsc,+rdtscp,+tsc-deadline
    flags="-cpu max -smp 4 -m 4G -no-reboot -no-shutdown"
    flags="$flags -drive file=build/disk.img,format=raw,if=ide"
    flags="$flags -nic user,model=rtl8139,hostfwd=tcp::5555-:22"
    flags="$flags -monitor stdio -serial file:CON"

    if [ $flag_debug -eq 1 ]
    then
        flags="$flags -d int,in_asm,guest_errors"
    fi
    $qemucmd $flags
fi
