use d7net::{tcp, IpAddr, SocketAddr};

use crate::{
    attachment,
    fs::{self, File},
    pinecone, select,
    syscall::SyscallResult,
};

use super::create_socket;

/// A TCP socket server, listening for connections.
pub struct Socket {
    file: File,
}
impl Socket {
    /// Bind to given host and port.
    /// Use `port = 0` to auto-assign a free port.
    pub fn bind(addr: SocketAddr) -> SyscallResult<Self> {
        Ok(Self {
            file: create_socket(addr)?,
        })
    }
}
