use alloc::prelude::v1::*;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use d7abi::fs::protocol;

use crate::multitasking::WaitFor;

use super::super::{error::IoResult, node::NodeId, FileClientId};
use super::{CloseAction, FileOps, Leafness};

/// Branch that doesn't require attached process, but
/// is instead managed on the vfs level.
#[derive(Debug, Clone)]
pub struct InternalBranch {
    /// Vec used to emulate binary mapping
    children: Vec<(NodeId, String)>,
    /// Readers and their snapshots of data.
    /// Data is already formatted for reading, and is
    /// stored in reverse order for fast `pop` operations.
    /// Read bytes are removed from the buffer.
    readers: HashMap<FileClientId, Vec<u8>>,
}
impl InternalBranch {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            readers: HashMap::new(),
        }
    }

    /// Formats contents for reading.
    /// Provides entries in arbitrary order.
    fn format_contents(&self, is_kernel: bool) -> Vec<u8> {
        let mut result = if is_kernel {
            pinecone::to_vec(&InternalRead(self.children.clone())).unwrap()
        } else {
            pinecone::to_vec(&protocol::ReadBranch {
                items: self.children.iter().map(|(_, n)| n.clone()).collect(),
            })
            .unwrap()
        };
        result.reverse();
        result
    }
}
impl FileOps for InternalBranch {
    fn leafness(&self) -> Leafness {
        Leafness::InternalBranch
    }

    /// Provides next bytes from reader buffer.
    /// See `format_contents` for explanation of the format.
    fn read(&mut self, fc: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        if !self.readers.contains_key(&fc) {
            let content = self.format_contents(fc.is_kernel());
            self.readers.insert(fc, content);
        }
        let reader_buf = self.readers.get_mut(&fc).unwrap();
        let mut count = 0;
        while count < buf.len() {
            if let Some(byte) = reader_buf.pop() {
                buf[count] = byte;
                count += 1;
            } else {
                break;
            }
        }
        IoResult::Success(count)
    }

    fn read_waiting_for(&mut self, fc: FileClientId) -> WaitFor {
        WaitFor::None
    }

    /// TODO: Currently doesn't buffer incoming data, so requires every single message to
    /// contain at least one full value. Multiple values are not read, but at least the
    /// number of read bytes is reported correctly.
    fn write(&mut self, fc: FileClientId, buf: &[u8]) -> IoResult<usize> {
        if fc.is_kernel() {
            let opt_msg: Result<InternalModification, _> = pinecone::from_bytes(buf);
            match opt_msg {
                Ok(InternalModification::Add(node_id, node_name)) => {
                    // Replace name if it exists, otherwise add new
                    let mut found = false;
                    for c in self.children.iter_mut() {
                        if c.1 == node_name {
                            c.0 = node_id;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        self.children.push((node_id, node_name));
                    }
                    IoResult::Success(buf.len())
                },
                Ok(InternalModification::Remove(node_id)) => {
                    // Remove child if it exists
                    let mut index: Option<usize> = None;
                    for (i, c) in self.children.iter_mut().enumerate() {
                        if c.0 == node_id {
                            index = Some(i);
                            break;
                        }
                    }
                    if let Some(i) = index {
                        self.children.remove(i);
                    }
                    IoResult::Success(buf.len())
                },
                Err(pinecone::Error::DeserializeUnexpectedEnd) => unimplemented!("Partial request"),
                Err(other) => panic!("Deser error {:?}", other),
            }
        } else {
            unimplemented!("Syscall::write node creation") // TODO
        }
    }

    /// Remove buffers when closing
    fn close(&mut self, fd: FileClientId) -> IoResult<CloseAction> {
        self.readers.remove(&fd);
        IoResult::Success(CloseAction::Normal)
    }
}

/// Filesystem-internal protocol modification operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InternalModification {
    Add(NodeId, String),
    Remove(NodeId),
}

/// Filesystem-internal protocol read operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalRead(Vec<(NodeId, String)>);
