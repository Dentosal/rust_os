use libd7::net::{
    d7net::{
        tcp::{ConnectionState, Segment, SegmentHeader},
        IpAddr, IpProtocol, Ipv6Addr,
    },
    socket::{SocketDescriptor, SocketOptions},
};
use libd7::prelude::*;

use crate::my_info::MyInfo;
use crate::NetState;

mod tcp;

pub use self::tcp::TcpSocket;

/// Socket address spaces.
/// Note that all ip addresses here are Ipv6-addresses.
/// Ipv4-addresses must be converted before lookup/insertion.
#[derive(Debug)]
pub struct Sockets {
    pub tcp: HashMap<(Ipv6Addr, u16), TcpSocket>,
    pub udp: HashMap<(Ipv6Addr, u16), UdpSocket>,
}
impl Sockets {
    pub fn new() -> Self {
        Self {
            tcp: HashMap::new(),
            udp: HashMap::new(),
        }
    }

    pub fn create_tcp_listen(
        &mut self,
        host: Ipv6Addr,
        mut port: u16,
    ) -> Option<(SocketDescriptor, &mut TcpSocket)> {
        let key = (host, port);
        if port == 0 {
            todo!("Autoselect port");
        }
        if self.tcp.contains_key(&key) {
            None
        } else {
            println!("Creatiung a TCP server socket {:?}", key);
            self.tcp.insert(key, TcpSocket::new_server());
            Some((
                SocketDescriptor::TcpServer { local: key },
                self.tcp.get_mut(&key).unwrap(),
            ))
        }
    }

    pub fn send(&mut self, my: &MyInfo, desc: SocketDescriptor, data: &[u8]) -> Result<(), ()> {
        match desc {
            SocketDescriptor::TcpServer { .. } => return Err(()), // Cannot send to a server socket
            SocketDescriptor::TcpClient { local, remote } => {
                if let Some((ackn, seqn)) = self
                    .tcp
                    .get_mut(&local)
                    .ok_or(())?
                    .prepare_send(remote, data.len() as u32)
                {
                    let segment = Segment {
                        header: SegmentHeader::new(local.1, remote.1, ackn, seqn),
                        payload: data.to_vec(),
                    };
                    crate::send_ip_packet(my, remote.0, IpProtocol::TCP, |h| segment.to_bytes(h))
                } else {
                    Err(()) // Invalid socket state for sending
                }
            }
            SocketDescriptor::Udp { .. } => todo!("Send udp packet"),
        }
    }

    pub fn close(&mut self, my: &MyInfo, desc: SocketDescriptor) -> Result<(), ()> {
        match desc {
            SocketDescriptor::TcpServer { local } => {
                let s = self.tcp.get_mut(&local).ok_or(())?;
                match s {
                    TcpSocket::Client(_client) => unreachable!(),
                    TcpSocket::Server(clients) => {
                        for (remote, client) in clients.iter_mut() {
                            client.state = ConnectionState::FinWait1;
                            let segment = Segment {
                                header: SegmentHeader::new(
                                    local.1,
                                    remote.1,
                                    client.ackn,
                                    client.seqn,
                                ),
                                payload: Vec::new(),
                            };
                            crate::send_ip_packet(my, remote.0, IpProtocol::TCP, |h| {
                                segment.to_bytes(h)
                            })?;
                        }
                    }
                }
            }
            SocketDescriptor::TcpClient { local, remote } => {
                let s = self.tcp.get_mut(&local).ok_or(())?;
                match s {
                    TcpSocket::Client(client) => {
                        client.state = ConnectionState::FinWait1;
                        let segment = Segment {
                            header: SegmentHeader::new(local.1, remote.1, client.ackn, client.seqn),
                            payload: Vec::new(),
                        };
                        crate::send_ip_packet(my, remote.0, IpProtocol::TCP, |h| {
                            segment.to_bytes(h)
                        })?;
                    }
                    TcpSocket::Server(clients) => {
                        for (remote, client) in clients.iter_mut() {
                            client.state = ConnectionState::FinWait1;
                            let segment = Segment {
                                header: SegmentHeader::new(
                                    local.1,
                                    remote.1,
                                    client.ackn,
                                    client.seqn,
                                ),
                                payload: Vec::new(),
                            };
                            crate::send_ip_packet(my, remote.0, IpProtocol::TCP, |h| {
                                segment.to_bytes(h)
                            })?;
                        }
                    }
                }
            }
            SocketDescriptor::Udp { .. } => todo!("Close udp socket"),
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct UdpSocket {
    /// Buffer for incoming messages
    buffer: VecDeque<Ipv6Addr>,
}
