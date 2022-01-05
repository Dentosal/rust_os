use serde::{Deserialize, Serialize};

use crate::process::{ProcessId, ProcessResult};

pub mod keyboard;
pub mod service;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTerminated {
    pub pid: ProcessId,
    pub result: ProcessResult,
}
