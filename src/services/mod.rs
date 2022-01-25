//! IPC-accessible services hosted from the kernel for technical reasons.
//! Only reliable connections accepted.

use alloc::vec::Vec;
use hashbrown::HashMap;
use serde::Serialize;
use spin::Mutex;

use d7abi::process::ProcessId;

use crate::ipc::{
    AcknowledgeId, DeliveryError, IpcResult, Manager, Message, SubscriptionId, TopicFilter, IPC,
};

mod initrd;

pub fn init() {
    register_exact("initrd/read", initrd::read);
}

fn register(filter: TopicFilter, service: Service) {
    let mut ipc_manager = IPC.try_lock().unwrap();
    let sub = ipc_manager
        .kernel_subscribe(filter)
        .expect("Could not register a service");
    let mut services = SERVICES.try_lock().unwrap();
    services.insert(sub, service);
}

fn register_exact(filter: &str, service: Service) {
    register(
        TopicFilter::try_new(filter, true).expect("Invalid filter"),
        service,
    )
}

fn register_prefix(filter: &str, service: Service) {
    register(
        TopicFilter::try_new(filter, false).expect("Invalid filter"),
        service,
    )
}

/// For now, scheduler and ipc are unavailable for services
type Service = fn(&mut Manager, ProcessId, Message) -> Result<(), DeliveryError>;

lazy_static::lazy_static! {
    static ref SERVICES: Mutex<HashMap<SubscriptionId, Service>> = Mutex::new(HashMap::new());
}

/// Return value used as the deliver/acknowledgement result
pub fn incoming(
    manager: &mut Manager, pid: ProcessId, sub: SubscriptionId, mut message: Message,
) -> IpcResult<()> {
    let mut services = SERVICES.try_lock().unwrap();
    let service = services
        .get_mut(&sub)
        .expect("No such subscription for the kernel services");

    let _ack_id = message
        .ack_id
        .take()
        .expect("Incoming messages must be reliable");

    IpcResult::new(service(manager, pid, message).map_err(|e| e.into()))
}
