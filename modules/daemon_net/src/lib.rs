//! Networking daemon
//!
//! TODO: access control, maybe with capability tokens?

#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![deny(unused_must_use)]
#![deny(private_in_public)]
// XXX
#![allow(unused_imports)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::prelude::v1::*;
use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use libd7::{
    ipc::{self, SubscriptionId},
    net::{
        d7net::{ethernet, Ipv4Addr, MacAddr},
        socket::{SocketDescriptor, SocketOptions},
    },
    pinecone,
    process::{Process, ProcessId},
    select, service, syscall,
    syscall::{SyscallErrorCode, SyscallResult},
};

mod arp_cache;
mod handler;
mod my_info;
mod send;
mod socket;

use self::handler::Handlers;
use self::my_info::MyInfo;
use self::socket::{Sockets, TcpSocket, UdpSocket};

pub use self::send::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Driver {
    name: String,
    path: String,
}

pub struct NetState {
    pub my_info: my_info::MyInfo,
    /// Handlers for each EtherType packet.
    /// This is wrapped in option to allow temporary removal,
    /// so that handlers can have write access to self.
    pub handlers: Option<Handlers>,
    /// Sockets
    /// This is wrapped in option to allow temporary removal,
    /// so that handlers can have write access to self.
    pub sockets: Option<Sockets>,
}
impl NetState {
    pub fn new(mac: MacAddr) -> Self {
        Self {
            my_info: MyInfo {
                mac,
                ipv4: Some(Ipv4Addr([10, 0, 2, 15])), // Use a fixed IP until DHCP is implemented
                ipv6: None,
            },
            handlers: Some(Handlers::new()),
            sockets: Some(Sockets::new()),
        }
    }

    pub fn on_receive(&mut self, packet: &[u8]) {
        let frame = ethernet::Frame::from_bytes(&packet);

        println!(
            "Received {:?} packet from {:?}",
            frame.header.ethertype, frame.header.src_mac
        );

        // Temporarily remove handlers to allow mutable access to self
        let mut handlers = self.handlers.take().unwrap();
        if let Some(handler) = handlers.get_mut(&frame.header.ethertype) {
            handler.on_receive(self, &frame);
        } else {
            println!("Ignoring message with unhandled ethertype");
        }
        self.handlers = Some(handlers);
    }

    pub fn modify_sockets<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut Sockets) -> R,
    {
        // Temporarily remove sockets to allow mutable access to self
        let mut sockets = self.sockets.take().unwrap();
        let result = f(self, &mut sockets);
        self.sockets = Some(sockets);
        result
    }
}

/// Binds a socket and (on connection-oriented protocols) listens to it
fn handle_socket_listen(a: &ipc::Server<SocketOptions, SocketDescriptor>, net: &mut NetState) {
    a.handle(|binding| {
        match binding {
            SocketOptions::Tcp { host, port } => {
                if let Some((desc, s)) = net
                    .sockets
                    .as_mut()
                    .unwrap()
                    .create_tcp_listen(host.to_ipv6(), port)
                {
                    Ok(desc)
                } else {
                    Err(SyscallErrorCode::ipc_delivery_target_nack)
                }
            }
            other => panic!("Unknown socket type {:?}", binding), // TODO: client error
        }
    })
    .unwrap();
}

fn handle_socket_send(
    a: &ipc::ReliableSubscription<(SocketDescriptor, Vec<u8>)>,
    net: &mut NetState,
) {
    let (ack_ctx, (desc, data)) = a.receive().unwrap();
    let r = net.modify_sockets(|n, sockets| sockets.send(&n.my_info, desc, &data));
    if r.is_ok() {
        ack_ctx.ack().unwrap();
    } else {
        ack_ctx.nack().unwrap();
    }
}

fn handle_socket_close(a: &ipc::ReliableSubscription<SocketDescriptor>, net: &mut NetState) {
    let desc = a.ack_receive().unwrap();
    net.modify_sockets(|n, sockets| sockets.close(&n.my_info, desc))
        .unwrap();
}

#[no_mangle]
fn main() -> ! {
    println!("Network daemon starting");

    // Wait until a driver is available
    service::wait_for_one("driver_rtl8139");

    let mac_addr: MacAddr = match ipc::request("nic/rtl8139/mac", &()) {
        Ok(mac) => mac,
        Err(SyscallErrorCode::ipc_delivery_no_target) => {
            panic!("No NIC drivers available");
        }
        Err(err) => panic!("NIC ping failed {:?}", err),
    };

    let mut net_state = NetState::new(mac_addr);

    // Subscribe to messages
    let s_listen =
        ipc::Server::<SocketOptions, SocketDescriptor>::exact("netd/socket/listen").unwrap();
    let s_send =
        ipc::ReliableSubscription::<(SocketDescriptor, Vec<u8>)>::exact("netd/socket/send")
            .unwrap();
    let s_close =
        ipc::ReliableSubscription::<SocketDescriptor>::exact("netd/socket/close").unwrap();
    let received = ipc::ReliableSubscription::<Vec<u8>>::exact("netd/received").unwrap();

    // Announce that we are running
    libd7::service::register("netd", false);

    loop {
        select! {
            one(received) => {
                let packet = received.ack_receive().unwrap();
                net_state.on_receive(&packet);
            },
            one(s_listen) => handle_socket_listen(&s_listen, &mut net_state),
            one(s_send) => handle_socket_send(&s_send, &mut net_state),
            one(s_close) => handle_socket_close(&s_close, &mut net_state),
            error -> e => panic!("ERROR {:?}", e)
        };
    }
}
