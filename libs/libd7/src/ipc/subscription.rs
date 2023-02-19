use core::marker::PhantomData;

use alloc::string::String;

use serde::de::DeserializeOwned;

use d7abi::ipc::*;

use super::InternalSubscription;

use crate::syscall::{self, SyscallResult};

/// TODO: Implement paged ipc buffers, and reduce this to max inlined size
/// Use huge buffer for now.
const BUFFER_SIZE: usize = 0x10_0000;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnreliableSubscription<T: DeserializeOwned> {
    id: SubscriptionId,
    msg_type: PhantomData<T>,
}
impl<T: DeserializeOwned> UnreliableSubscription<T> {
    pub fn exact(filter: &str) -> SyscallResult<Self> {
        Ok(Self {
            id: syscall::ipc_subscribe(filter, SubscriptionFlags::empty())?,
            msg_type: PhantomData,
        })
    }

    pub fn prefix(filter: &str) -> SyscallResult<Self> {
        Ok(Self {
            id: syscall::ipc_subscribe(filter, SubscriptionFlags::PREFIX)?,
            msg_type: PhantomData,
        })
    }

    /// Receive, data only
    pub fn receive(&self) -> SyscallResult<T> {
        Ok(self.receive_topic()?.0)
    }

    /// Receive, including topic name
    pub fn receive_topic(&self) -> SyscallResult<(T, String)> {
        let mut buffer = [0u8; BUFFER_SIZE];
        let count = syscall::ipc_receive(self.id, &mut buffer)?;
        let msg: Message = pinecone::from_bytes(&buffer[..count]).expect("Invalid message");
        let data: T = pinecone::from_bytes(&msg.data).expect("Invalid message");
        Ok((data, msg.topic))
    }
}
impl<T: DeserializeOwned> InternalSubscription for UnreliableSubscription<T> {
    fn sub_id(&self) -> SubscriptionId {
        self.id
    }
}
impl<T: DeserializeOwned> Drop for UnreliableSubscription<T> {
    /// Unsubscribe on drop
    fn drop(&mut self) {
        syscall::ipc_unsubscribe(self.id).expect("Unsubscribe on drop failed");
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReliableSubscription<T: DeserializeOwned> {
    id: SubscriptionId,
    msg_type: PhantomData<T>,
}
impl<T: DeserializeOwned> ReliableSubscription<T> {
    pub fn exact(filter: &str) -> SyscallResult<Self> {
        Ok(Self {
            id: syscall::ipc_subscribe(filter, SubscriptionFlags::RELIABLE)?,
            msg_type: PhantomData,
        })
    }

    pub fn prefix(filter: &str) -> SyscallResult<Self> {
        Ok(Self {
            id: syscall::ipc_subscribe(
                filter,
                SubscriptionFlags::RELIABLE | SubscriptionFlags::PREFIX,
            )?,
            msg_type: PhantomData,
        })
    }

    pub fn pipe(filter: &str) -> SyscallResult<Self> {
        Ok(Self {
            id: syscall::ipc_subscribe(filter, SubscriptionFlags::PIPE)?,
            msg_type: PhantomData,
        })
    }

    /// Receive, data only
    pub fn receive(&self) -> SyscallResult<(AcknowledgeContext, T)> {
        let (ack_ctx, data, _topic) = self.receive_topic()?;
        Ok((ack_ctx, data))
    }

    /// Receive, including topic name
    pub fn receive_topic(&self) -> SyscallResult<(AcknowledgeContext, T, String)> {
        let mut buffer = [0u8; BUFFER_SIZE];
        let count = syscall::ipc_receive(self.id, &mut buffer)?;
        let msg: Message = pinecone::from_bytes(&buffer[..count]).expect("Invalid message");
        let ack_ctx = AcknowledgeContext {
            sub_id: self.id,
            ack_id: msg.ack_id,
        };
        let data: T = pinecone::from_bytes(&msg.data).expect("Invalid message payload");
        Ok((ack_ctx, data, msg.topic))
    }

    /// Receive and acknowledge, data only
    pub fn ack_receive(&self) -> SyscallResult<T> {
        let (data, _topic) = self.ack_receive_topic()?;
        Ok(data)
    }

    /// Receive and acknowledge, including topic name
    pub fn ack_receive_topic(&self) -> SyscallResult<(T, String)> {
        let (ctx, data, topic) = self.receive_topic()?;
        ctx.ack()?;
        Ok((data, topic))
    }
}
impl<T: DeserializeOwned> InternalSubscription for ReliableSubscription<T> {
    fn sub_id(&self) -> SubscriptionId {
        self.id
    }
}
impl<T: DeserializeOwned> Drop for ReliableSubscription<T> {
    /// Unsubscribe on drop
    fn drop(&mut self) {
        syscall::ipc_unsubscribe(self.id).expect("Unsubscribe on drop failed");
    }
}

/// Automatically NACKs if not used
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[must_use]
pub struct AcknowledgeContext {
    sub_id: SubscriptionId,
    /// None can be used here to signal that no acknowledgement is required.
    /// This is used for two purposes:
    /// * Drop handler checks if this is Some to auto-nack
    /// * Kernel responses do not require acknowledgements
    ack_id: Option<AcknowledgeId>,
}
impl AcknowledgeContext {
    /// Positive acknowledge
    pub fn ack(mut self) -> SyscallResult<()> {
        if let Some(ack_id) = self.ack_id.take() {
            syscall::ipc_acknowledge(self.sub_id, ack_id, true)
        } else {
            Ok(())
        }
    }

    /// Negetive acknowledge
    pub fn nack(mut self) -> SyscallResult<()> {
        if let Some(ack_id) = self.ack_id.take() {
            syscall::ipc_acknowledge(self.sub_id, ack_id, false)
        } else {
            Ok(())
        }
    }
}

impl Drop for AcknowledgeContext {
    /// Automatically negetive-acknowledge transactions on drop
    /// if not manually acknowledged
    fn drop(&mut self) {
        if let Some(ack_id) = self.ack_id {
            log::debug!("Dropped AckCtx NACK");
            syscall::ipc_acknowledge(self.sub_id, ack_id, false)
                .expect("Failed to negative-acknowledge in drop");
        }
    }
}
