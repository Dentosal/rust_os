//! ATA PIO mode driver: https://wiki.osdev.org/ATA_PIO
//! Slow disk transfer supported by all ATA drives.
//! This driver only supports primary ATA bus and only the first drive.

use cpuio::{inb, inw, outb};

use super::error;

pub const SECTOR_SIZE: usize = 0x200;

const PORT_DATA: u16 = 0x1F0;
const PORT_SECCOUNT: u16 = 0x1F2;
const PORT_LBA0: u16 = 0x1F3;
const PORT_LBA1: u16 = 0x1F4;
const PORT_LBA2: u16 = 0x1F5;
const PORT_DRIVESELECT: u16 = 0x1F6;
const PORT_COMMAND: u16 = 0x1F7;
const PORT_DEV_CTRL: u16 = 0x3F6;

pub fn init() -> u64 {
    unsafe {
        reset_drives();
    };

    unsafe { identify() }
}

unsafe fn reset_drives() {
    // https://wiki.osdev.org/ATA_PIO_Mode#Resetting_a_drive_.2F_Software_Reset

    // Disable interupts, run software reset
    outb(0, PORT_DEV_CTRL);

    // Wait for BSY to be clear and RDY set
    for _ in 0..4 {
        // 400ns delay
        let _ = inb(PORT_DEV_CTRL);
    }

    loop {
        let v = inb(PORT_DEV_CTRL);
        if (v & 0xc0) == 0x40 {
            // BSY clear, RDY set?
            break;
        }
    }
}

pub unsafe fn read_lba(lba: u64, sectors: u8, dst: *mut u16) {
    // https://wiki.osdev.org/ATA_read/write_sectors#Read_in_LBA_mode

    if sectors == 0 {
        error('N'); // No sectors
    }

    if lba >= (1 << 28) {
        error('6'); // no LBA64 support (yet)
    }

    // Send bits 24-27 of LBA, drive number and LBA mode
    let mut bits24_27: u8 = (lba >> 24) as u8;
    bits24_27 |= 0b11100000; // LBA mode
    outb(bits24_27, PORT_DRIVESELECT);

    // Send number of sectors
    outb(sectors, PORT_SECCOUNT);

    // Send bits 0-7 of LBA
    outb((lba & 0xFF) as u8, PORT_LBA0);

    // Send bits 8-15 of LBA
    outb(((lba & 0xFF00) >> 0x8) as u8, PORT_LBA1);

    // Send bits 16-23 of LBA
    outb(((lba & 0xFF0000) >> 0x10) as u8, PORT_LBA2);

    // Send command
    send_command(0x20); // Read with retry

    wait_ready();

    let u16_per_sector = SECTOR_SIZE / 2;

    let mut offset = 0;
    for _ in 0..sectors {
        for _ in 0..u16_per_sector {
            let word: u16 = inw(PORT_DATA);
            *dst.add(offset) = word;
            offset += 1;
        }
    }
}

unsafe fn send_command(cmd: u8) {
    outb(cmd, PORT_COMMAND);
}

unsafe fn read_status() -> u8 {
    inb(PORT_COMMAND)
}

/// Polls ATA controller to see if the drive is ready
unsafe fn is_ready() -> bool {
    for _ in 0..4 {
        let _ = read_status();
    }
    let data: u8 = read_status();
    (data & 0xc0) == 0x40 // BSY clear, RDY set?
}

/// Polls ATA controller to until the drive is ready
unsafe fn wait_ready() {
    while !is_ready() {}
}

/// Reads identification of a drive
unsafe fn identify() -> u64 {
    // https://wiki.osdev.org/ATA_PIO_Mode#IDENTIFY_command

    outb(0xa0, PORT_DRIVESELECT);

    // Clear ports
    outb(0, PORT_SECCOUNT);
    outb(0, PORT_LBA0);
    outb(0, PORT_LBA1);
    outb(0, PORT_LBA2);

    // Send IDENTIFY command
    send_command(0xEC);

    let mut first_cleared = true;
    loop {
        let data: u8 = read_status();

        if data == 0 {
            // Drive does not exist
            return 0;
        }

        if (data & 1) != 0 {
            error('I'); // Drive controller error on IDENTIFY
        }

        if (data & (1 << 7)) != 0 {
            // is busy
            continue;
        }

        if first_cleared {
            first_cleared = false;
            let v1 = inb(PORT_LBA1);
            let v2 = inb(PORT_LBA2);
            if v1 != 0 || v2 != 0 {
                // Not an ATA drive
                error('A');
            }
            continue;
        }

        if (data & (1 << 3)) != 0 {
            break;
        }
    }

    let mut data: [u16; 256] = [0; 256];

    for i in 0..256 {
        data[i] = inw(PORT_DATA);
    }

    let lba48_supported = (data[83] & (1 << 10)) != 0;
    let lba28_sectors = ((data[60] as u32) | ((data[61] as u32) << 0x10)) as u64;
    let lba48_sectors: u64 = if lba48_supported {
        (data[100] as u64)
            | ((data[101] as u64) << 0x10)
            | ((data[102] as u64) << 0x20)
            | ((data[103] as u64) << 0x30)
    } else {
        0
    };

    if lba28_sectors == 0 && lba48_sectors == 0 {
        // The drive controller does not support LBA
        error('L');
    }

    if lba48_supported {
        lba48_sectors.max(lba28_sectors)
    } else {
        lba28_sectors
    }
}
