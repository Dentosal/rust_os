#!/bin/bash
set -e

# give -n to run  without virtualizing the build environment
# give -u to run "vagrant up"
# give -v to open in VirtualBox
# give -b to open in Bochs
# give -s to use "qemu -s" for gdb in port 1234
# give -d to use additional debug options
# give -c to compile only
# give -r to run only

flag_native=0
flag_vagrant=0
flag_vbox=0
flag_bochs=0
flag_qemu_s=0
flag_debug=0
flag_build_only=0
flag_run_only=0

while getopts 'abf:nuvbsdcr' flag; do
  case "${flag}" in
    n) flag_native=1 ;;
    u) flag_vagrant=1 ;;
    v) flag_vbox=1 ;;
    b) flag_bochs=1 ;;
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
    if [ $flag_native -eq 1 ]
    then
        factory
    else
        vagrant ssh -c "cd /vagrant/ && factory"
    fi
fi


if [ -d "/mnt/c/Windows" ]; then
    # This is Windows subsystem for Linux
    qemucmd='qemu-system-x86_64.exe'
    vboxcmd='VBoxManage.exe'
else
    # Generic posix
    qemucmd='qemu-system-x86_64'
    vboxcmd='VirtualBox'
fi


if [ $flag_build_only -ne 1 ]
then
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
        if [ $flag_debug -eq 1 ]
        then
            $qemucmd -cpu max -enable-kvm -smp 4 -d int,in_asm,guest_errors -m 4G -no-reboot -drive file=build/disk.img,format=raw,if=ide -monitor stdio -serial file:CON
        else
            $qemucmd -cpu max -enable-kvm -smp 4 -m 4G -no-reboot -no-shutdown -drive file=build/disk.img,format=raw,if=ide -monitor stdio -nic user,model=rtl8139,hostfwd=tcp::5555-:22
        fi
    fi
fi
