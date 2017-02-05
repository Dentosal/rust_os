// Mostly taken from, with MIT license
// https://github.com/emk/toyos-rs/blob/master/crates/pic8259_simple/src/lib.rs
// and from, public domain:
// http://wiki.osdev.org/PIC#Programming_the_PIC_chips

use spin::Mutex;
use cpuio::{Port, UnsafePort};

// PIC ports
const PIC1_COMMAND: u16 = 0x20;
const PIC2_COMMAND: u16 = 0xA0;
const PIC1_DATA:    u16 = 0x21;
const PIC2_DATA:    u16 = 0xA1;

// PIC commands
const PIC_CMD_EOI:  u8 = 0x20;
const PIC_CMD_INIT: u8 = 0x11;


struct Pic {
    offset: u8,
    command_port: UnsafePort<u8>,
    data_port: UnsafePort<u8>,
}

impl Pic {
    fn handles_interrupt(&self, interupt_id: u8) -> bool {
        self.offset <= interupt_id && interupt_id < self.offset + 8
    }
    /// Send end of interrupt signal to pic
    unsafe fn send_eoi(&mut self) {
        self.command_port.write(PIC_CMD_EOI);
    }
}


pub struct ChainedPics {
    pics: [Pic; 2],
}

impl ChainedPics {
    pub const unsafe fn new(pic1_offset: u8, pic2_offset: u8) -> ChainedPics {
        ChainedPics {
            pics: [
                Pic {
                    offset: pic1_offset,
                    command_port:   UnsafePort::new(PIC1_COMMAND),
                    data_port:      UnsafePort::new(PIC1_DATA),
                },
                Pic {
                    offset: pic2_offset,
                    command_port:   UnsafePort::new(PIC2_COMMAND),
                    data_port:      UnsafePort::new(PIC2_DATA),
                },
            ]
        }
    }
    /// Init - remap irqs (http://wiki.osdev.org/PIC#Initialisation, https://pdos.csail.mit.edu/6.828/2005/readings/hardware/8259A.pdf)
    pub unsafe fn init(&mut self) {
        // Create local IO wait function
        let mut io_wait_port: Port<u8> = Port::new(0x80);
        let mut io_wait = || { io_wait_port.write(0) };

        // Save masks
        let mut mask1: u8 = self.pics[0].data_port.read();
        let mut mask2: u8 = self.pics[1].data_port.read();

        // Initialization sequence
        self.pics[0].command_port.write(PIC_CMD_INIT);
        io_wait();
        self.pics[1].command_port.write(PIC_CMD_INIT);
        io_wait();

        // Set interrupt offset
        self.pics[0].data_port.write(self.pics[0].offset);
        io_wait();
        self.pics[1].data_port.write(self.pics[1].offset);
        io_wait();

        // Introduce PICs to each other
        self.pics[0].data_port.write(4);
        io_wait();
        self.pics[1].data_port.write(2);
        io_wait();

        // Use 8086/88 (MCS-80/85) mode (http://forum.osdev.org/viewtopic.php?f=1&t=12960&start=0)
        self.pics[0].data_port.write(1);
        io_wait();
        self.pics[1].data_port.write(1);
        io_wait();

        // Modify masks
        // http://wiki.osdev.org/IRQ#Standard_ISA_IRQs
        // http://wiki.osdev.org/8259_PIC#Masking
        mask1 &= 0b11111100; // Enable PIT and Keyboard
        mask2 &= 0b11111111; // Do nothing

        // Restore / Set masks
        self.pics[0].data_port.write(mask1);
        self.pics[1].data_port.write(mask2);
    }
    pub fn handles_interrupt(&self, interrupt_id: u8) -> bool {
        self.pics.iter().any(|p| p.handles_interrupt(interrupt_id))
    }
    pub unsafe fn notify_eoi(&mut self, interrupt_id: u8) {
        if self.handles_interrupt(interrupt_id) {
            // chain
            if self.pics[1].handles_interrupt(interrupt_id) {
                self.pics[1].send_eoi();
            }
            self.pics[0].send_eoi();
        }
    }
}

pub static PICS: Mutex<ChainedPics> = Mutex::new(unsafe { ChainedPics::new(0x20, 0x28) });

pub fn init() {
    unsafe {
        PICS.lock().init();
    }
}
