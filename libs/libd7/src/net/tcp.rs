use d7net::{tcp, IpAddr, Ipv6Addr};

use crate::ipc::{self, ReliableSubscription};
use crate::syscall::SyscallResult;

use super::socket::{SocketDescriptor, SocketOptions};
use super::*;

#[derive(Debug)]
pub struct TcpListener {
    desc: SocketDescriptor,
    sub: ReliableSubscription<SocketDescriptor>,
}
impl TcpListener {
    /// Bind to given host and port and listen for connections
    /// Use `port = 0` to auto-assign a free port.
    pub fn listen(host: IpAddr, port: u16) -> SyscallResult<Self> {
        let (desc, sub) = create_socket_listen(SocketOptions::Tcp { host, port })?;
        Ok(Self { desc, sub })
    }

    /// Accept a new connection
    pub fn accept(&self) -> SyscallResult<TcpStream> {
        let desc = self.sub.ack_receive()?;
        let sub = ReliableSubscription::exact(&desc.topic())?;
        Ok(TcpStream { desc, sub })
    }
}

impl Drop for TcpListener {
    /// Close socket on drop
    fn drop(&mut self) {
        // Ignore errors
        let _ = ipc::deliver_reply("netd/socket/close", &self.desc);
    }
}

#[derive(Debug)]
pub struct TcpStream {
    desc: SocketDescriptor,
    sub: ReliableSubscription<Vec<u8>>,
}
impl TcpStream {
    /// Connect to a remote host
    pub fn connect(host: IpAddr, port: u16) -> SyscallResult<Self> {
        assert!(port != 0);
        todo!()
    }

    /// Get remote IP and port
    pub fn remote(&self) -> (Ipv6Addr, u16) {
        match self.desc {
            SocketDescriptor::TcpClient { local, remote } => remote,
            _ => unreachable!(),
        }
    }

    /// Receive bytes
    pub fn receive(&self) -> SyscallResult<Vec<u8>> {
        self.sub.ack_receive()
    }

    /// Send bytes
    pub fn send(&self, bytes: &[u8]) -> SyscallResult<()> {
        ipc::deliver("netd/socket/send", &(&self.desc, bytes))
    }
}

impl Drop for TcpStream {
    /// Close socket on drop
    fn drop(&mut self) {
        // Ignore errors
        let _ = ipc::deliver("netd/socket/close", &self.desc);
    }
}
