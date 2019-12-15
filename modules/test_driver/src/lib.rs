#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![deny(unused_must_use)]

use libd7::{fs, process::Process, attachment::*, syscall};

#[macro_use]
extern crate alloc;

const SEND_DATA: [u8; 4] = [0x12, 0x34, 0x56, 0xff];

#[no_mangle]
fn main() -> u64 {
    let pid = syscall::get_pid();

    // List processes
    syscall::debug_print(&format!("{:?}", fs::list_dir("/mnt").unwrap()));

    if pid == 1 {
        let a = Leaf::new("/mnt/a0").unwrap();
        let p = Process::spawn("/mnt/staticfs/mod_test").unwrap();

        let avail_fd = syscall::fd_select(&[], None).unwrap();
        loop {
            let avail_fd = syscall::fd_select(&[a.fd, p.fd], None).unwrap();
            if avail_fd == p.fd {
                syscall::debug_print(&format!("Wait proc"));
                let retcode = p.wait();
                syscall::debug_print(&format!("Return code {:?}", retcode));
                break;
            } else if avail_fd == a.fd {
                syscall::debug_print(&format!("Wait attach"));
                let req = a.next_request().unwrap();
                match req.type_ {
                    FileOperationType::Read => {
                        a.reply(req.into_reply(SEND_DATA.to_vec())).unwrap();
                    }
                    other => unimplemented!("Unsupported request: {:?}", other)
                }
                syscall::debug_print(&format!("Replied"));
            } else {
                unreachable!()
            }
        }
    } else if pid == 2 {
        let proc0 = syscall::fs_open("/mnt/a0").unwrap();
        let mut buffer = [0u8; 4];
        let count = syscall::fd_read(proc0, &mut buffer).unwrap();
        assert_eq!(buffer[..count], SEND_DATA);
    }

    pid
}
