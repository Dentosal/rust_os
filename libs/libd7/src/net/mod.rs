#![allow(unreachable_code)] // TODO remove this

pub use d7net;

pub mod tcp;
// pub mod udp;

use alloc::vec::Vec;
use d7net::SocketAddr;

use crate::syscall::SyscallResult;

fn create_socket(addr: SocketAddr) -> SyscallResult<!> {
    todo!()
    // let f = File::open("/srv/net/newsocket")?;
    // f.write_all(&pinecone::to_vec(&addr).unwrap())?;

    // let mut buffer = [0u8; 10];
    // let count = f.read(&mut buffer)?;
    // assert!(0 < count && count < buffer.len());

    // let socket_id: u64 = pinecone::from_bytes(&buffer[..count]).expect("Invalid socket id response");
    // File::open(&format!("/srv/net/socket/{}", socket_id))
}
