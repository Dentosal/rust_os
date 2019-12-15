use alloc::collections::VecDeque;
use alloc::prelude::v1::*;
use hashbrown::HashMap;

use d7abi::fs::protocol::attachment::*;
use d7abi::fs::FileDescriptor;

use crate::multitasking::{ExplicitEventId, ProcessId, WaitFor};

use super::super::{error::IoResult, node::NodeId, FileClientId};
use super::{FileOps, Leafness, Trigger};

/// # Attachment point
/// This node and its contents are managed by a driver
/// software. On branch nodes, the driver can provide
/// child nodes that are used. The nodes themselves,
/// however, must exist before a read is attempted.
/// ## Nesting attach points
/// Nested mounts are allowed.
/// The innermost mount point will receive all operations.
/// ## Unattaching
/// Unlike Linux, where unmounting requires that all inner
/// mounts are unmounted first, this implementation simply
/// allows unattaching the point, causing all inner attachments
/// to be closed as well.
#[derive(Debug, Clone)]
pub struct Attachment {
    /// Process and file descriptor managing this attachment
    manager: FileClientId,
    /// Leafness is a static property of an attachment,
    /// the controlling process cannot change this
    is_leaf: bool,
    /// This attachements manager is trying to read or write,
    /// but is blocked until this event here
    manager_wait_id: Option<ExplicitEventId>,
    /// Pending reads, served in order. Contains `(reader, event)` pairs.
    /// The events here are client wakeup ids.
    reads_pending: VecDeque<(FileClientId, ExplicitEventId)>,
    /// Reads in progress. This is used store client wakeup ids.
    reads_in_progress: HashMap<FileClientId, ExplicitEventId>,
    /// Completed reads. Data is removed when read. When all data has been removed,
    /// and the queue is empty, the entry here will be removed as well.
    reads_completed: HashMap<FileClientId, VecDeque<u8>>,
}
impl Attachment {
    pub fn new(manager: FileClientId, is_leaf: bool) -> Self {
        Self {
            manager,
            is_leaf,
            manager_wait_id: None,
            reads_pending: VecDeque::new(),
            reads_in_progress: HashMap::new(),
            reads_completed: HashMap::new(),
        }
    }
}
impl FileOps for Attachment {
    fn leafness(&self) -> Leafness {
        if self.is_leaf {
            Leafness::Leaf
        } else {
            Leafness::Branch
        }
    }

    fn read(&mut self, fc: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        if fc == self.manager {
            // Reading from attachment fd
            if let Some((reader_fc, event_id)) = self.reads_pending.pop_front() {
                // The next client is trying to read
                let req = Message {
                    sender_pid: reader_fc.process.expect("TODO? kernel"),
                    sender_f: unsafe { reader_fc.fd.as_u64() },
                    type_: FileOperationType::Read,
                    data: Vec::new(),
                };

                let bytes = pinecone::to_vec(&req).expect("Could not serialize");
                if bytes.len() <= buf.len() {
                    buf[..bytes.len()].copy_from_slice(&bytes);
                } else {
                    // TODO: Process error, not kernel panic
                    panic!("Target buffer not large enough");
                }
                // Mark the read to be in progress
                // TODO: Assert not alredy existing? (for debugging only)
                self.reads_in_progress.insert(reader_fc, event_id);
                IoResult::Success(bytes.len())
            } else {
                // No reads pending, wait until some other process tries to read
                let wait_id = WaitFor::new_event_id();
                self.manager_wait_id = Some(wait_id);
                IoResult::RepeatAfter(WaitFor::Event(wait_id))
            }
        } else if let Some(mut data) = self.reads_completed.remove(&fc) {
            let mut i = 0;
            while i < buf.len() {
                if let Some(byte) = data.pop_front() {
                    buf[i] = byte;
                    i += 1;
                } else {
                    break;
                }
            }
            if !data.is_empty() {
                // Insert back
                self.reads_completed.insert(fc, data);
            }
            IoResult::Success(i)
        } else {
            assert!(
                self.reads_pending
                    .iter()
                    .find(|(pending_fc, _)| { *pending_fc == fc })
                    .is_none()
            ); // TODO: remove this?

            let event_id = WaitFor::new_event_id();
            self.reads_pending.push_back((fc, event_id));

            let repeat = IoResult::RepeatAfter(WaitFor::Event(event_id));
            if let Some(event_id) = self.manager_wait_id {
                IoResult::TriggerEvent(event_id, Box::new(repeat))
            } else {
                repeat
            }
        }
    }

    fn read_waiting_for(&mut self, fc: FileClientId) -> WaitFor {
        if fc == self.manager {
            self.manager_wait_id
                .map(WaitFor::Event)
                .unwrap_or(WaitFor::None)
        } else if let Some((_, event_id)) = self
            .reads_pending
            .iter()
            .find(|(pending_fc, _)| *pending_fc == fc)
        {
            WaitFor::Event(*event_id)
        } else {
            WaitFor::None
        }
    }

    fn write(&mut self, fc: FileClientId, buf: &[u8]) -> IoResult<usize> {
        if fc == self.manager {
            // Manager writes response to a request. The whole response must be written at once.
            let (message, rest): (Message, &[u8]) =
                pinecone::take_from_bytes(buf).expect("Partial write from manager");

            let client_fc = FileClientId::process(message.sender_pid, unsafe {
                FileDescriptor::from_u64(message.sender_f)
            });

            let client_wakeup_event = self
                .reads_in_progress
                .remove(&client_fc)
                .expect("Client does not exist");

            self.reads_completed.insert(client_fc, message.data.into());
            IoResult::TriggerEvent(
                client_wakeup_event,
                Box::new(IoResult::Success(buf.len() - rest.len())),
            )
        } else {
            // Writes to attachments must be of type `d7abi::fs::protocol::attachment::Request`,
            // and the whole request must be written at once

            // let (req, rest): (Request, &[u8]) = pinecone::take_from_bytes(buf).except("Partial read");
            // panic!("R {:?}", req);
            // Ok(buf.len() - rest.len())
            unimplemented!()
        }
    }

    /// Trigger all waiting processes
    fn destroy(&mut self) -> Trigger {
        Trigger::events(
            self.reads_pending
                .iter()
                .map(|(_, w)| w)
                .chain(self.reads_in_progress.values())
                .copied()
                .collect(),
        )
    }
}
