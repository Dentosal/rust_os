#![no_std]
#![feature(asm)]
#![deny(unused_must_use)]

use d7abi::syscall;

#[no_mangle]
pub extern "C" fn main() {
    // Test: get pid and use it as exit code
    let pid = syscall::get_pid();

    match syscall::print_string("a b c") {
        Ok(_) => syscall::exit(5),
        Err(_) => syscall::exit(6),
    }

    syscall::exit(pid);
}
