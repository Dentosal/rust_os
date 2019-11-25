use alloc::prelude::v1::*;
use hashbrown::HashMap;

use d7abi::fs::{protocol, FileInfo};

use super::super::{error::IoResult, node::NodeId, FileClientId};
use super::{FileOps, Leafness};

/// Branch that doesn't require attached process, but
/// is instead managed on the vfs level.
#[derive(Debug, Clone)]
pub struct InternalBranch {
    children: HashMap<String, NodeId>,
    /// Readers and their snapshots of data.
    /// Data is already formatted for reading, and is
    /// stored in reverse order for fast `pop` operations.
    /// Read bytes are removed from the buffer.
    readers: HashMap<FileClientId, Vec<u8>>,
}
impl InternalBranch {
    pub fn new() -> Self {
        Self {
            children: HashMap::new(),
            readers: HashMap::new(),
        }
    }

    /// Formats contents for reading.
    /// Provides entries in arbitrary order.
    fn format_contents(&self, is_kernel: bool) -> Vec<u8> {
        let mut result = if is_kernel {
            pinecone::to_vec(&self.children).unwrap()
        } else {
            pinecone::to_vec(&protocol::ReadBranch {
                items: self.children.keys().cloned().collect(),
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
        Ok(count)
    }

    /// TODO: Currently doesn't buffer incoming data, so requires every single message to
    /// contain at least one full value. Multiple values are not read, but at least the
    /// number of read bytes is reported correctly.
    fn write(&mut self, fc: FileClientId, buf: &[u8]) -> IoResult<usize> {
        if fc.is_kernel() {
            let opt_msg: Result<(String, NodeId), _> = pinecone::from_bytes(buf);
            match opt_msg {
                Ok((node_name, node_id)) => {
                    self.children.insert(node_name, node_id);
                    Ok(buf.len())
                },
                Err(pinecone::Error::DeserializeUnexpectedEnd) => unimplemented!("Partial request"),
                Err(other) => panic!("Deser error {:?}", other),
            }
        } else {
            unimplemented!("Syscall::write node creation") // TODO
        }
    }
}
