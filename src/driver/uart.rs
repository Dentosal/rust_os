//! https://wiki.osdev.org/Serial_Ports
//! UART, output only, COM1 only

use core::sync::atomic::{AtomicBool, Ordering};
use cpuio::{inb, inw, outb};

const COM1: u16 = 0x3f8;

/// Returns true if serial exists and works.
/// # Safety
/// `port_base` must be valid
#[must_use]
unsafe fn init_serial(port_base: u16) -> bool {
    outb(0x00, port_base + 1); // Disable all interrupts
    outb(0x80, port_base + 3); // Enable DLAB (set baud rate divisor)
    outb(0x03, port_base + 0); // Set divisor to 3 (lo byte) 38400 baud
    outb(0x00, port_base + 1); //                  (hi byte)
    outb(0x03, port_base + 3); // 8 bits, no parity, one stop bit
    outb(0xc7, port_base + 2); // Enable FIFO, clear them, with 14-byte threshold
    outb(0x0b, port_base + 4); // IRQs enabled, RTS/DSR set
    outb(0x1b, port_base + 4); // Set in loopback mode, test the serial chip
    outb(0xae, port_base + 0); // Test serial chip (send byte 0xae and same returned)

    // Check if serial is faulty (i.e: not same byte as sent)
    if inb(port_base) != 0xae {
        return false;
    }

    // If serial is not faulty set it in normal operation mode
    // (not-loopback with IRQs enabled and OUT#1 and OUT#2 bits enabled)
    outb(0x0f, port_base + 4);
    true
}

unsafe fn is_transmit_empty(port_base: u16) -> bool {
    inb(port_base + 5) & 0x20 != 0
}

unsafe fn write_serial(port_base: u16, c: u8) {
    while !is_transmit_empty(port_base) {}
    outb(c, port_base);
}

static HAS_COM1: AtomicBool = AtomicBool::new(false);

pub fn has_com1() -> bool {
    HAS_COM1.load(Ordering::SeqCst)
}

pub fn write_com1(c: u8) {
    unsafe { write_serial(COM1, c) }
}

pub fn init() {
    let has_com1 = unsafe { init_serial(COM1) };
    log::debug!("COM1 enabled: {}", has_com1);
    HAS_COM1.store(has_com1, Ordering::SeqCst);
}
