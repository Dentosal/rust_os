use alloc::prelude::v1::*;
use hashbrown::HashSet;

use d7abi::fs::protocol::console::*;
use d7keymap::{KeyAction, KeyCodes, KeyMap, KeySymbol};

use crate::fs::{self, File};
use crate::syscall::SyscallResult;

pub struct Console {
    file: File,
    keycodes: KeyCodes,
    keymap: KeyMap,
    pressed_modifiers: HashSet<KeySymbol>,
}
impl Console {
    pub fn open(path: &str, keycodes_path: &str, keymap_path: &str) -> SyscallResult<Self> {
        Ok(Self {
            file: File::open(path)?,
            keycodes: serde_json::from_slice(&fs::read(keycodes_path)?)
                .expect("Invalid keycodes.json"),
            keymap: serde_json::from_slice(&fs::read(keymap_path)?).expect("Invalid keymap.json"),
            pressed_modifiers: HashSet::new(),
        })
    }

    fn process_event(&mut self, event: KeyboardEvent) -> EventAction {
        if let Some(keysym) = self.keycodes.clone().get(&event.keycode) {
            if self.keymap.modifiers.contains(&keysym) {
                if event.release {
                    self.pressed_modifiers.remove(keysym);
                } else {
                    self.pressed_modifiers.insert(keysym.clone());
                }
                EventAction::Ignore
            } else if event.release {
                let result = self.process_keysym_press(&keysym);
                if let Some(action) = result {
                    EventAction::KeyAction(action)
                } else {
                    EventAction::Unmatched(keysym.clone())
                }
            } else {
                EventAction::Ignore
            }
        } else {
            EventAction::NoSuchSymbol
        }
    }

    fn process_keysym_press(&mut self, keysym: &KeySymbol) -> Option<KeyAction> {
        for (k, v) in &self.keymap.mapping {
            if k.matches(keysym, &self.pressed_modifiers) {
                return Some(if let KeyAction::Remap(to) = v.clone() {
                    self.process_keysym_press(&to)?
                } else {
                    v.clone()
                });
            }
        }

        None
    }

    pub fn read_line(&mut self) -> SyscallResult<String> {
        use unicode_normalization::UnicodeNormalization;
        use unicode_segmentation::UnicodeSegmentation;

        let mut event_buffer = [0u8; 32];
        let mut result = String::new();
        let mut buffer = String::new();

        loop {
            self.file.read(&mut event_buffer)?;
            let event: KeyboardEvent =
                pinecone::from_bytes(&event_buffer).expect("Invalid key event");

            match self.process_event(event) {
                EventAction::KeyAction(action) => match action {
                    KeyAction::Text(text) => {
                        if !buffer.is_empty() {
                            result.extend(buffer.drain(..));
                        }
                        result.push_str(&text);
                        result = result.nfc().collect();
                    }
                    KeyAction::Buffer(text) => {
                        buffer.push_str(&text);
                    }
                    KeyAction::Remap(_) => unreachable!(),
                    KeyAction::Ignore => {}
                },
                EventAction::Unmatched(symbol) => match symbol.as_str() {
                    "Enter" => {
                        break;
                    }
                    "Backspace" => {
                        buffer.clear();
                        let mut c: Vec<_> =
                            UnicodeSegmentation::graphemes(result.as_str(), true).collect();
                        c.pop();
                        result = c.join("");
                    }
                    _ => {}
                },
                EventAction::Ignore | EventAction::NoSuchSymbol => {}
            }

            let msg: Vec<_> = pinecone::to_vec(&TextUpdate {
                line: result.clone(),
                newline: false,
            }).unwrap();
            let x = self.file.write(&msg)?;
            assert!(x == msg.len()); // TODO: write_all
        }

        let msg: Vec<_> = pinecone::to_vec(&TextUpdate {
            line: result.clone(),
            newline: true,
        }).unwrap();
        let x = self.file.write(&msg)?;
        assert!(x == msg.len()); // TODO: write_all

        Ok(result)
    }
}

#[must_use]
enum EventAction {
    KeyAction(KeyAction),
    Unmatched(KeySymbol),
    Ignore,
    NoSuchSymbol,
}
