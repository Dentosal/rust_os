use alloc::prelude::v1::*;
use hashbrown::HashMap;

use d7abi::fs::protocol::attachment::*;
use d7abi::fs::FileDescriptor;

use crate::multitasking::{EventQueue, ExplicitEventId, ProcessId, QueueLimit, WaitFor};

use super::{
    result::{IoResult, IoResultPure},
    CloseAction, FileClientId, FileOps, Path, PathBuf, Trigger,
};

mod client;

use client::*;

/// # Attachment point
/// This node and its contents are managed by a driver
/// software. On branch nodes, the driver can provide
/// child nodes that are used. The nodes themselves,
/// however, must exist before a read is attempted.
/// ## Nesting attach points
/// Nested mounts are allowed.
/// The innermost mount point will receive all operations.
/// ## Unattaching
/// Unlike Linux, where unmounting requires that all inner
/// mounts are unmounted first, this implementation simply
/// allows unattaching the point, causing all inner attachments
/// to be closed as well.
#[derive(Debug)]
pub struct Attachment {
    /// Process and file descriptor managing this attachment
    manager: FileClientId,
    /// Leafness is a static property of an attachment,
    /// the controlling process cannot change this
    is_leaf: bool,

    /// All clients, key is (fc_id, suffix)
    clients: HashMap<ClientKey, Client>,

    /// Manager queue for pending client events
    manager_pending_events: EventQueue<ClientKey>,
}
impl Attachment {
    pub fn new(manager: FileClientId, is_leaf: bool) -> Self {
        Self {
            manager,
            is_leaf,
            clients: HashMap::new(),
            manager_pending_events: EventQueue::new("Attachment", QueueLimit::None),
        }
    }

    pub fn pid(&self) -> IoResultPure<ProcessId> {
        IoResultPure::Success(self.manager.process.unwrap())
    }

    pub fn open(&mut self, suffix: Option<PathBuf>, fc: FileClientId) -> IoResult<()> {
        let key = ClientKey { fc, suffix };
        log::trace!("Open {:?}", key);
        self.clients.insert(key, Client::new());
        // TODO: ask about open from the manager
        IoResult::success(())
    }

    pub fn read(
        &mut self, suffix: Option<PathBuf>, fc: FileClientId, buf: &mut [u8],
    ) -> IoResult<usize> {
        if fc == self.manager {
            assert!(suffix.is_none());

            let key = self
                .manager_pending_events
                .pop_event()
                .expect("Manager event queue should not be empty");

            let client = self
                .clients
                .get_mut(&key)
                .expect("Attachment: write without open");

            return match client.clone_state() {
                ClientState::ReadPending(event, count) => {
                    let req = Request {
                        sender: Sender {
                            pid: key.fc.process,
                            f: key.fc.fd.as_u64(),
                        },
                        suffix: key.suffix.clone().map(|s| s.to_string()),
                        operation: RequestFileOperation::Read(count as u64),
                    };

                    let bytes = pinecone::to_vec(&req).unwrap();
                    if bytes.len() <= buf.len() {
                        buf[..bytes.len()].copy_from_slice(&bytes);
                    } else {
                        // TODO: Process error, not kernel panic
                        panic!("Target buffer not large enough");
                    }
                    // Mark the read to be in progress
                    client.set_state(&key, ClientState::ReadInProgress(event));

                    log::trace!("PUSH EVENT {:?}", (key.fc, suffix.clone())); // XXX

                    self.manager_pending_events
                        .push_io(key)
                        .map(|()| bytes.len())
                },
                ClientState::WritePending(event, data) => {
                    let req = Request {
                        sender: Sender {
                            pid: key.fc.process,
                            f: key.fc.fd.as_u64(),
                        },
                        suffix: key.suffix.clone().map(|s| s.to_string()),
                        operation: RequestFileOperation::Write(data.iter().copied().collect()),
                    };

                    let bytes = pinecone::to_vec(&req).expect("Could not serialize");
                    if bytes.len() <= buf.len() {
                        buf[..bytes.len()].copy_from_slice(&bytes);
                    } else {
                        // TODO: Process error, not kernel panic
                        panic!("Target buffer not large enough");
                    }
                    // Mark the write to be in progress
                    client.set_state(&key, ClientState::WriteInProgress(event));
                    IoResult::success(bytes.len())
                },
                ClientState::ClosePending => {
                    let req = Request {
                        sender: Sender {
                            pid: key.fc.process,
                            f: key.fc.fd.as_u64(),
                        },
                        suffix: key.clone().suffix.map(|s| s.to_string()),
                        operation: RequestFileOperation::Close,
                    };

                    let bytes = pinecone::to_vec(&req).expect("Could not serialize");
                    if bytes.len() <= buf.len() {
                        buf[..bytes.len()].copy_from_slice(&bytes);
                    } else {
                        // TODO: Process error, not kernel panic
                        panic!("Target buffer not large enough");
                    }
                    // Mark close to be complete
                    client.set_state(&key, ClientState::Closed);
                    IoResult::success(bytes.len())
                },
                other => unreachable!(
                    "Invalid client state ({:?}) (fc={:?} suffix={:?}) on manager read",
                    other, key.fc, key.suffix
                ),
            };
        }

        let key = ClientKey {
            fc,
            suffix: suffix.clone(),
        };
        if let Some(client) = self.clients.get_mut(&key.clone()) {
            match client.state_mut() {
                ClientState::Ready => {
                    // New read operation
                    let event_id = WaitFor::new_event_id();
                    client.set_state(&key, ClientState::ReadPending(event_id, buf.len()));

                    self.manager_pending_events.push(key);

                    IoResult::repeat_after(WaitFor::Event(event_id))
                },
                ClientState::ReadComplete(data) => {
                    let mut i = 0;
                    while i < buf.len() {
                        if let Some(byte) = data.pop_front() {
                            buf[i] = byte;
                            i += 1;
                        } else {
                            break;
                        }
                    }

                    if data.is_empty() {
                        client.set_state(&key, ClientState::Ready);
                    }

                    // Return
                    IoResult::success(i)
                },
                ClientState::WriteComplete(count) => {
                    let result = IoResult::success(*count);
                    client.set_state(&key, ClientState::Ready);
                    result
                },
                ClientState::Error(error_code) => {
                    let result = IoResult::error(*error_code);
                    client.set_state(&key, ClientState::Ready);
                    result
                },
                other => unreachable!("Invalid client state ({:?}) on read", other),
            }
        } else {
            panic!("Attachment: read without open");
        }
    }

    pub fn read_waiting_for(&mut self, suffix: Option<PathBuf>, fc: FileClientId) -> WaitFor {
        if fc == self.manager {
            assert!(suffix.is_none());
            return self.manager_pending_events.wait_for();
        }

        let key = ClientKey { fc, suffix };
        if let Some(client) = self.clients.get(&key) {
            client
                .state()
                .event()
                .map(|event| WaitFor::Event(event))
                .unwrap_or(WaitFor::None)
        } else {
            WaitFor::None
        }
    }

    pub fn write(
        &mut self, suffix: Option<PathBuf>, fc: FileClientId, buf: &[u8],
    ) -> IoResult<usize> {
        if fc == self.manager {
            assert!(suffix.is_none());

            // Manager writes response to a request. The whole response must be written at once.
            let (response, rest): (Response, &[u8]) =
                pinecone::take_from_bytes(buf).expect("Partial write from manager");

            let client_fc = FileClientId {
                process: response.sender.pid,
                fd: FileDescriptor::from_u64(response.sender.f),
            };

            let key = ClientKey {
                fc: client_fc,
                suffix: response.suffix.map(|s| Path::new(&s).to_owned()),
            };

            let client = self.clients.get_mut(&key).expect("No such client");

            let client_wakeup_event = match response.operation {
                ResponseFileOperation::Read(data) => match client.clone_state() {
                    ClientState::ReadInProgress(event_id) => {
                        client.set_state(&key, ClientState::ReadComplete(data.into()));
                        event_id
                    },
                    other => panic!(
                        "Client is incorrect state {:?} on manager read complete",
                        other
                    ),
                },
                ResponseFileOperation::Write(count) => match client.clone_state() {
                    ClientState::WriteInProgress(event_id) => {
                        client.set_state(&key, ClientState::WriteComplete(count as usize));
                        event_id
                    },
                    other => panic!(
                        "Client is incorrect state {:?} on manager write complete",
                        other
                    ),
                },
                ResponseFileOperation::Error(error_code) => match client.clone_state() {
                    ClientState::ReadInProgress(event_id) => {
                        client.set_state(&key, ClientState::Error(error_code));
                        event_id
                    },
                    ClientState::WriteInProgress(event_id) => {
                        client.set_state(&key, ClientState::Error(error_code));
                        event_id
                    },
                    other => panic!(
                        "Client is incorrect state {:?} on manager returning error",
                        other
                    ),
                },
            };

            IoResult::success(buf.len() - rest.len()).with_event(client_wakeup_event)
        } else {
            // Writes to attachments must be of type `d7abi::fs::protocol::attachment::Request`,
            // and the whole request must be written at once
            let key = ClientKey { fc, suffix };

            let client = self
                .clients
                .get_mut(&key)
                .expect("Attachment: write without open");

            match client.clone_state() {
                ClientState::Ready => {
                    // Add to queue, and wait until manager processes the write request
                    let event_id = WaitFor::new_event_id();
                    client.set_state(
                        &key,
                        ClientState::WritePending(event_id, buf.into_iter().copied().collect()),
                    );

                    log::trace!("Creating new write operation + wait {:?}", event_id);

                    let (result, ctx) = self.manager_pending_events.push_io(key).separate_events();

                    assert!(result.is_success());
                    IoResult::repeat_after(WaitFor::Event(event_id)).with_context(ctx)
                },
                ClientState::WriteComplete(count) => {
                    client.set_state(&key, ClientState::Ready);
                    IoResult::success(count)
                },
                ClientState::Error(error_code) => IoResult::error(error_code),
                other => panic!(
                    "Client is incorrect state {:?} on client write (key={:?})",
                    other, key
                ),
            }
        }
    }

    /// When manager closes the file, destroy this, triggering all waiting processes
    /// When client closes the file, send close message
    pub fn close(&mut self, suffix: Option<PathBuf>, fc: FileClientId) -> IoResult<CloseAction> {
        let key = ClientKey { fc, suffix };
        if fc == self.manager {
            return IoResult::success(CloseAction::Destroy);
        } else if let Some(client) = self.clients.get_mut(&key.clone()) {
            // Remove all ongoing reads for this client
            client.set_state(&key, ClientState::ClosePending);
            log::trace!("PUSH EVENT {:?}", key.clone()); // XXX
            self.manager_pending_events
                .push_io(key)
                .map(|()| CloseAction::Normal)
        } else {
            IoResult::success(CloseAction::Normal)
        }
    }

    /// Trigger all waiting processes
    pub fn destroy(&mut self) -> Trigger {
        Trigger::events(
            self.clients
                .values()
                .filter_map(|client| client.state().event())
                .collect(),
        )
    }
}
