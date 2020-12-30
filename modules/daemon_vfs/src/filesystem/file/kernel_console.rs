//! Kernel console device, linked with the physical keyboard and text console

use alloc::collections::VecDeque;
use alloc::prelude::v1::*;

use d7abi::fs::protocol::console::*;

use crate::driver::keyboard::KEYBOARD;
use crate::multitasking::ExplicitEventId;
use crate::multitasking::WaitFor;

use super::super::{result::*, FileClientId};
use super::FileOps;

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
    /// Reads from the physical keyboard
    fn read(&mut self, _fd: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        let mut kbd = KEYBOARD.try_lock().unwrap();
        let (kbd_event, ctx) = kbd.event_queue.io_pop_event()?;
        let data = pinecone::to_vec(&kbd_event).expect("Couldn't serialize keyboard event");
        assert!(data.len() <= buf.len(), "Buffer is too small"); // TODO: client error, not a kernel panic
        buf[..data.len()].copy_from_slice(&data);
        IoResult::success(data.len()).with_context(ctx)
    }

    fn read_waiting_for(&mut self, _fc: FileClientId) -> WaitFor {
        let mut kbd = KEYBOARD.try_lock().unwrap();
        WaitFor::Event(kbd.event_queue.get_event())
    }

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

        IoResult::success(buf.len())
    }
}
