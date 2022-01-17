//! Command line arguments

use core::{slice, str};

use crate::d7abi::PROCESS_STACK_END;

pub struct Args {
    index: usize,
    ptr: *const u8,
}
impl Iterator for Args {
    type Item = &'static str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= argc() {
            return None;
        }

        let result;

        unsafe {
            let arg_len = arg_len(self.index);
            result = str::from_utf8(slice::from_raw_parts(self.ptr, arg_len as usize)).unwrap();
            self.ptr = self.ptr.add(arg_len);
        }

        self.index += 1;
        Some(result)
    }
}

fn argc() -> usize {
    unsafe {
        let cursor = PROCESS_STACK_END.as_ptr::<u8>().sub(8);
        let argc: u64 = *(cursor as *const u64);
        argc as usize
    }
}

/// # Safety: index must be in bounds
unsafe fn arg_len(index: usize) -> usize {
    let cursor = PROCESS_STACK_END.as_ptr::<u8>().sub(16);
    let len_per_arg_start = cursor as *const u64;
    let arg_len: u64 = *len_per_arg_start.sub(index);
    arg_len as usize
}

pub fn args() -> Args {
    unsafe {
        let cursor = PROCESS_STACK_END.as_ptr::<u8>().sub(8);
        let argc: u64 = *(cursor as *const u64);
        let argc = argc as usize;

        let len_per_arg_start = cursor as *const u64;
        let len_per_arg = |i| -> usize {
            let arg_len: u64 = *len_per_arg_start.sub(i + 1);
            arg_len as usize
        };

        Args {
            index: 0,
            ptr: cursor
                .sub(argc * 8)
                .sub((0..argc).map(|i| len_per_arg(i)).sum()),
        }
    }
}
