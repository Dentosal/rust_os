use super::key::Key;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyboardEventType {
    Press,
    Release,
    Repeat, // ?
}

#[derive(Debug, Clone, Copy)]
pub struct KeyboardEvent {
    pub key: Key,
    pub event_type: KeyboardEventType, // modifiers
}
impl KeyboardEvent {
    pub fn new(key: Key, event_type: KeyboardEventType) -> KeyboardEvent {
        KeyboardEvent { key, event_type }
    }
}
