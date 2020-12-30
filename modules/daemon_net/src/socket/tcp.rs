//! TODO: sequence/ack number verification

use libd7::{
    ipc,
    net::{
        d7net::{
            tcp::{ConnectionState, Segment},
            IpAddr, Ipv6Addr,
        },
        socket::{SocketDescriptor, SocketOptions},
    },
    prelude::*,
};

use crate::NetState;

pub fn gen_random_u32() -> u32 {
    let mut buffer = [0u8; 4];
    libd7::syscall::get_random(&mut buffer);
    u32::from_le_bytes(buffer)
}

#[derive(Debug)]
pub struct TcpClientState {
    pub state: ConnectionState,
    /// Send but unacknowledged bytes
    pub unacked: u32,
    /// Sequence number, i.e. start random value + cumulative number of send and ack'd payload bytes
    pub seqn: u32,
    /// Acknowledgement number, , i.e. cumulative number of received bytes
    pub ackn: u32,
}

#[derive(Debug)]
pub enum TcpSocket {
    /// This is a client socket
    Client(TcpClientState),
    /// This is a server socket, these are all clients it is handling
    /// Mapping remote -> state
    Server(HashMap<(Ipv6Addr, u16), TcpClientState>),
}
impl TcpSocket {
    pub fn new_server() -> Self {
        Self::Server(HashMap::new())
    }

    /// Returns a (acknowlede number, next sequence number) if sending is possible
    pub fn prepare_send(
        &mut self,
        remote: (Ipv6Addr, u16),
        payload_len: u32,
    ) -> Option<(u32, u32)> {
        match self {
            Self::Client(client) => {
                if client.state == ConnectionState::Established {
                    client.unacked += payload_len;
                    return Some((client.ackn, client.seqn));
                }
            }
            Self::Server(clients) => {
                if let Some(client) = clients.get_mut(&remote) {
                    if client.state == ConnectionState::Established {
                        client.unacked += payload_len;
                        return Some((client.ackn, client.seqn));
                    }
                }
            }
        }
        None
    }

    pub fn on_receive(
        &mut self,
        bound: (Ipv6Addr, u16), // address this socket is bound to
        dst: (Ipv6Addr, u16),   // destination from the ip packet
        src: (Ipv6Addr, u16),   // source from the ip packet
        segment: Segment,
    ) -> Option<Segment> {
        match self {
            Self::Client(client) => todo!("Client sockets are not supported yet"),
            Self::Server(clients) => {
                if let Some(client) = clients.get_mut(&src) {
                    if segment.header.is_normal() {
                        println!("SEQN {} + {}", client.seqn, client.unacked);
                        client.seqn += client.unacked;
                        client.unacked = 0;
                        client.ackn = client.ackn.wrapping_add(segment.payload.len() as u32);
                        let reply_ackn = segment
                            .header
                            .sequence
                            .wrapping_add(segment.payload.len() as u32);

                        // Send to the reading socket
                        if client.state == ConnectionState::SynReceived {
                            client.state = ConnectionState::Established;
                            println!("TCP ESTABLISHED");
                            let server_topic = SocketDescriptor::TcpServer { local: bound }.topic();
                            let client_desc = SocketDescriptor::TcpClient {
                                local: bound,
                                remote: src,
                            };
                            ipc::deliver_reply(&server_topic, &client_desc).unwrap();
                            return None;
                        } else if !segment.payload.is_empty() {
                            let client_topic = SocketDescriptor::TcpClient {
                                local: bound,
                                remote: src,
                            }
                            .topic();
                            ipc::deliver_reply(&client_topic, &segment.payload).unwrap();
                            // Response with an acknowledgement message
                            println!(
                                "DLT seqn={} ackn={}",
                                segment.header.sequence,
                                segment.payload.len()
                            );
                            println!("ACK seqn={} ackn={}", client.seqn, reply_ackn);
                            return Some(Segment {
                                header: segment.header.ack_reply(client.seqn, reply_ackn),
                                payload: Vec::new(),
                            });
                        }
                    }
                } else if segment.header.is_initialization() {
                    println!("New tcp connection from {:?} to {:?}", src, dst);
                    let ackn = gen_random_u32();
                    let mut client = TcpClientState {
                        state: ConnectionState::SynReceived,
                        unacked: 0,
                        seqn: segment.header.ack_number,
                        ackn,
                    };
                    clients.insert(src, client);
                    return Some(Segment {
                        header: segment.header.reply_to_initialization(ackn),
                        payload: Vec::new(),
                    });
                }
            }
        }

        println!("Unhandled segment {:?}", segment);

        None
    }
}
