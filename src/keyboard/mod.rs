mod key;
mod keymap;

use spin::Mutex;
use cpuio::{Port, UnsafePort};

use util::io_wait;

use self::key::Key;
use self::keymap::KeyReader;

// PS/2 ports
const PS2_DATA:     u16 = 0x60; // rw
const PS2_STATUS:   u16 = 0x64; // r-
const PS2_COMMAND:  u16 = 0x64; // -w

// PIC commands
const PIC_CMD_EOI:  u8 = 0x20;
const PIC_CMD_INIT: u8 = 0x11;

// Sensible timeout
const IO_WAIT_TIMEOUT: usize = 100;

// Event buffer
const EVENT_BUFFER_SIZE: usize = 100;


pub struct Keyboard {
    enabled: bool,
    offset: u8,
    data_port: UnsafePort<u8>,
    status_port: UnsafePort<u8>,
    command_port: UnsafePort<u8>,
    key_reader: KeyReader
}

impl Keyboard {
    pub const unsafe fn new() -> Keyboard {
        Keyboard {
            enabled: false,
            offset: 0,
            data_port: UnsafePort::new(PS2_DATA),
            status_port: UnsafePort::new(PS2_STATUS),
            command_port: UnsafePort::new(PS2_COMMAND),
            key_reader: KeyReader::new()
        }
    }

    unsafe fn init(&mut self) {
        if self.self_test() {
            rprintln!("Keyboard: self test ok");
        }
        else {
            rprintln!("Keyboard: self test failed");
            panic!("Keyboard: self test failed");
        }

        // self.test_echo();
        // rprintln!("Keyboard: echo ok");

        self.disable_scanning();
        self.verify_keyboard();
        self.configure();
        self.enable_scanning();
        self.enabled = true;
    }

    pub unsafe fn test_result(&mut self, result: u8) -> bool {
        for _ in 0..IO_WAIT_TIMEOUT {
            io_wait();
            if self.read_byte() == result {
                return true;
            }
        }
        false
    }

    pub unsafe fn test_echo(&mut self) -> bool {
        self.data_port.write(0xEE);
        self.test_result(0xEE)
    }

    pub unsafe fn self_test(&mut self) -> bool {
        self.data_port.write(0xFF);
        self.test_result(0xAA)
    }

    pub unsafe fn verify_keyboard(&mut self) {
        self.data_port.write(0xF2);

        // This doesn't care about ack byte (0xFA), because Qemu doesn't support it, and it's not needed

        for _ in 0..IO_WAIT_TIMEOUT {
            let x = self.read_byte();
            if x == 0xAB {
                for _ in 0..IO_WAIT_TIMEOUT {
                    let kbd_subtype = self.read_byte();
                    if kbd_subtype == 0x41 || kbd_subtype == 0xC1 || kbd_subtype == 0x83 {
                        return;
                    }
                }
                panic!("Unsupported keyboard: unknown subtype");
            }
            io_wait();
        }
        panic!("Unsupported keyboard: cannot identify");
    }

    pub unsafe fn configure(&mut self) {
        // configure PS/2 controller
        self.ps2_write_command(0x20);
        io_wait();
        let mut conf = self.read_byte();
        conf &= 0b1011_1111; // disable translation
        self.ps2_write_command(0x60);
        self.data_port.write(conf);

        // configure keyboard
        self.data_port.write(0xF0);
        io_wait();
        self.data_port.write(0x02);
        io_wait();
        if !self.test_result(0xFA) {
            panic!("Unsupported keyboard");
        }
    }

    pub unsafe fn disable_scanning(&mut self) {
        self.data_port.write(0xF5);
        if !self.test_result(0xFA) {
            panic!("Unsupported keyboard");
        }
    }

    pub unsafe fn enable_scanning(&mut self) {
        self.data_port.write(0xF4);
        io_wait();
        if self.read_byte() != 0xFA {
            panic!("Unsupported keyboard");
        }
    }

    pub unsafe fn notify(&mut self) {
        let key = self.read_byte();
        if !self.enabled || key == 0xFA || key == 0xEE {
            return;
        }
        match self.key_reader.insert(key) {
            Some(key) => {
                let event = KeyboardEvent::new(key);
                rprintln!("YES: {:?}", event.key);
                loop {}
            }
            None => {
                rprintln!("{:x}", key);
            }
        }

    }

    unsafe fn read_byte(&mut self) -> u8 {
        let mut c: u8 = 0;
        loop {
            if self.data_port.read() != c {
                c = self.data_port.read();
                if c > 0 {
                    return c;
                }
            }
        }
    }
    unsafe fn ps2_write_command(&mut self, c: u8) {
        loop {
            if self.status_port.read() & 2 == 0 {
                break;
            }
        }
        self.command_port.write(c)
    }
}

#[derive(Clone,Copy,Debug)]
pub struct KeyboardEvent {
    pub key: Key,
    // modifiers
    // timestamp?
    // event type (press, release, repeat?)
    // char
}
impl KeyboardEvent {
    pub fn new(key: Key) -> KeyboardEvent {
        KeyboardEvent {
            key: key
        }
    }
}


pub static KEYBOARD: Mutex<Keyboard> = Mutex::new(unsafe { Keyboard::new() });


pub fn init() {
    unsafe {
        KEYBOARD.lock().init();
    }
}
