//! https://wiki.osdev.org/Ne2k

use alloc::vec::Vec;
use bitflags::bitflags;
use cpuio::UnsafePort;

use d7pci::Device;
use libd7::net::d7net::MacAddr;

const TX_BUFFER_COUNT: usize = 4;

mod reg {
    pub mod page0 {
        pub const CMD: u16 = 0; // Command
        pub const CLDA0: u16 = 1; // Currrent local DMA Address 0
        pub const CLDA1: u16 = 2; // Currrent local DMA Address 1
        pub const BNRY: u16 = 3; // Ringbuffer boundery pointer
        pub const TSR: u16 = 4; // Transmit status
        pub const NCR: u16 = 5; // Collision counter
        pub const FIFO: u16 = 6; // ??
        pub const ISR: u16 = 7; // Interrupt status
        pub const CRDA0: u16 = 8; // Currrent remote DMA Address 0
        pub const CRDA1: u16 = 9; // Currrent remote DMA Address 1
        pub const RSR: u16 = 0x0c; // Receive status register

        pub const REMOTE_DMA: u16 = 0x10; // Remote DMA port
        pub const RESET: u16 = 0x1f; // Reset register

        /// If register is different when written, it's listed with a W_ prefix
        pub const W_PSTART: u16 = 1; // Page start (init only)
        pub const W_PSTOP: u16 = 2; // Page stop (init only)
        pub const W_TPSR: u16 = 4; // transmit page start address
        pub const W_TBCR0: u16 = 5; // transmit byte count (low)
        pub const W_TBCR1: u16 = 6; // transmit byte count (high)
        pub const W_RSAR0: u16 = 8; // remote start address (lo)
        pub const W_RSAR1: u16 = 9; // remote start address (hi)
        pub const W_RBCR0: u16 = 0xa; // remote byte count (lo)
        pub const W_RBCR1: u16 = 0xb; // remote byte count (hi)
        pub const W_RCR: u16 = 0xc; // receive config register
        pub const W_TCR: u16 = 0xd; // transmit config register
        pub const W_DCR: u16 = 0xe; // data config register    (init)
        pub const W_IMR: u16 = 0xf; // interrupt mask register (init)
    }

    /// If register is different when written, it's listed here
    pub mod page1 {
        pub const PH_START: u16 = 1; // Physical address
        pub const MC_START: u16 = 8; // Multicast address
        pub const RX_CURRENT_PAGE: u16 = 7; // First page address for receiving packets
    }
}

bitflags! {
    /// Interrupt status flags
    struct IntStatus: u8 {
        /// Reset state, or buffer overflow
        const RESET = 1 << 7;
        /// Remote DMA operation complete
        const REMOTE_DMA_COMPLETE = 1 << 6;
        /// At least one counter has MSB set, i.e. halfway used
        const COUNTER_MSB = 1 << 5;
        /// Receiver buffer full
        const RX_BUFFER_FULL = 1 << 4;
        /// Transmit aborted (too many collisions)
        const TX_ERROR = 1 << 3;
        /// Receive error (missing or invalid packet)
        const RX_ERROR = 1 << 2;
        // Packet sent succesfully
        const TX_OK = 1 << 1;
        // Packet received succesfully
        const RX_OK = 1 << 0;

        const ERRORS = Self::RX_BUFFER_FULL.bits | Self::TX_ERROR.bits| Self::RX_ERROR.bits;
    }

    /// Receive status flags
    struct RxStatus: u8 {
        /// Defer on carrier or collision
        const DEFER = 1 << 7;
        /// Receiving disabled in monitor mode
        const DISABLED = 1 << 6;
        /// Multicast or broadcast packet
        const CAST_MULTI_BROAD = 1 << 5;
        /// Packet missing, either no buffer space or monitor mode
        const MISSING = 1 << 4;
        /// Reserved always one
        const RESERVED_ONE = 1 << 3;
        /// Invalid frame alignment of the incoming packet
        const FRAME_ALIGN_ERROR = 1 << 2;
        // Invalid CRC
        const CRC_ERROR = 1 << 1;
        // No errors
        const SUCCESS = 1 << 0;

        const ERRORS = Self::MISSING.bits | Self::FRAME_ALIGN_ERROR.bits | Self::CRC_ERROR.bits;
    }
}

const MEM_FIRST_PAGE: u8 = 0x40;
const MEM_SIZE_PAGES: u8 = 0x40;
const MEM_LAST_PAGE: u8 = MEM_FIRST_PAGE + MEM_SIZE_PAGES - 1;
const MEM_PAGE_SIZE_BYTES: u16 = 0x100;

/// Space reserved for tx buffer, enough for max size packat
const TX_PAGES: u8 = 8;

/// Space reserved for rx buffer
const RX_PAGES: u8 = MEM_SIZE_PAGES - TX_PAGES;

/// RX area start
const PSTART: u8 = MEM_FIRST_PAGE;

/// RX area end
const PSTOP: u8 = MEM_FIRST_PAGE + RX_PAGES - 1;

/// TX area start
const TX_START: u8 = PSTOP + 1;

enum CmdFunc {
    RemoteRead,
    RemoteWrite,
    SendPacket,
    NoDMA,
}

// Build command register operation
fn cmd(page: u8, func: CmdFunc, txp: bool, start: bool) -> u8 {
    assert!(page <= 3);

    let func_bits = match func {
        CmdFunc::RemoteRead => 0b001,
        CmdFunc::RemoteWrite => 0b010,
        CmdFunc::SendPacket => 0b011,
        CmdFunc::NoDMA => 0b100,
    };

    (page << 6) | (func_bits << 3) | ((txp as u8) << 2) | (1u8 << if start { 1 } else { 0 })
}

pub struct Ne2k {
    pci_device: Device,
    pub irq: u8,
    io_base: u16,
    link_up: bool,
    mac_addr: MacAddr,
    /// Position of next packet in the ring buffer
    next_packet: u8,
}
impl Ne2k {
    pub unsafe fn new(pci_device: Device) -> Self {
        // TODO: check PCI id
        // assert_eq!((pci_device.vendor, pci_device.id), (0x10ec, 0x8139));
        // Ethernet controller
        assert_eq!((pci_device.class.0, pci_device.class.1), (2, 0));

        pci_device.enable_bus_mastering();

        let mut device = Self {
            pci_device,
            irq: pci_device
                .get_interrupt_line()
                .expect("Missing interrupt line"),
            io_base: (pci_device.get_bar(0) & (!3u32)) as u16,
            mac_addr: MacAddr::ZERO,
            link_up: false,
            next_packet: PSTART + 1,
        };
        device.reset();
        device
    }

    fn reset(&mut self) {
        println!("Resetting ne2k");

        unsafe {
            let base = self.io_base;
            let port = |n| UnsafePort::<u8>::new(base + n);

            // Write reset command
            let b: u8 = port(reg::page0::RESET).read();
            port(reg::page0::RESET).write(b);

            // Wait for the reset to complete
            while (port(reg::page0::ISR).read() & 0x80) == 0 {
                println!("Reset loop");
            }

            // Clear all interrupts
            port(reg::page0::ISR).write(0xff);

            // Page 0, no DMA, stop
            port(reg::page0::CMD).write(cmd(0, CmdFunc::NoDMA, false, false));

            // Set word-sized DMA transfer, no loopback
            port(reg::page0::W_DCR).write(0b0100_1001);

            // Clear the count registers
            port(reg::page0::W_RBCR0).write(0);
            port(reg::page0::W_RBCR1).write(0);

            // Disable all interrupts (mask)
            port(reg::page0::W_IMR).write(0);
            port(reg::page0::ISR).write(0xff);

            // Set to monitor, e.g. do not buffer packets to memory automatically
            port(reg::page0::W_RCR).write(0x0e);
            // port(reg::page0::W_RCR).write(0x20);

            // TODO: internal loopback mode
            port(reg::page0::W_TCR).write(0x02);

            // Read 6 bytes (for some reason the count is halved by the chip?)
            port(reg::page0::W_RBCR0).write(12); // Low
            port(reg::page0::W_RBCR1).write(0); // High

            // DNA start address
            port(reg::page0::W_RSAR0).write(0); // Low
            port(reg::page0::W_RSAR1).write(0); // High

            // Start read
            port(reg::page0::CMD).write(cmd(0, CmdFunc::RemoteRead, false, true));

            // Read MAC addr
            let mut mac_addr = [0u8; 6];
            for i in 0..mac_addr.len() {
                mac_addr[i] = port(reg::page0::REMOTE_DMA).read();
            }

            self.mac_addr = MacAddr(mac_addr);

            // Set up ring buffers
            port(reg::page0::W_PSTART).write(PSTART);
            port(reg::page0::W_PSTOP).write(PSTOP);
            port(reg::page0::BNRY).write(PSTART);

            port(reg::page0::W_TPSR).write(TX_START);

            // Enable interrupts with given mask
            self.mask_isr(IntStatus::ERRORS | IntStatus::RX_OK);

            // Write stop command twice, sets page number to 1
            port(reg::page0::CMD).write(cmd(1, CmdFunc::NoDMA, false, false));
            port(reg::page0::CMD).write(cmd(1, CmdFunc::NoDMA, false, false));

            // Write MAC addr PAR0..6 registers to listen to it
            for i in 0..mac_addr.len() {
                port((i + 1) as u16).write(mac_addr[i]);
            }

            // Set word-sized DMA transfer, no loopback
            port(reg::page0::W_DCR).write(0b0100_1001);

            // Write current page to register page 1
            self.port(reg::page0::CMD)
                .write(cmd(1, CmdFunc::NoDMA, false, true));
            self.port(reg::page1::RX_CURRENT_PAGE)
                .write(self.next_packet);
            self.port(reg::page0::CMD)
                .write(cmd(0, CmdFunc::NoDMA, false, true));

            // Normal mode, CRC enabled
            port(reg::page0::W_TCR).write(0);

            // Enable receiving multicast and broadcast packets
            port(reg::page0::W_RCR).write(0x0c);
        }

        println!("Reset of ne2k complete");
    }

    pub fn send(&mut self, packet: &[u8]) {
        let txbuffer = 0x40;

        unsafe {
            let base = self.io_base;
            let port = |n| UnsafePort::<u8>::new(base + n);

            // Packet size (read)
            port(reg::page0::W_RBCR0).write(packet.len() as u8);
            port(reg::page0::W_RBCR1).write((packet.len() >> 8) as u8);

            // Page number
            port(reg::page0::W_RSAR0).write(0);
            port(reg::page0::W_RSAR1).write(txbuffer);

            port(reg::page0::CMD).write(cmd(0, CmdFunc::RemoteWrite, false, true));

            println!("write {}", packet.len());

            // TODO: buffer bound check

            let mut data_port = UnsafePort::<u16>::new(base + reg::page0::REMOTE_DMA);
            for slice in packet.chunks(2) {
                data_port.write(u16::from_le_bytes([
                    slice[0],
                    slice.get(1).copied().unwrap_or(0),
                ]));
            }

            println!("Polling status");
            loop {
                if self.read_isr().contains(IntStatus::REMOTE_DMA_COMPLETE) {
                    break;
                }
            }
            println!("Polling status done");

            // Page number
            port(reg::page0::W_TPSR).write(txbuffer);

            // Packet size (transmit)
            port(reg::page0::W_TBCR0).write(packet.len() as u8);
            port(reg::page0::W_TBCR1).write((packet.len() >> 8) as u8);

            port(reg::page0::CMD).write(cmd(0, CmdFunc::NoDMA, true, true));

            println!("Polling status");
            loop {
                if self.read_isr().contains(IntStatus::TX_OK) {
                    break;
                }
            }
            println!("Polling status done");
            self.clear_isr(IntStatus::REMOTE_DMA_COMPLETE | IntStatus::TX_OK);
        }
    }

    /// Assumes page 0 is active
    fn read_rsr(&self) -> RxStatus {
        unsafe {
            let base = self.io_base;
            let port = |n| UnsafePort::<u8>::new(base + n);
            RxStatus::from_bits_truncate(port(reg::page0::RSR).read())
        }
    }

    /// Assumes page 0 is active
    fn read_isr(&self) -> IntStatus {
        unsafe {
            let base = self.io_base;
            let port = |n| UnsafePort::<u8>::new(base + n);
            IntStatus::from_bits_truncate(port(reg::page0::ISR).read())
        }
    }

    /// Assumes page 0 is active
    fn clear_isr(&self, clear: IntStatus) {
        unsafe {
            let base = self.io_base;
            let port = |n| UnsafePort::<u8>::new(base + n);
            port(reg::page0::ISR).write(clear.bits);
        }
    }

    /// Assumes page 0 is active
    fn mask_isr(&self, mask: IntStatus) {
        unsafe {
            let base = self.io_base;
            let port = |n| UnsafePort::<u8>::new(base + n);
            port(reg::page0::W_IMR).write(mask.bits);
        }
    }

    fn port(&self, offset: u16) -> UnsafePort<u8> {
        unsafe {
            let base = self.io_base;
            UnsafePort::<u8>::new(base + offset)
        }
    }

    fn read_dma(&self, page: u8, offset: u8, buffer: &mut [u8]) {
        assert!(buffer.len() <= (u16::MAX as usize));
        assert!(page >= MEM_FIRST_PAGE && page <= MEM_LAST_PAGE);
        assert!(
            ((page as u16) >> 8) | (offset as u16) + (buffer.len() as u16)
                < ((MEM_LAST_PAGE as u16) + 1) * MEM_PAGE_SIZE_BYTES
        );

        unsafe {
            self.port(reg::page0::W_RBCR0).write(buffer.len() as u8);
            self.port(reg::page0::W_RBCR1)
                .write((buffer.len() >> 8) as u8);
            self.port(reg::page0::W_RSAR0).write(offset);
            self.port(reg::page0::W_RSAR1).write(page);
            self.port(reg::page0::CMD)
                .write(cmd(0, CmdFunc::RemoteRead, false, true));

            let mut data_port = UnsafePort::<u16>::new(self.io_base + reg::page0::REMOTE_DMA);

            for chunk in buffer.chunks_mut(2) {
                let w = data_port.read();
                let b = w.to_le_bytes();
                chunk[0] = b[0];
                if chunk.len() == 2 {
                    chunk[1] = b[1];
                }
            }

            self.clear_isr(IntStatus::REMOTE_DMA_COMPLETE);
        }
    }

    /// Called when new packet has been received
    fn read_packets(&mut self) -> Vec<Vec<u8>> {
        let mut result = Vec::new();

        unsafe {
            // Read current page from register page 1
            self.port(reg::page0::CMD)
                .write(cmd(1, CmdFunc::NoDMA, false, true));
            let curr = self.port(reg::page1::RX_CURRENT_PAGE).read();
            self.port(reg::page0::CMD)
                .write(cmd(0, CmdFunc::NoDMA, false, true));

            while curr != self.next_packet {
                println!("loop {} != {}", curr, self.next_packet);

                // Read packet info
                let mut buf = [0; 4];
                self.read_dma(self.next_packet, 0, &mut buf);
                let rsr = RxStatus::from_bits_truncate(buf[0]);
                let next = buf[1];
                let len = (buf[2] as u16) | ((buf[3] as u16) << 8);
                let len = len - 4; // ???

                println!("pckt info {:?} {} {}", rsr, next, len);

                if (rsr & RxStatus::ERRORS).is_empty()
                    && rsr.contains(RxStatus::SUCCESS)
                    && next >= PSTART
                    && next <= PSTOP
                    && len <= 1532
                {
                    println!("ne2k: Recv ok, reading {} bytes", len);
                    let mut packet = vec![0; len as usize];
                    self.read_dma(self.next_packet, 4, &mut packet);
                    result.push(packet);
                }

                // Ring buffer wraparound
                self.next_packet = if next >= PSTOP { PSTART } else { next };

                // Update boundary with backwards wraparound if required
                self.port(reg::page0::BNRY)
                    .write(if self.next_packet == PSTART {
                        PSTOP - 1
                    } else {
                        self.next_packet - 1
                    });

                // Write current page to register page 1
                self.port(reg::page0::CMD)
                    .write(cmd(1, CmdFunc::NoDMA, false, true));
                self.port(reg::page1::RX_CURRENT_PAGE).write(curr);
                self.port(reg::page0::CMD)
                    .write(cmd(0, CmdFunc::NoDMA, false, true));
            }
        }
        result
    }

    pub fn notify_irq(&mut self) -> Vec<Vec<u8>> {
        let mut result = Vec::new();

        loop {
            let status = self.read_isr();

            println!("ne2k: Processing irq, status={:?}", status);

            if status.is_empty() {
                return result;
            }

            if status.contains(IntStatus::RX_OK) {
                result.extend(self.read_packets());
                self.clear_isr(IntStatus::RX_OK);
            }

            if status.contains(IntStatus::COUNTER_MSB) {
                // TODO: persist counters?
                self.clear_isr(IntStatus::COUNTER_MSB);
            }

            if status.contains(IntStatus::REMOTE_DMA_COMPLETE) {
                self.clear_isr(IntStatus::REMOTE_DMA_COMPLETE);
            }

            if status.contains(IntStatus::TX_OK) {
                panic!("ne2k: TX_OK set unexpectedly");
                // self.clear_isr(IntStatus::TX_OK);
            }

            if status.contains(IntStatus::RESET) {
                panic!("ne2k: RESET set unexpectedly");
                // self.clear_isr(IntStatus::RESET);
            }

            if status.contains(IntStatus::ERRORS) {
                todo!("ne2k: ERROR => handle");
                // self.clear_isr(IntStatus::ERROR);
            }
        }
    }

    pub fn mac_addr(&self) -> MacAddr {
        self.mac_addr
    }
}
