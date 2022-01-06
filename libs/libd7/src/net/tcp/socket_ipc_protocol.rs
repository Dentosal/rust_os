use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::net::{NetworkError, SocketId};
use d7net::{tcp, SocketAddr};

#[derive(Debug, Serialize, Deserialize)]
pub struct Bind(pub SocketAddr);

#[derive(Debug, Serialize, Deserialize)]
#[must_use]
pub enum BindError {
    /// Caller-requested address already in use
    AlreadyInUse,
    /// Caller-requested address was not acceptable,
    /// e.g. binding to non-available IP
    NotAcceptable,
    /// Caller requested automatically assigned port,
    /// but no ports are available for that
    NoPortsAvailable,
    /// Caller cannot bind to this address
    PermissionDenied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    GetState,
    GetOption(OptionKey),
    SetOption(Option),
    Connect {
        to: SocketAddr,
    },
    Listen {
        backlog: usize,
    },
    Accept,
    Shutdown,
    Close,
    Abort,
    Recv(usize),
    Send(Vec<u8>),
    /// A special request used to indicate that this socket is
    /// no longer used, sent by the Drop impl. Must be replied
    /// with a success reply.
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reply {
    State(tcp::state::ConnectionState),
    Option(Option),
    Recv(Vec<u8>),
    Accept { addr: SocketAddr, id: SocketId },
    NoData,
}

impl From<()> for Reply {
    fn from(_: ()) -> Self {
        Self::NoData
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
    Network(NetworkError),
    Tcp(tcp::state::Error),
}
impl From<tcp::state::Error> for Error {
    fn from(error: tcp::state::Error) -> Self {
        Self::Tcp(error)
    }
}
impl From<NetworkError> for Error {
    fn from(error: NetworkError) -> Self {
        Self::Network(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionKey {
    NagleDelay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Option {
    NagleDelay(core::time::Duration),
}
