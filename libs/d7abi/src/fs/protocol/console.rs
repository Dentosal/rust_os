use serde::{Deserialize, Serialize};
use alloc::prelude::v1::*;

use d7time::Instant;

pub type KeyCode = u16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KeyboardEvent {
    /// Release or press
    pub release: bool,
    /// Keycode, i.e. index
    pub keycode: KeyCode,
    /// Timestamp
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextUpdate {
    pub line: String,
    pub newline: bool,
}
