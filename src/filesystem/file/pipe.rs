use alloc::prelude::v1::Vec;

use super::super::{error::*, path::Path, FileClientId};
use super::{FileOps, Leafness};

/// Simplex FIFO communication channel,
/// which can be used for IPC messages.
///
/// Processes can obtain pipes from `/dev/pipe`.
#[derive(Debug)]
pub struct Pipe {
    /// Target file descriptor
    /// Buffer
    buffer: Vec<u8>,
    /// Max size for the buffer, in bytes. Must be nonzero.
    buffer_limit: u64,
}
