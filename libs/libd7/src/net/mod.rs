pub use d7net;

pub mod tcp;
// pub mod udp;

use alloc::prelude::v1::*;

use d7net::SocketAddr;

use crate::{fs::File, syscall::SyscallResult};

fn create_socket(addr: SocketAddr) -> SyscallResult<File> {
    let f = File::open("/srv/net/newsocket")?;
    f.write_all(&pinecone::to_vec(&addr).unwrap())?;

    let mut buffer = [0u8; 64];
    let count = f.read(&mut buffer)?;
    assert!(0 < count && count < buffer.len());

    File::open(&format!("/srv/net/socket/{}", core::str::from_utf8(&buffer[..count]).unwrap()))
}