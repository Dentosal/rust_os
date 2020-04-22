use d7net::{tcp, IpAddr, SocketAddr};

use crate::{
    pinecone,
    syscall::SyscallResult,
};

use super::create_socket;

/// A TCP socket
pub struct Socket {
    // file: File,
}
impl Socket {
    /// Bind to given host and port.
    /// Use `port = 0` to auto-assign a free port.
    pub fn bind(addr: SocketAddr) -> SyscallResult<Self> {
        todo!()
    }

    /// Accept a new incoming connection.
    ///
    /// This function will block the calling thread until a new TCP connection is established.
    /// When established, the corresponding Stream and the remote peer's address will be returned.
    pub fn accept(&self, addr: SocketAddr) -> SyscallResult<(Stream, SocketAddr)> {
        // let x = self.file.read();
        todo!();
    }
}

pub struct Stream {
    // file: File,
}
impl Stream {

}
