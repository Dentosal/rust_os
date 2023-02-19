use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::HashMap;

use libd7::{
    ipc::{self, InternalSubscription, SubscriptionId},
    net::tcp::socket_ipc_protocol::{BindError, Error, Reply, Request},
    net::{d7net::*, NetworkError, SocketId},
    random, time,
};

use crate::{ports, NET_STATE};

use super::new_socket_id;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TcpTime(pub time::Instant);
impl tcp::state::UserTime for TcpTime {
    fn now() -> Self {
        Self(time::Instant::now())
    }
    fn add(&self, duration: time::Duration) -> Self {
        Self(self.0.checked_add(duration).unwrap())
    }
}

#[derive(Debug)]
enum SuspendMode {
    Continue,
    Retry(Request),
}

struct SocketData {
    handler: SocketHandler,
    local_port: u16,
    send_error: Option<NetworkError>,
    events_suspended:
        HashMap<tcp::state::Cookie, (SuspendMode, ipc::ReplyCtx<Result<Reply, Error>>)>,
    events_ready: Vec<(
        SuspendMode,
        ipc::ReplyCtx<Result<Reply, Error>>,
        Result<(), tcp::state::Error>,
    )>,
}

impl SocketData {
    fn send_inner(
        &mut self, to: SocketAddr, seg: tcp::state::SegmentMeta,
    ) -> Result<(), NetworkError> {
        let (dst_mac, src_mac, src_ip) = {
            let net_state = NET_STATE.try_read().expect("NET_STATE locked");

            let intf = net_state
                .default_send_interface()
                .ok_or(NetworkError::NoInterfaces)?;

            let router_ip = intf
                .settings
                .routers
                .first()
                .ok_or(NetworkError::NoRouters)?;

            let router_mac = net_state
                .arp_table
                .get(router_ip)
                .ok_or(NetworkError::NoArpEntry)?;

            let ip_addr = intf.settings.ipv4.ok_or(NetworkError::NoIpAddr)?;

            (*router_mac, intf.mac_addr, ip_addr)
        };
        let src_port = self.local_port;

        let dst_ip = match to.host {
            IpAddr::V4(addr) => addr,
            IpAddr::V6(_) => todo!("IPv6 support"),
        };
        let dst_port = to.port;

        let payload = builder::ipv4_tcp::Builder::new(
            src_ip,
            dst_ip,
            src_port,
            dst_port,
            seg.seqn.raw(),
            seg.ackn.raw(),
            seg.window,
            seg.flags,
            seg.data,
        );

        println!("send payload {:?}", payload);

        let ef = ethernet::Frame {
            header: ethernet::FrameHeader {
                // dst_mac: MacAddr([0x52, 0x55, 0x0a, 0x00, 0x02, 0x02]), // XXX
                dst_mac,
                src_mac,
                ethertype: EtherType::Ipv4,
            },
            payload: payload.build(),
        };

        let mut packet = ef.to_bytes();
        while packet.len() < 64 {
            packet.push(0);
        }

        ipc::publish("nic/send", &packet).expect("Delivery failed");
        Ok(())
    }
}

impl tcp::state::UserData for SocketData {
    type Time = TcpTime;
    type Addr = SocketAddr;

    fn new_seqn(&mut self) -> u32 {
        // TODO: use a clock instead of random
        let arr = random::fast_arr();
        u32::from_le_bytes(arr)
    }

    fn send(&mut self, to: SocketAddr, seg: tcp::state::SegmentMeta) {
        println!("send {:?} to {:?}", seg, to);
        match self.send_inner(to, seg) {
            Ok(()) => {},
            Err(err) => self.send_error = Some(err),
        }
    }

    fn event(&mut self, cookie: tcp::state::Cookie, result: Result<(), tcp::state::Error>) {
        log::debug!("Event {:?} result {:?}", cookie, result);
        let (smode, reply_ctx) = self
            .events_suspended
            .remove(&cookie)
            .expect("No such event");
        self.events_ready.push((smode, reply_ctx, result));
    }

    fn add_timeout(&mut self, _c: TcpTime) {
        println!("TODO: add timeout"); // TODO
    }
}

pub struct SocketHandler {
    /// Option is used here to allow swapping the server out temporarily
    msg_subscription: ipc::Server<Request, Result<Reply, Error>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Binding {
    local: SocketAddr,
    remote: Option<SocketAddr>,
}
impl Binding {
    /// Anything as long as the port matches
    pub fn match_any(local_port: u16) -> Self {
        Self {
            local: SocketAddr {
                host: IpAddr::V4(Ipv4Addr::ZERO),
                port: local_port,
            },
            remote: None,
        }
    }
}

fn new_user_handler() -> SocketHandler {
    let bytes: [u8; 16] = random::crypto_arr();
    let v = u128::from_le_bytes(bytes);
    let topic_name = format!("netd/tcp/socket/{}", v);

    SocketHandler {
        msg_subscription: ipc::Server::pipe(&topic_name).expect("IPC server creation failed"),
    }
}

pub struct TcpHandler {
    bindings: HashMap<Binding, SocketId>,
    sockets: HashMap<SocketId, tcp::state::Socket<SocketData>>,
}
impl TcpHandler {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            sockets: HashMap::new(),
        }
    }

    pub fn new_user_socket(&mut self, port: u16) -> Result<String, BindError> {
        let id = new_socket_id();

        let bytes: [u8; 16] = random::crypto_arr();
        let v = u128::from_le_bytes(bytes);
        let topic_name = format!("netd/tcp/socket/{}", v);

        let local_port = if port != 0 {
            port
        } else {
            self.pick_free_port().ok_or(BindError::NoPortsAvailable)?
        };

        self.sockets.insert(
            id,
            tcp::state::Socket::new(SocketData {
                handler: SocketHandler {
                    msg_subscription: ipc::Server::pipe(&topic_name)
                        .expect("IPC server creation failed"),
                },
                local_port,
                send_error: None,
                events_suspended: HashMap::new(),
                events_ready: Vec::new(),
            }),
        );

        self.bindings.insert(Binding::match_any(local_port), id);

        Ok(topic_name)
    }

    /// Returns None if no ports are available
    /// TODO: should this be ip-binding dependent, i.e. can you bind
    /// different services to 127.0.0.1:80 and 192.168.1.123:80 ?
    fn pick_free_port(&self) -> Option<u16> {
        // Try fast random find
        for _ in 0..10 {
            let port = ports::random_dynamic_port();
            if self.socket_for(Binding::match_any(port)).is_none() {
                return Some(port);
            }
        }

        log::warn!("TCP port picker falling back to slow linear scan");

        for port in ports::RANGE_DYNAMIC {
            if self.socket_for(Binding::match_any(port)).is_none() {
                return Some(port);
            }
        }

        log::warn!("No free dynamic TCP ports found");
        None
    }

    /// Returns a set of subscription ids usable by ipc_select
    pub fn subscriptions(&self) -> impl Iterator<Item = (SubscriptionId, SocketId)> + '_ {
        self.sockets
            .iter()
            .filter_map(|(s_id, s)| Some((s.user_data().handler.msg_subscription.sub_id(), *s_id)))
    }

    /// User-socket has IPC event available, process it
    pub fn user_socket_event(&mut self, socket_id: SocketId) {
        let reply_ctx;
        let request;

        let socket = self
            .handler_for(socket_id)
            .expect("Socket has been removed incorrectly");

        let SocketHandler { msg_subscription } = &mut socket.user_data_mut().handler;
        (reply_ctx, request) = msg_subscription.receive().expect("TODO: handle disconnect");

        log::trace!("User request (socket={:?}): {:?}", socket_id, request);

        let check_events = self.user_socket_event_inner(socket_id, request, reply_ctx);

        if check_events {
            self.process_events(socket_id);
        }
    }

    #[must_use = "Event processing"]
    fn user_socket_event_inner(
        &mut self, socket_id: SocketId, request: Request,
        reply_ctx: ipc::ReplyCtx<Result<Reply, Error>>,
    ) -> bool {
        let mut accepted_new_socket = None;

        let reply = {
            let socket = self
                .handler_for(socket_id)
                .expect("Socket has been removed incorrectly");

            log::trace!("User request (socket={:?}): {:?}", socket_id, request);

            if socket.user_data().send_error.is_some() {
                match &request {
                    Request::Remove => {},
                    _ => {
                        let err: NetworkError = socket.user_data_mut().send_error.take().unwrap();
                        reply_ctx
                            .reply(Err(err.into()))
                            .expect("TODO: disconnected");
                        return true;
                    },
                }
            }

            log::debug!("User request {:?}", &request);

            match request.clone() {
                Request::Remove => {
                    let mut s = self
                        .sockets
                        .remove(&socket_id)
                        .expect("Socket has been removed incorrectly");
                    let _ = self.bindings.drain_filter(|_, b| *b == socket_id);
                    let r = s.call_abort().map(|()| Reply::NoData).map_err(|e| e.into());
                    let _ = reply_ctx.reply(r); // Ignore client errors after remove
                    return false;
                },
                Request::Accept => {
                    match socket.call_accept(|parent| SocketData {
                        handler: new_user_handler(),
                        local_port: parent.user_data().local_port,
                        send_error: None,
                        events_suspended: HashMap::new(),
                        events_ready: Vec::new(),
                    }) {
                        Ok((addr, socket)) => {
                            let new_id = new_socket_id();
                            accepted_new_socket = Some((new_id, socket));
                            Ok(Reply::Accept { addr, id: new_id })
                        },
                        Err(err) => Err(err),
                    }
                },
                Request::GetState => Ok(Reply::State(socket.state())),
                Request::GetOption(_option_key) => todo!("GetOption"),
                Request::SetOption(_option) => todo!("SetOption"),
                Request::Connect { to } => socket.call_connect(to).map(|()| Reply::NoData),
                Request::Listen { backlog } => socket.call_listen(backlog).map(|()| Reply::NoData),
                Request::Shutdown => socket.call_shutdown().map(|()| Reply::NoData),
                Request::Close => socket.call_close().map(|()| Reply::NoData),
                Request::Abort => socket.call_abort().map(|()| Reply::NoData),
                Request::Send(data) => socket.call_send(data).map(|()| Reply::NoData),
                Request::Recv(n) => {
                    let mut buffer = vec![0; n];
                    match socket.call_recv(&mut buffer) {
                        Ok(r) => {
                            buffer.truncate(r);
                            Ok(Reply::Recv(buffer))
                        },
                        Err(err) => Err(err),
                    }
                },
            }
        };

        if let Some((new_id, socket)) = accepted_new_socket {
            self.bindings.insert(
                Binding {
                    local: SocketAddr {
                        host: IpAddr::V4(Ipv4Addr::ZERO), // TODO: if socket has any set, inherit that
                        port: (&socket).user_data().local_port,
                    },
                    remote: Some(socket.remote()),
                },
                new_id,
            );
            self.sockets.insert(new_id, socket.into());
        }

        log::debug!("TCP USER REPLY {:?}", reply);

        let socket = self
            .handler_for(socket_id)
            .expect("Socket has been removed incorrectly");

        match reply {
            Err(tcp::state::Error::RetryAfter(cookie)) => {
                socket
                    .user_data_mut()
                    .events_suspended
                    .insert(cookie, (SuspendMode::Retry(request), reply_ctx));
            },
            Err(tcp::state::Error::ContinueAfter(cookie)) => {
                socket
                    .user_data_mut()
                    .events_suspended
                    .insert(cookie, (SuspendMode::Continue, reply_ctx));
            },
            other => {
                let response: Result<Reply, Error> = other.map_err(|e| e.into());
                reply_ctx
                    .reply(response)
                    .expect("TODO: handle disconnection(?)");
            },
        }

        true
    }

    fn socket_for(&self, mut binding: Binding) -> Option<SocketId> {
        // Prefer exact address match
        if self.bindings.contains_key(&binding) {
            return Some(*self.bindings.get(&binding).unwrap());
        }

        // If no exact match is found, try without a fixed remote
        let remote = binding.remote.take();
        if remote.is_some() && self.bindings.contains_key(&binding) {
            return Some(*self.bindings.get(&binding).unwrap());
        }

        // Then with a fixed remote but any local ip
        binding.local = SocketAddr {
            host: IpAddr::V4(Ipv4Addr::ZERO),
            port: binding.local.port,
        };
        binding.remote = remote;
        if self.bindings.contains_key(&binding) {
            return Some(*self.bindings.get(&binding).unwrap());
        }

        // Then without remote and any local ip
        binding.remote = None;
        self.bindings.get(&binding).copied()
    }

    fn handler_for(&mut self, socket_id: SocketId) -> Option<&mut tcp::state::Socket<SocketData>> {
        self.sockets.get_mut(&socket_id)
    }

    pub fn handle_packet(&mut self, ip_header: ipv4::Header, tcp_segment: tcp::Segment) {
        let seg = tcp::state::SegmentMeta {
            seqn: tcp::state::SeqN::new(tcp_segment.header.sequence),
            ackn: tcp::state::SeqN::new(tcp_segment.header.ack_number),
            window: tcp_segment.header.window_size,
            flags: tcp_segment.header.flags,
            data: tcp_segment.payload,
        };

        let Some(socket_id) = self.socket_for(
            Binding {
                local: SocketAddr { host: IpAddr::V4(ip_header.dst_ip), port: tcp_segment.header.dst_port },
                remote: Some(SocketAddr { host: IpAddr::V4(ip_header.src_ip), port: tcp_segment.header.src_port }),
            }
        )
        else {
            log::warn!("No TCP handlers assigned for {}:{}", ip_header.dst_ip, tcp_segment.header.dst_port);
            log::trace!("Bindings {:?}", self.bindings);
            if let Some(reply) = tcp::state::response_to_closed(seg) {
                // TODO: send reply
            }
            return;
        };

        let handler = self
            .handler_for(socket_id)
            .expect("Socket for SocketId not available");

        log::trace!("Packet to (socket={:?}): {:?}", socket_id, seg);

        handler.on_segment(
            SocketAddr {
                host: IpAddr::V4(ip_header.src_ip),
                port: tcp_segment.header.src_port,
            },
            seg,
        );

        self.process_events(socket_id);
    }

    pub fn process_events(&mut self, socket_id: SocketId) {
        let socket = self
            .handler_for(socket_id)
            .expect("Socket has been removed incorrectly");

        let ev = &mut socket.user_data_mut().events_ready;
        if ev.is_empty() {
            return;
        }
        let events = core::mem::replace(ev, Vec::new());

        let send_error = socket.user_data_mut().send_error.take();

        for (suspend_mode, reply_ctx, result) in events {
            log::trace!("Processing event {:?} {:?}", suspend_mode, result);

            if let Some(error) = send_error {
                reply_ctx
                    .reply(Err(error.clone().into()))
                    .expect("TODO: disconnection");
                continue;
            }

            match suspend_mode {
                SuspendMode::Retry(request) => {
                    if result.is_err() {
                        reply_ctx
                            .reply(match result {
                                Ok(()) => Ok(Reply::NoData),
                                Err(err) => Err(err.into()),
                            })
                            .expect("TODO: disconnection");
                    } else {
                        // There is never need to check_events after retry
                        let _ = self.user_socket_event_inner(socket_id, request, reply_ctx);
                    }
                },
                SuspendMode::Continue => {
                    reply_ctx
                        .reply(match result {
                            Ok(()) => Ok(Reply::NoData),
                            Err(err) => Err(err.into()),
                        })
                        .expect("TODO: disconnection");
                },
            }
        }
    }
}
