use super::key::Key;

use collections::Vec;

pub struct Keymap {
    mapping: Option<Vec<(Vec<u8>, Key)>>
}

impl Keymap {
    pub const fn new() -> Keymap {
        Keymap {
            mapping: None
        }
    }

    pub fn init(&mut self) {
        self.mapping = Some(vec![
            (vec![0x1c], Key::A)
        ]);
    }

    pub fn get(&self, v: Vec<u8>) -> Option<Key> {
        match self.mapping {
            Some(ref mapping) => mapping.iter().find(|x| x.0==v).map(|x| x.1),
            None => None
        }
    }
}
