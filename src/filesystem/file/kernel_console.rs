//! Kernel console device, linked with the physical keyboard and text console

use alloc::collections::VecDeque;
use alloc::prelude::v1::*;

use d7abi::fs::protocol::console::*;

use crate::driver::keyboard::KEYBOARD;
use crate::multitasking::ExplicitEventId;
use crate::multitasking::WaitFor;

use super::super::{error::*, FileClientId};
use super::{FileOps, Leafness};

/// `/dev/console`
pub struct KernelConsoleDevice {
    current_line: String,
}
impl KernelConsoleDevice {
    pub fn new() -> Self {
        Self {
            current_line: String::new(),
        }
    }
}
impl FileOps for KernelConsoleDevice {
    fn leafness(&self) -> Leafness {
        Leafness::Leaf
    }

    /// Reads from the physical keyboard
    fn read(&mut self, _fd: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        let mut kbd = KEYBOARD.try_lock().unwrap();
        if let Some(event) = kbd.pop_event_nonblocking() {
            let data = pinecone::to_vec(&event).expect("Couldn't serialize keyboard event");
            assert!(data.len() <= buf.len(), "Buffer is too small"); // TODO: client error, not a kernel panic
            buf[..data.len()].copy_from_slice(&data);
            IoResult::Success(data.len())
        } else {
            IoResult::RepeatAfter(WaitFor::Event(kbd.get_event()))
        }
    }

    fn read_waiting_for(&mut self, _fc: FileClientId) -> WaitFor {
        let mut kbd = KEYBOARD.try_lock().unwrap();
        WaitFor::Event(kbd.get_event())
    }

    /// Discards all data
    fn write(&mut self, _fd: FileClientId, buf: &[u8]) -> IoResult<usize> {
        // TODO: client error, not kernel panic
        let tu: TextUpdate = pinecone::from_bytes(&buf).expect("Invalid message");

        assert!(tu.line.len() < 80); // TODO: support line longer than 80 chars, or at least do not crash

        // TODO: proper implementation
        rclearline!();
        rprint!("{}", tu.line);
        if tu.newline {
            rprintln!("");
        }

        IoResult::Success(buf.len())
    }
}