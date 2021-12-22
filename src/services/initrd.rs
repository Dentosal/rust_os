use alloc::string::String;
use d7abi::process::ProcessId;

use crate::ipc::{DeliveryError, IpcResult, Manager, Message, Topic};

pub fn read(manager: &mut Manager, pid: ProcessId, message: Message) -> Result<(), DeliveryError> {
    let (reply_to, path): (String, String) = pinecone::from_bytes(&message.data)
        .expect("Invalid message: TODO: just reply client error");

    let reply_to = Topic::new(&reply_to).ok_or_else(|| {
        log::warn!("Invalid reply_to topic name from {:?}", pid);
        DeliveryError::NegativeAcknowledgement
    })?;

    let data = crate::initrd::read(&path).ok_or_else(|| {
        log::warn!("Missing initrd {} file requested by {:?}", path, pid);
        DeliveryError::NegativeAcknowledgement
    })?;

    manager.kernel_deliver_reply(reply_to, data)
}
