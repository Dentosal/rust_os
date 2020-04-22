//! Utility functions for interacting with serviced

use alloc::prelude::v1::*;
use hashbrown::HashSet;

use crate::ipc::protocol::service::{Registration, ServiceName};

pub fn register(name: &str, oneshot: bool) {
    crate::ipc::deliver(
        "serviced/register",
        &Registration {
            name: ServiceName(name.to_owned()),
            oneshot,
        },
    )
    .unwrap();
}

pub fn wait_for_one(name: &str) {
    let mut hs = HashSet::new();
    hs.insert(ServiceName(name.to_owned()));
    crate::ipc::deliver("serviced/waitfor/any", &hs).unwrap();
}
