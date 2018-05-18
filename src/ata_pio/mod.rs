use alloc::Vec;
use cpuio::UnsafePort;
use spin::Mutex;

const SECTOR_SIZE: usize = 0x200;

const PORT_DATA:            u16 = 0x1F0;
const PORT_SECCOUNT:        u16 = 0x1F2;
const PORT_LBA0:            u16 = 0x1F3;
const PORT_LBA1:            u16 = 0x1F4;
const PORT_LBA2:            u16 = 0x1F5;
const PORT_LBA3:            u16 = 0x1F6;
const PORT_COMMAND:         u16 = 0x1F7;
const PORT_DEV_CTRL:        u16 = 0x3F6;

#[derive(Debug, Clone)]
pub struct DriveProperties {
    lba28_sectors: u32,
    lba48_sectors: Option<u64>,
}
impl DriveProperties {
    fn supports_lba48(&self) -> bool {
        self.lba48_sectors.is_some()
    }

    fn sector_count(&self) -> u64 {
        self.lba48_sectors.unwrap_or(self.lba28_sectors as u64)
    }
}

pub struct AtaPio {
    properties: Option<DriveProperties>
}
impl AtaPio {
    pub const fn new() -> AtaPio {
        AtaPio {
            properties: None
        }
    }

    pub fn init(&mut self) {
        unsafe {
            self.check_floating_bus();
            self.reset_drives();
            self.properties = Some(self.identify());
        }
        rprintln!("LBA48 support: {}", self.properties.clone().unwrap().supports_lba48());
    }

    #[inline]
    unsafe fn send_command(&self, cmd: u8) {
        let mut cmd_port = UnsafePort::<u8>::new(PORT_COMMAND);
        cmd_port.write(cmd);
    }

    #[inline]
    unsafe fn read_status(&self) -> u8 {
        let mut status_port = UnsafePort::<u8>::new(PORT_COMMAND);
        status_port.read()
    }

    unsafe fn reset_drives(&self) {
        // https://wiki.osdev.org/ATA_PIO_Mode#Resetting_a_drive_.2F_Software_Reset

        // TODO: currently using (primary bus, master drive) only

        let mut ctrl = UnsafePort::<u8>::new(PORT_DEV_CTRL);

        // Disable interupts, run software reset
        ctrl.write(0);

        // Wait for BSY to be clear and RDY set
        for _ in 0..4 { // 400ns delay
            let _ = ctrl.read();
        }

        loop {
            let v = ctrl.read();
            if (v & 0xc0) == 0x40 { // BSY clear, RDY set?
                break;
            }
        }
    }

    unsafe fn select_drive(&self) {
        // https://wiki.osdev.org/ATA_PIO_Mode#400ns_delays

        self.reset_drives(); // HACK: selects master drive
    }

    unsafe fn check_floating_bus(&self) {
        let data: u8 = self.read_status();
        if data == 0xFF {
            panic!("No ATA drives attached.");
        }
    }


    /// Polls ATA controller to see if the drive is ready
    #[inline]
    unsafe fn is_ready(&self) -> bool {
        for _ in 0..4 {
            let _ = self.read_status();
        }
        let data: u8 = self.read_status();
        (data & 0xc0) == 0x40 // BSY clear, RDY set?
    }

    unsafe fn wait_ready(&self) {
        while !self.is_ready() {}
    }

    unsafe fn identify(&self) -> DriveProperties {
        // https://wiki.osdev.org/ATA_PIO_Mode#IDENTIFY_command

        // Clear LBA_N ports
        let mut port_lba0 = UnsafePort::<u8>::new(PORT_LBA0); port_lba0.write(0);
        let mut port_lba1 = UnsafePort::<u8>::new(PORT_LBA1); port_lba1.write(0);
        let mut port_lba2 = UnsafePort::<u8>::new(PORT_LBA2); port_lba2.write(0);
        let mut port_lba3 = UnsafePort::<u8>::new(PORT_LBA3); port_lba3.write(0);

        // Send IDENTIFY command
        self.send_command(0xEC);

        use time::sleep_ms;
        sleep_ms(1);

        let mut first_cleared = true;
        loop {
            let data: u8 = self.read_status();

            if data == 0 {
                panic!("ATA_PIO: Drive does not exist");
            }

            if (data & 1) != 0 {
                panic!("ATA_PIO: Drive controller error on IDENTIFY");
            }

            if (data & (1 << 7)) != 0 { // is busy
                continue;
            }

            if first_cleared {
                first_cleared = false;
                let v1 = port_lba1.read();
                let v2 = port_lba2.read();
                if v1 != 0 || v2 != 0 {
                    panic!("ATA_PIO: Not an ATA drive");
                }
                continue;
            }

            if (data & (1 << 3)) != 0 {
                break;
            }
        }

        let mut data_port = UnsafePort::<u16>::new(PORT_DATA);
        let mut data: [u16; 256] = [0; 256];

        for i in 0..256 {
            data[i] = data_port.read();
            sleep_ms(1);
        }

        let lba48_supported = (data[83] & (1 << 10)) != 0;
        let lba28_sectors = (data[60] as u32) | ((data[61] as u32) << 0x10);
        let mut lba48_sectors: Option<u64> = None;
        if lba48_supported {
            lba48_sectors = Some(
                ( data[100] as u64) |
                ((data[101] as u64) << 0x10) |
                ((data[102] as u64) << 0x20) |
                ((data[103] as u64) << 0x30)
            );
        }

        if lba28_sectors == 0 && (lba48_sectors.is_none() || lba48_sectors == Some(0)) {
            panic!("ATA_PIO: The drive controller does not support LBA.");
        }

        DriveProperties {
            lba28_sectors,
            lba48_sectors
        }
    }

    pub unsafe fn read(&self, lba: u32, sectors: u8) -> Vec<u8> {
        // https://wiki.osdev.org/ATA_read/write_sectors#Read_in_LBA_mode

        assert!(sectors > 0);

        // Send bits 24-27 of LBA, drive number and LBA mode
        let mut port = UnsafePort::<u8>::new(PORT_LBA3);
        let mut bits24_27: u8 = (lba >> 24) as u8;
        assert!(bits24_27 < 8);
        bits24_27 |= 0b11100000; // LBA mode
        port.write(bits24_27);

        // Send number of sectors
        let mut port = UnsafePort::<u8>::new(PORT_SECCOUNT);
        port.write(sectors);

        // Send bits 0-7 of LBA
        let mut port = UnsafePort::<u8>::new(PORT_LBA0);
        port.write((lba & 0xFF) as u8);

        // Send bits 8-15 of LBA
        let mut port = UnsafePort::<u8>::new(PORT_LBA1);
        port.write(((lba & 0xFF00) >> 0x8) as u8);

        // Send bits 16-23 of LBA
        let mut port = UnsafePort::<u8>::new(PORT_LBA2);
        port.write(((lba & 0xFF0000) >> 0x10) as u8);

        // Send command
        self.send_command(0x20); // Read with retry

        self.wait_ready();

        let mut data_port = UnsafePort::<u16>::new(PORT_DATA);
        let u16_per_sector = SECTOR_SIZE / 2;

        let mut result: Vec<u8> = Vec::new();
        for _ in 0..sectors {
            for _ in 0..u16_per_sector {
                let word: u16 = data_port.read();
                result.push((word & 0xFF) as u8);
                result.push(((word & 0xFF00) >> 0x8) as u8);
            }
        }

        result
    }
}

// Create static pointer mutex with spinlock to make TERMINAL thread-safe
pub static ATA_PIO: Mutex<AtaPio> = Mutex::new(AtaPio::new());

pub fn init() {
    ATA_PIO.lock().init();
}