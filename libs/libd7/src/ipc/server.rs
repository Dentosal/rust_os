//! # Connectionless client-server messaging channels over reliable IPC
//!
//! ## Example: Client (C) wishes to send a message to Server (S).
//!
//! S subscribes to known exact topic (A_LISTEN) (reliable).
//! C subscribes to a random topic (C_RANDOM).
//! C sends message `Request { reply_to: "C_RANDOM", data: "$DATA" }` to A_LISTEN.
//! S receives the message, replies to C_RANDOM with deliver_reply
//! S acknowledges the message.
//! C wakes up and receives the reply from C_RANDOM.
//!
//! If there is no known topic for S, a discovery service could be used.
//! (Discovery service always has a known topic)

use alloc::prelude::v1::*;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU64, Ordering};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use d7abi::ipc::*;

use crate::syscall::{self, SyscallResult};

use super::*;

type Request<T> = (String, T);

pub struct Server<RQ: Serialize + DeserializeOwned, RS: Serialize + DeserializeOwned> {
    sub: ReliableSubscription<Request<RQ>>,
    response_type: PhantomData<RS>,
}
impl<RQ: Serialize + DeserializeOwned, RS: Serialize + DeserializeOwned> Server<RQ, RS> {
    pub fn new(sub: ReliableSubscription<Request<RQ>>) -> Self {
        Self {
            sub,
            response_type: PhantomData,
        }
    }

    pub fn exact(filter: &str) -> SyscallResult<Self> {
        Ok(Self::new(ReliableSubscription::exact(filter)?))
    }

    pub fn prefix(filter: &str) -> SyscallResult<Self> {
        Ok(Self::new(ReliableSubscription::prefix(filter)?))
    }

    /// Handle one request
    pub fn handle<F>(&self, f: F) -> SyscallResult<()>
    where
        F: FnOnce(RQ) -> SyscallResult<RS>,
    {
        self.handle_topic(|message, _topic| f(message))
    }

    /// Handle one request, including topic name
    pub fn handle_topic<F>(&self, f: F) -> SyscallResult<()>
    where
        F: FnOnce(RQ, String) -> SyscallResult<RS>,
    {
        let (ack_ctx, request, topic): (_, Request<RQ>, _) = self.sub.receive_topic()?;
        let (reply_to, message): (String, RQ) = request;
        let response: RS = f(message, topic)?;
        deliver_reply(&reply_to, &response)?;
        ack_ctx.ack()?;
        Ok(())
    }
}

impl<RQ: Serialize + DeserializeOwned, RS: Serialize + DeserializeOwned> InternalSubscription
    for Server<RQ, RS>
{
    fn sub_id(&self) -> SubscriptionId {
        self.sub.sub_id()
    }
}

static NEXT_TOPIC: AtomicU64 = AtomicU64::new(0);

/// Request to a `Server`, blocks until reply is received and then returns it
pub fn request<RQ: Serialize, RS: DeserializeOwned>(topic: &str, message: RQ) -> SyscallResult<RS> {
    use d7abi::process::ProcessId;
    lazy_static::lazy_static! {
        static ref PID: ProcessId = crate::syscall::get_pid();
    }

    // TODO: just use a random number to improve performance
    let reply_topic_num = NEXT_TOPIC.fetch_add(1, Ordering::SeqCst);
    let reply_to = format!("libd7/ipc/request/{}/{}", *PID, reply_topic_num);

    let subscription = ReliableSubscription::exact(&reply_to)?;
    deliver(topic, &(reply_to, message))?;
    let (ack_ctx, data) = subscription.receive()?;
    ack_ctx.ack()?;
    Ok(data)
}
