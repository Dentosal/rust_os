//! Endpoints for network controller, until it's moved to a separate process, if ever

use alloc::prelude::v1::*;

use d7abi::fs::protocol::network::*;

use crate::driver::nic::NETWORK;
use crate::multitasking::ExplicitEventId;
use crate::multitasking::WaitFor;

use super::super::{error::*, FileClientId};
use super::{FileOps, Leafness};

/// `/dev/nic`
pub struct NetworkDevice;
impl FileOps for NetworkDevice {
    fn leafness(&self) -> Leafness {
        Leafness::Leaf
    }

    fn read(&mut self, _fd: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        let mut nw = NETWORK.try_lock().unwrap();
        let event = nw.received_queue.io_pop_event()?;
        let data = pinecone::to_vec(&event).expect("Couldn't serialize network event");
        assert!(data.len() <= buf.len(), "Buffer is too small"); // TODO: client error, not a kernel panic
        buf[..data.len()].copy_from_slice(&data);
        IoResult::Success(data.len())
    }

    fn read_waiting_for(&mut self, _fc: FileClientId) -> WaitFor {
        let mut nw = NETWORK.try_lock().unwrap();
        WaitFor::Event(nw.received_queue.get_event())
    }

    /// Whole packet must be written in one operation
    fn write(&mut self, _fd: FileClientId, buf: &[u8]) -> IoResult<usize> {
        let mut nw = NETWORK.try_lock().unwrap();
        let msg: OutboundPacket = pinecone::from_bytes(&buf).expect("Invalid outbound packet"); // TODO: client error, not a kernel panic
        nw.map(&mut |drv| drv.send(&msg.packet))
            .expect("No NIC connected"); // TODO: report error
        IoResult::Success(buf.len())
    }
}

/// `/dev/nic_mac`
pub struct MacAddrDevice;
impl FileOps for MacAddrDevice {
    fn leafness(&self) -> Leafness {
        Leafness::Leaf
    }

    fn read(&mut self, _fd: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        let mut nw = NETWORK.try_lock().unwrap();
        if let Some(mac) = nw.map(&mut |d| d.mac_addr()) {
            assert!(mac.len() <= buf.len(), "Buffer is too small"); // TODO: client error, not a kernel panic
            buf[..mac.len()].copy_from_slice(&mac);
            IoResult::Success(mac.len())
        } else {
            IoResult::Success(0)
        }
    }

    fn read_waiting_for(&mut self, _fc: FileClientId) -> WaitFor {
        WaitFor::None
    }
}
