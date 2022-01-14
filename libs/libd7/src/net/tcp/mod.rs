use alloc::string::String;

use d7net::SocketAddr;

use crate::{
    ipc,
    net::{NetworkError, ToSocketAddrs},
    syscall::{SyscallErrorCode, SyscallResult},
};

pub mod socket_ipc_protocol;

use self::socket_ipc_protocol as proto;

#[derive(Debug)]
pub enum Error {
    Bind(proto::BindError),
    Protocol(proto::Error),
    Syscall(SyscallErrorCode),
}
impl From<proto::BindError> for Error {
    fn from(e: proto::BindError) -> Error {
        Self::Bind(e)
    }
}
impl From<proto::Error> for Error {
    fn from(e: proto::Error) -> Error {
        Self::Protocol(e)
    }
}
impl From<NetworkError> for Error {
    fn from(e: NetworkError) -> Error {
        Self::Protocol(e.into())
    }
}
impl From<SyscallErrorCode> for Error {
    fn from(e: SyscallErrorCode) -> Error {
        Self::Syscall(e)
    }
}

/// A TCP connection
struct SocketInner {
    topic: String,
}
impl SocketInner {
    fn new(bind: SocketAddr) -> Result<Self, Error> {
        let r: Result<String, proto::BindError> =
            ipc::request("netd/newsocket/tcp", proto::Bind(bind))?;
        Ok(Self { topic: r? })
    }

    fn request(&self, request: proto::Request) -> Result<proto::Reply, Error> {
        let r: Result<proto::Reply, proto::Error> = ipc::request(&self.topic, request)?;
        Ok(r?)
    }
}

impl Drop for SocketInner {
    fn drop(&mut self) {
        log::debug!("Dropping TCP socket");
        let r = self.request(proto::Request::Remove);
        if let Err(e) = r {
            log::warn!("Dropping TCP socket failed: {:?}", e);
        }
    }
}

/// A TCP connection
pub struct Stream {
    inner: SocketInner,
}
impl Stream {
    /// Connect to a given host and port.
    /// Use `port = 0` to auto-assign a free port.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self, Error> {
        let inner = SocketInner::new(SocketAddr::ZERO)?;
        let r = inner.request(proto::Request::Connect {
            to: addr
                .to_socket_addrs()?
                .next()
                .ok_or(NetworkError::InvalidSocketAddr)?,
        })?;
        assert!(r == proto::Reply::NoData, "Invalid reply variant");
        Ok(Self { inner })
    }

    pub fn state(&self) -> Result<d7net::tcp::state::ConnectionState, Error> {
        let r = self.inner.request(proto::Request::GetState)?;
        let proto::Reply::State(state) = r else {
            unreachable!("Invalid reply variant");
        };
        Ok(state)
    }

    /// Close outgoing data stream
    pub fn shutdown(&self) -> Result<(), Error> {
        let r = self.inner.request(proto::Request::Shutdown)?;
        assert!(r == proto::Reply::NoData, "Invalid reply variant");
        Ok(())
    }

    pub fn close(&self) -> Result<(), Error> {
        let r = self.inner.request(proto::Request::Close)?;
        assert!(r == proto::Reply::NoData, "Invalid reply variant");
        Ok(())
    }

    pub fn abort(&self) -> Result<(), Error> {
        let r = self.inner.request(proto::Request::Abort)?;
        assert!(r == proto::Reply::NoData, "Invalid reply variant");
        Ok(())
    }

    pub fn send(&self, data: &[u8]) -> Result<(), Error> {
        let r = self.inner.request(proto::Request::Send(data.to_vec()))?;
        assert!(r == proto::Reply::NoData, "Invalid reply variant");
        Ok(())
    }

    pub fn recv(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        let r = self.inner.request(proto::Request::Recv(buffer.len()))?;
        let proto::Reply::Recv(data) = r else {
            unreachable!("Invalid reply variant");
        };
        buffer[..data.len()].copy_from_slice(&data);
        Ok(data.len())
    }
}

/// A TCP connection
pub struct Listener {
    inner: SocketInner,
}
impl Listener {
    /// Bind to given host and port.
    /// Use `port = 0` to auto-assign a free port.
    pub fn bind(addr: SocketAddr) -> Result<Self, Error> {
        let inner = SocketInner::new(addr)?;
        Ok(Self { inner })
    }

    /// Accept a new incoming connection.
    ///
    /// This function will block the calling thread until a new TCP connection is established.
    /// When established, the corresponding Stream and the remote peer's address will be returned.
    pub fn accept(&self, _addr: SocketAddr) -> SyscallResult<(Stream, SocketAddr)> {
        todo!("TCP Listening sockets are not supported yet");
    }

    pub fn state(&self) -> Result<d7net::tcp::state::ConnectionState, Error> {
        let r = self.inner.request(proto::Request::GetState)?;
        let proto::Reply::State(state) = r else {
            unreachable!("Invalid reply variant");
        };
        Ok(state)
    }

    pub fn close(&self) -> Result<(), Error> {
        let r = self.inner.request(proto::Request::Close)?;
        assert!(r == proto::Reply::NoData, "Invalid reply variant");
        Ok(())
    }
}
