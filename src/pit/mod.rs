#![allow(unused_variables)]
#![allow(unused_imports)]

use util::{inb, outb};

const PIT_CH0: u16 = 0x40; // Channel 0 data port (read/write) (PIC TIMER)
const PIT_CH1: u16 = 0x41; // Channel 1 data port (read/write) (UNUSED)
const PIT_CH2: u16 = 0x42; // Channel 2 data port (read/write) (PC SPEAKER)
const PIT_REG: u16 = 0x43; // Mode/Command register (write only, a read is ignored)
const PIT_CH2_CONTROL: u16 = 0x61;

// set frequency approximately to 1000 Hz
// frequency = (1193182 / reload) Hz <=> reload = (1193182 Hz / frequency)
// reload = 1193182.0 / 1000.0 = 1193.182 => round => 1193
// actual_freq = 1193182.0/reload = 1000.15255660
// time_between = 1/actual_freq = 0.0009998474667 s ~= 999.847467 ns
// floats are disabled in kernel code, so these are calculated by hand
const TARGET_FREQ: u64 = 1000; // Hz
const RELOAD_VALUE: u64 = 1193;
pub const ACTUAL_FREQ_E_9: u64 = 1000_152556600; // Hz * 10 ** 12
pub const TIME_BETWEEN_E_12: u64 = 999847467; // s * 10 ** 12

pub fn init() {
    // Channel 0, lobyte/hibyte, Rate generator, Binary mode
    unsafe {
        outb(PIT_REG, 0b00_11_010_0); // command
        outb(PIT_CH0, (RELOAD_VALUE & 0x00FF) as u8); // low
        outb(PIT_CH0, ((RELOAD_VALUE & 0xFF00) >> 8) as u8); // high
    }
}
