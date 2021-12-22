use serde::{Deserialize, Serialize};

pub type KeyCode = u16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KeyboardEvent {
    /// Release or press
    pub release: bool,
    /// Keycode, i.e. index
    pub keycode: KeyCode,
    // TODO: Timestamp
    // pub timestamp: SystemTime,
}
