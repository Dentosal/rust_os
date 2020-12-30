use alloc::prelude::v1::*;
use serde::de::DeserializeOwned;

use crate::{
    ipc::{self, ReliableSubscription},
    syscall::SyscallResult,
};

pub use d7net;

pub mod socket;
pub mod tcp;
// pub mod udp;

use self::socket::{SocketDescriptor, SocketOptions};

fn create_socket_listen<P: DeserializeOwned>(
    options: SocketOptions,
) -> SyscallResult<(SocketDescriptor, ReliableSubscription<P>)> {
    let desc: SocketDescriptor = ipc::request("netd/socket/listen", &options)?;
    let sub = ReliableSubscription::<P>::exact(&desc.topic())?;
    Ok((desc, sub))
}
