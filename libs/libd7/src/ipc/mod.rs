pub use d7abi::ipc::*;

mod select;
mod send;
mod server;
mod subscription;

pub use self::send::*;
pub use self::server::*;
pub use self::subscription::*;

/// Any items that contain internal subscriptions must implement this,
/// so that select! can use them
pub trait InternalSubscription {
    fn sub_id(&self) -> SubscriptionId;
}

impl InternalSubscription for SubscriptionId {
    fn sub_id(&self) -> SubscriptionId {
        *self
    }
}
