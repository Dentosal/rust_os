use super::key::Key;
use super::keymap::Keymap;

use alloc::Vec;


const BUFFER_SIZE: usize = 10;

pub struct KeyReader {
    buffer: Option<Vec<u8>>,
    keymap: Keymap
}
impl KeyReader {
    pub const fn new() -> KeyReader {
        KeyReader {
            buffer: None,
            keymap: Keymap::new()
        }
    }

    pub fn init(&mut self) {
        self.buffer = Some(Vec::new());
        self.keymap.init();
    }

    /// Insert a byte into reader
    /// Returns a key if complete, else insert more
    pub fn insert(&mut self, b: u8) -> Option<Key> {
        match self.buffer {
            Some(ref mut buf) => {
                buf.push(b);
                let key = self.keymap.get(buf.clone());
                if key.is_some() {
                    buf.clear();
                }
                else {
                    // XXX
                    buf.clear();
                }
                key
            },
            None => {
                None
            }
        }
    }
}
