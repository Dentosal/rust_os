/// http://wiki.osdev.org/PIC
use cpuio::{Port, UnsafePort};
use spin::Mutex;

// PIC ports
const PIC1_COMMAND: u16 = 0x20;
const PIC2_COMMAND: u16 = 0xA0;
const PIC1_DATA: u16 = 0x21;
const PIC2_DATA: u16 = 0xA1;

// PIC commands
const PIC_CMD_EOI: u8 = 0x20;
const PIC_CMD_INIT: u8 = 0x11;
const PIC_CMD_READ_ISR: u8 = 0x0b;

struct Pic {
    offset: u8,
    command_port: UnsafePort<u8>,
    data_port: UnsafePort<u8>,
}

impl Pic {
    /// Test if an interrupt id is operated by this pic
    fn handles_interrupt(&self, interupt_id: u8) -> bool {
        self.offset <= interupt_id && interupt_id < self.offset + 8
    }

    /// Read ISR: https://wiki.osdev.org/8259_PIC#ISR_and_IRR
    unsafe fn read_isr(&mut self) -> u8 {
        self.command_port.write(PIC_CMD_READ_ISR);
        self.command_port.read()
    }

    /// Send end of interrupt signal to pic
    unsafe fn send_eoi(&mut self) {
        self.command_port.write(PIC_CMD_EOI);
    }
}

pub struct ChainedPics {
    pics: [Pic; 2],
    enabled: bool,
}

impl ChainedPics {
    pub const unsafe fn new(pic1_offset: u8, pic2_offset: u8) -> ChainedPics {
        ChainedPics {
            pics: [
                Pic {
                    offset: pic1_offset,
                    command_port: UnsafePort::new(PIC1_COMMAND),
                    data_port: UnsafePort::new(PIC1_DATA),
                },
                Pic {
                    offset: pic2_offset,
                    command_port: UnsafePort::new(PIC2_COMMAND),
                    data_port: UnsafePort::new(PIC2_DATA),
                },
            ],
            enabled: false,
        }
    }
    /// Init - remap irqs
    /// http://wiki.osdev.org/PIC#Initialisation
    /// https://pdos.csail.mit.edu/6.828/2005/readings/hardware/8259A.pdf
    pub unsafe fn init(&mut self) {
        // Create local IO wait function
        let mut io_wait_port: Port<u8> = Port::new(0x80);
        let mut io_wait = || io_wait_port.write(0);

        // Initialization sequence
        for pic in &mut self.pics {
            pic.command_port.write(PIC_CMD_INIT);
            io_wait();
        }

        // Set interrupt offset
        for pic in &mut self.pics {
            pic.data_port.write(pic.offset);
            io_wait();
        }

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
        // PIC1: PIT, Keyboard, Cascade
        // PIC2: Free IRQs (9,10,11), Primary ATA
        // Disable everything else
        let mask1 = 0b11111000;
        let mask2 = 0b10110001;

        // Restore / Set masks
        self.pics[0].data_port.write(mask1);
        self.pics[1].data_port.write(mask2);

        self.enabled = true;
    }

    /// Disable PICs by masking all interrupts.
    /// Used when transitioning to LAPIC.
    /// https://wiki.osdev.org/PIC#Disabling
    pub fn disable(&mut self) {
        for pic in &mut self.pics {
            unsafe {
                pic.data_port.write(u8::MAX);
            }
        }
        self.enabled = false;
    }

    /// Verify that an interrupt is produced by pics
    pub fn handles_interrupt(&self, interrupt_id: u8) -> bool {
        self.pics.iter().any(|p| p.handles_interrupt(interrupt_id))
    }

    /// Read ISR values from pics as a u16 bitmap
    pub unsafe fn read_isr(&mut self) -> u16 {
        let isr0 = self.pics[0].read_isr() as u16;
        let isr1 = self.pics[1].read_isr() as u16;
        (isr1 << 8) | isr0
    }

    /// Send end of interrupt notification
    pub unsafe fn notify_eoi(&mut self, interrupt_id: u8) {
        if self.handles_interrupt(interrupt_id) {
            // chain
            if self.pics[1].handles_interrupt(interrupt_id) {
                self.pics[1].send_eoi();
            }
            self.pics[0].send_eoi();
        }
    }

    /// Send end of interrupt notification to primary PIC only
    /// Used for spurious interrupt handling
    pub unsafe fn notify_eoi_primary(&mut self) {
        self.pics[0].send_eoi();
    }
}

// Rempap interrupts to 0x20..0x30
pub static PICS: Mutex<ChainedPics> = Mutex::new(unsafe { ChainedPics::new(0x20, 0x28) });

pub fn init() {
    unsafe {
        PICS.lock().init();
    }
}

pub fn disable() {
    unsafe {
        PICS.lock().disable();
    }
}

pub fn is_enabled() -> bool {
    PICS.lock().enabled
}
