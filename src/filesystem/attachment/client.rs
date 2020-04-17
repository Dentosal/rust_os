use alloc::collections::VecDeque;

use crate::multitasking::ExplicitEventId;

use super::super::{result::ErrorCode, FileClientId, OpenFile, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientKey {
    pub fc: FileClientId,
    pub suffix: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Client {
    state: ClientState,
}
impl Client {
    pub fn new() -> Self {
        Self {
            state: ClientState::Ready,
        }
    }

    pub fn clone_state(&self) -> ClientState {
        self.state.clone()
    }

    pub fn state(&self) -> &ClientState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut ClientState {
        &mut self.state
    }

    pub fn set_state(&mut self, key: &ClientKey, new_state: ClientState) {
        self.state = new_state;
        log::trace!("New client state = {:?} ({:?})", self.state, key);
    }
}

#[derive(Debug, Clone)]
pub enum ClientState {
    /// Waiting for the client to start a new operation
    Ready,
    /// Waiting for the manager to start processing a read operation
    ReadPending(ExplicitEventId, usize),
    /// Waiting for the manager to respond to a read operation
    ReadInProgress(ExplicitEventId),
    /// Waiting for the client to retrieve results of a read operation
    ReadComplete(VecDeque<u8>),
    /// Waiting for the manager to start processing a write operation
    WritePending(ExplicitEventId, VecDeque<u8>),
    /// Waiting for the manager to respond to a write operation
    WriteInProgress(ExplicitEventId),
    /// Waiting for the client to retrieve results of a write operation
    WriteComplete(usize),
    /// Waiting for the manager to process a close notification
    ClosePending,
    /// File closed TODO: just remove the client when it's closed
    Closed,
    /// Client respoded with an error
    Error(ErrorCode),
}
impl ClientState {
    /// Client wakeup id, if any, for attachment destruction
    pub fn event(&self) -> Option<ExplicitEventId> {
        match self {
            Self::Ready => None,
            Self::ReadPending(event, _) => Some(*event),
            Self::ReadInProgress(event) => Some(*event),
            Self::ReadComplete(_) => None,
            Self::WritePending(event, _) => Some(*event),
            Self::WriteInProgress(event) => Some(*event),
            Self::WriteComplete(_) => None,
            Self::ClosePending => None,
            Self::Closed => None,
            Self::Error(_) => None,
        }
    }
}
