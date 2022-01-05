use hashbrown::HashSet;

use d7abi::SyscallErrorCode;

use crate::multitasking::{ExplicitEventId, Scheduler};

/// Marker type to indicate that an even should be triggered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TriggerEvent(pub ExplicitEventId);

#[derive(Debug, Clone)]
#[must_use]
pub struct IpcResult<T> {
    value: Result<T, Error>,
    events: HashSet<TriggerEvent>,
}
impl<T> IpcResult<T> {
    pub fn new(value: Result<T, Error>) -> Self {
        Self {
            value,
            events: HashSet::new(),
        }
    }

    pub fn success(value: T) -> Self {
        Self {
            value: Ok(value),
            events: HashSet::new(),
        }
    }

    pub fn error(error: Error) -> Self {
        Self {
            value: Err(error),
            events: HashSet::new(),
        }
    }

    pub fn with_event(mut self, new_event: TriggerEvent) -> Self {
        self.events.insert(new_event);
        self
    }

    pub fn with_events(mut self, new_events: impl Iterator<Item = TriggerEvent>) -> Self {
        self.events.extend(new_events);
        self
    }

    pub fn separate_events(self) -> (Result<T, Error>, HashSet<TriggerEvent>) {
        (self.value, self.events)
    }

    pub fn consume_events(self, sched: &mut Scheduler) -> Result<T, Error> {
        for event in self.events.into_iter() {
            sched.on_explicit_event(event.0);
        }
        self.value
    }

    pub fn map<F, R>(self, f: F) -> IpcResult<R>
    where F: FnOnce(T) -> R {
        IpcResult {
            value: self.value.map(f),
            events: self.events,
        }
    }
}
impl<T> core::convert::From<Error> for IpcResult<T> {
    fn from(error: Error) -> Self {
        Self::error(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    InvalidTopic,
    Unsubscribed,
    ReAcknowledge,
    PipeReserved,
    PipeSenderTerminated,
    Subscription(SubscriptionError),
    Delivery(DeliveryError),
    Permission(PermissionError),
}
impl core::convert::From<SubscriptionError> for Error {
    fn from(error: SubscriptionError) -> Self {
        Self::Subscription(error)
    }
}
impl core::convert::From<DeliveryError> for Error {
    fn from(error: DeliveryError) -> Self {
        Self::Delivery(error)
    }
}
impl core::convert::From<PermissionError> for Error {
    fn from(error: PermissionError) -> Self {
        Self::Permission(error)
    }
}
impl core::convert::Into<SyscallErrorCode> for Error {
    fn into(self) -> SyscallErrorCode {
        match self {
            Self::InvalidTopic => SyscallErrorCode::ipc_invalid_topic,
            Self::Unsubscribed => SyscallErrorCode::ipc_unsubscribed,
            Self::ReAcknowledge => SyscallErrorCode::ipc_re_acknowledge,
            Self::PipeReserved => SyscallErrorCode::ipc_pipe_reserved,
            Self::PipeSenderTerminated => SyscallErrorCode::ipc_pipe_sender_terminated,
            Self::Subscription(e) => e.into(),
            Self::Delivery(e) => e.into(),
            Self::Permission(e) => e.into(),
        }
    }
}
impl core::convert::Into<u64> for Error {
    fn into(self) -> u64 {
        let s: SyscallErrorCode = self.into();
        s.into()
    }
}

/// Subscribing to a topic filter failed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscriptionError {
    /// Conflicts with exclusive subscriptions
    Exclusion,
}
impl core::convert::Into<SyscallErrorCode> for SubscriptionError {
    fn into(self) -> SyscallErrorCode {
        match self {
            Self::Exclusion => SyscallErrorCode::ipc_filter_exclusion,
        }
    }
}
impl core::convert::Into<u64> for SubscriptionError {
    fn into(self) -> u64 {
        let s: SyscallErrorCode = self.into();
        s.into()
    }
}

/// Reliable delivery failed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliveryError {
    /// Nobody has subscribed to this topic,
    /// or the subcriber desubscribed before processing the message
    NoSubscriber,
    /// Subscribers queue is full
    QueueFull,
    /// Subscriber negative-acknowledged the message
    NegativeAcknowledgement,
}
impl core::convert::Into<SyscallErrorCode> for DeliveryError {
    fn into(self) -> SyscallErrorCode {
        match self {
            Self::NoSubscriber => SyscallErrorCode::ipc_delivery_no_target,
            Self::QueueFull => SyscallErrorCode::ipc_delivery_target_full,
            Self::NegativeAcknowledgement => SyscallErrorCode::ipc_delivery_target_nack,
        }
    }
}
impl core::convert::Into<u64> for DeliveryError {
    fn into(self) -> u64 {
        let s: SyscallErrorCode = self.into();
        s.into()
    }
}

/// Operation not permitted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionError {
    /// Subscription is not owned by this process
    NotOwner,
    /// No permissions to publish/deliver to this topic
    NoAccess,
}
impl core::convert::Into<SyscallErrorCode> for PermissionError {
    fn into(self) -> SyscallErrorCode {
        SyscallErrorCode::ipc_permission_error
    }
}
impl core::convert::Into<u64> for PermissionError {
    fn into(self) -> u64 {
        let s: SyscallErrorCode = self.into();
        s.into()
    }
}
