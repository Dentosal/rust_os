use alloc::vec::Vec;

use super::SocketAddr;
use crate::ipc;

pub struct UdpSocket {
    recv: ipc::UnreliableSubscription<>,
    send_topic: String,
}
impl UdpSocket {
    pub fn bind(addr: SocketAddr) -> SyscallResult<Self> {
        let socket = ipc::request("netd/newsocket/udp");
    }
}