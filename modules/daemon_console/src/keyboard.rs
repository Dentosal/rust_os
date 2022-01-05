use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::HashSet;

use d7keymap::{KeyAction, KeyCodes, KeyMap, KeySymbol};
use libd7::ipc::{self, protocol::keyboard::KeyboardEvent};

pub struct Keyboard {
    keycodes: KeyCodes,
    keymap: KeyMap,
    pub pressed_modifiers: HashSet<KeySymbol>,
}
impl Keyboard {
    pub fn new() -> Self {
        let keycodes_json: Vec<u8> =
            ipc::request("initrd/read", "keycodes.json".to_owned()).unwrap();
        let keymap_json: Vec<u8> = ipc::request("initrd/read", "keymap.json".to_owned()).unwrap();

        Self {
            keycodes: serde_json::from_slice(&keycodes_json).unwrap(),
            keymap: serde_json::from_slice(&keymap_json).unwrap(),
            pressed_modifiers: HashSet::new(),
        }
    }

    pub fn process_event(&mut self, event: KeyboardEvent) -> EventAction {
        if let Some(keysym) = self.keycodes.clone().get(&event.keycode) {
            if self.keymap.modifiers.contains(&keysym) {
                if event.release {
                    self.pressed_modifiers.remove(keysym);
                } else {
                    self.pressed_modifiers.insert(keysym.clone());
                }
                EventAction::Ignore
            } else if !event.release {
                let result = self.process_keysym_press(&keysym);
                if let Some(action) = result {
                    EventAction::KeyAction(action)
                } else {
                    EventAction::Unmatched(keysym.clone(), self.pressed_modifiers.clone())
                }
            } else {
                EventAction::Ignore
            }
        } else {
            EventAction::NoSuchSymbol
        }
    }

    pub fn process_keysym_press(&mut self, keysym: &KeySymbol) -> Option<KeyAction> {
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
}

#[derive(Debug)]
#[must_use]
pub enum EventAction {
    KeyAction(KeyAction),
    Unmatched(KeySymbol, HashSet<KeySymbol>),
    Ignore,
    NoSuchSymbol,
}
