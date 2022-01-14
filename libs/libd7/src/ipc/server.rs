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

use alloc::string::String;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU64, Ordering};
use serde::{de::DeserializeOwned, Serialize};

use d7abi::ipc::*;

use crate::syscall::SyscallResult;

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

    pub fn pipe(filter: &str) -> SyscallResult<Self> {
        Ok(Self::new(ReliableSubscription::pipe(filter)?))
    }

    /// Handle one request
    pub fn handle<F>(&self, f: F) -> SyscallResult<()>
    where F: FnOnce(RQ) -> SyscallResult<RS> {
        self.handle_topic(|message, _topic| f(message))
    }

    /// Handle one request, including topic name
    pub fn handle_topic<F>(&self, f: F) -> SyscallResult<()>
    where F: FnOnce(RQ, String) -> SyscallResult<RS> {
        let (ack_ctx, request, topic): (_, Request<RQ>, _) = self.sub.receive_topic()?;
        let (reply_to, message): (String, RQ) = request;
        let response: RS = f(message, topic)?;
        deliver_reply(&reply_to, &response)?;
        ack_ctx.ack()?;
        Ok(())
    }

    /// Handle one request
    /// This can be used to delay the response
    pub fn receive(&self) -> SyscallResult<(ReplyCtx<RS>, RQ)> {
        let (reply_ctx, value, _topic) = self.receive_topic()?;
        Ok((reply_ctx, value))
    }

    /// Handle one request, including topic name
    /// This can be used to delay the response
    pub fn receive_topic(&self) -> SyscallResult<(ReplyCtx<RS>, RQ, String)> {
        let (ack_ctx, request, topic): (_, Request<RQ>, _) = self.sub.receive_topic()?;
        let (reply_topic, message): (String, RQ) = request;
        Ok((ReplyCtx::new(reply_topic, ack_ctx), message, topic))
    }
}

impl<RQ: Serialize + DeserializeOwned, RS: Serialize + DeserializeOwned> InternalSubscription
    for Server<RQ, RS>
{
    fn sub_id(&self) -> SubscriptionId {
        self.sub.sub_id()
    }
}

/// Client of the server is suspended while the ReplyCtx exists
pub struct ReplyCtx<RS: Serialize + DeserializeOwned> {
    reply_topic: String,
    ack_ctx: AcknowledgeContext,
    response_type: PhantomData<RS>,
}
impl<RS: Serialize + DeserializeOwned> ReplyCtx<RS> {
    fn new(reply_topic: String, ack_ctx: AcknowledgeContext) -> Self {
        Self {
            reply_topic,
            ack_ctx,
            response_type: PhantomData,
        }
    }
}
impl<RS: Serialize + DeserializeOwned> ReplyCtx<RS> {
    /// Consumes this context to send a reply
    pub fn reply(self, data: RS) -> SyscallResult<()> {
        deliver_reply(&self.reply_topic, &data)?;
        self.ack_ctx.ack()
    }

    /// Consumes this context to send a nack
    pub fn nack(self) -> SyscallResult<()> {
        self.ack_ctx.nack()
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
