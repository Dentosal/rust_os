#![no_std]

use core::panic::PanicInfo;
use libd7::syscall;

#[no_mangle]
pub extern "C" fn main() {
    // Test: get pid and use it as exit code
    let pid = syscall::get_pid();
    syscall::exit(pid);
}
