use libd7::d7abi::ipc::protocol::keyboard::KeyboardEvent;

const RELEASED: u8 = 0xf0;
const ALTERNATIVE: u8 = 0xe0;
const ERROR_0: u8 = 0x00;
const ERROR_1: u8 = 0xff;

#[derive(Clone, Copy)]
pub struct KeyboardState {
    /// Next keypress is a release
    pub next_is_release: bool,
    /// Next keypress from alternative set
    pub next_is_alternative: bool,
}
impl KeyboardState {
    pub const fn new() -> Self {
        Self {
            next_is_release: false,
            next_is_alternative: false,
        }
    }

    pub fn apply(&mut self, byte: u8) -> Option<KeyboardEvent> {
        if byte == RELEASED {
            self.next_is_release = true;
            None
        } else if byte == ALTERNATIVE {
            self.next_is_alternative = true;
            None
        } else if byte == ERROR_0 || byte == ERROR_1 {
            self.next_is_release = false;
            self.next_is_alternative = false;
            None
        } else {
            let keycode = (byte as u16) | ((self.next_is_alternative as u16) << 8);
            let event = KeyboardEvent {
                keycode,
                release: self.next_is_release,
            };
            self.next_is_release = false;
            self.next_is_alternative = false;
            Some(event)
        }
    }
}
