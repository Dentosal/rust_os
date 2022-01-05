//! Utility functions for interacting with serviced

use alloc::borrow::ToOwned;
use hashbrown::HashSet;

use crate::ipc::protocol::service::{Registration, ServiceName};

pub fn register(name: &str, oneshot: bool) {
    crate::ipc::deliver("serviced/register", &Registration {
        name: ServiceName(name.to_owned()),
        oneshot,
    })
    .unwrap();
}

pub fn wait_for_one(name: &str) {
    let mut hs = HashSet::new();
    hs.insert(ServiceName(name.to_owned()));
    crate::ipc::deliver("serviced/waitfor/any", &hs).unwrap();
}

pub fn wait_for_any<T>(names: T)
where
    T: IntoIterator,
    T::Item: AsRef<str>,
{
    let mut hs = HashSet::new();
    for name in names.into_iter() {
        hs.insert(ServiceName(name.as_ref().to_owned()));
    }
    crate::ipc::deliver("serviced/waitfor/any", &hs).unwrap();
}

pub fn wait_for_all<T>(names: T)
where
    T: IntoIterator,
    T::Item: AsRef<str>,
{
    let mut hs = HashSet::new();
    for name in names.into_iter() {
        hs.insert(ServiceName(name.as_ref().to_owned()));
    }
    crate::ipc::deliver("serviced/waitfor/all", &hs).unwrap();
}
