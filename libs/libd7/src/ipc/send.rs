use serde::Serialize;

use crate::syscall::{self, SyscallResult};

/// Send an unreliable (fire-and-forget) message to a topic
pub fn publish<T: Serialize>(topic: &str, message: &T) -> SyscallResult<()> {
    let data = pinecone::to_vec(message).unwrap();
    syscall::ipc_publish(topic, &data)
}

/// Send a reliable message to a topic, and wait until receiver acknowledges it
pub fn deliver<T: Serialize>(topic: &str, message: &T) -> SyscallResult<()> {
    let data = pinecone::to_vec(message).unwrap();
    syscall::ipc_deliver(topic, &data)
}

/// Send a reliable message to a topic, but don't require acknowledgement
pub fn deliver_reply<T: Serialize>(topic: &str, message: &T) -> SyscallResult<()> {
    let data = pinecone::to_vec(message).unwrap();
    syscall::ipc_deliver_reply(topic, &data)
}
