use super::key::Key;


const BUFFER_SIZE: usize = 10;

pub struct KeyReader {
    buffer: [Option<u8>; BUFFER_SIZE]
}
impl KeyReader {
    pub const fn new() -> KeyReader {
        KeyReader {
            buffer: [None; BUFFER_SIZE]
        }
    }

    /// Insert a byte into reader
    /// Returns a key if complete, else insert more
    pub fn insert(&mut self, b: u8) -> Option<Key> {
        self.buffer_insert(b);
        self.try_make_key()
    }

    /// Tries to convert buffer into key code
    /// Clears buffer if successful
    fn try_make_key(&mut self) -> Option<Key> {
        let key = get_key(self.buffer);
        if key.is_some() {
            for i in 0..BUFFER_SIZE {
                self.buffer[i] = None;
            }
        }
        key
    }

    fn buffer_insert(&mut self, b: u8) {
        for i in 0..BUFFER_SIZE {
            if self.buffer[i].is_none() {
                self.buffer[i] = Some(b);
                return;
            }
        }
        // FIXME: handle error properly
        panic!("KeyReader buffer full");
    }
}

fn get_key(buffer: [Option<u8>; BUFFER_SIZE]) -> Option<Key> {
    match buffer[0] {
        Some(b0) => {
            match b0 {
                0x1c => Some(Key::A),
                _ => None
            }
        }
        None => None
    }
}
