//! https://wiki.osdev.org/RTL8139

use alloc::vec::Vec;
use bitflags::bitflags;
use core::intrinsics::{copy_nonoverlapping, write_bytes};
use cpuio::UnsafePort;

use d7pci::Device;
use libd7::net::d7net::MacAddr;

use super::dma::DMARegion;

const TX_BUFFER_COUNT: usize = 4;

mod reg {
    pub const MAC: u16 = 0x00;
    pub const MAR0: u16 = 0x08;
    pub const MAR4: u16 = 0x12;
    pub const TXSTATUS0: u16 = 0x10;
    pub const TXADDR0: u16 = 0x20;
    pub const RXBUF: u16 = 0x30;
    pub const COMMAND: u16 = 0x37;
    pub const CAPR: u16 = 0x38;
    pub const IMR: u16 = 0x3C;
    pub const ISR: u16 = 0x3E;
    pub const TXCFG: u16 = 0x40;
    pub const RXCFG: u16 = 0x44;
    pub const MPC: u16 = 0x4C;
    pub const CFG9346: u16 = 0x50;
    pub const CONFIG1: u16 = 0x52;
    pub const MSR: u16 = 0x58;
    pub const BMCR: u16 = 0x62;
}

mod tx_status {
    pub const OWN: u32 = 0x2000;
    pub const THRESHOLD_MAX: u32 = 0x3F0000;
}

mod command {
    pub const RX_EMPTY: u8 = 0x01;
    pub const TX_ENABLE: u8 = 0x04;
    pub const RX_ENABLE: u8 = 0x08;
    pub const RESET: u8 = 0x10;
}

bitflags! {
    /// Interrupt status flags
    struct IntFlags: u16 {
        const RXOK = 0x01;
        const RXERR = 0x02;
        const TXOK = 0x04;
        const TXERR = 0x08;
        const RX_BUFFER_OVERFLOW = 0x10;
        const LINK_CHANGE = 0x20;
        const RX_FIFO_OVERFLOW = 0x40;
        const LENGTH_CHANGE = 0x2000;
        const SYSTEM_ERROR = 0x8000;

        const ALL_SUPPORTED = Self::RXOK.bits
                    | Self::RXERR.bits
                    | Self::TXOK.bits
                    | Self::TXERR.bits
                    | Self::RX_BUFFER_OVERFLOW.bits
                    | Self::LINK_CHANGE.bits
                    | Self::RX_FIFO_OVERFLOW.bits
                    | Self::LENGTH_CHANGE.bits
                    | Self::SYSTEM_ERROR.bits;
    }
}

bitflags! {
    /// Receive status flags
    struct RxStatus: u16 {
        const MULTICAST = 0x8000;
        const PHYSICAL_MATCH = 0x4000;
        const BROADCAST = 0x2000;
        const INVALID_SYMBOL_ERROR = 0x20;
        const RUNT = 0x10;
        const LONG = 0x08;
        const CRC_ERROR = 0x04;
        const FRAME_ALIGNMENT_ERROR = 0x02;
        const OK = 0x01;

        const ALL_ERRORS =
            Self::INVALID_SYMBOL_ERROR.bits | Self::CRC_ERROR.bits | Self::FRAME_ALIGNMENT_ERROR.bits;
    }
}

mod cfg9346 {
    pub const NONE: u8 = 0x00;
    pub const EEM0: u8 = 0x40;
    pub const EEM1: u8 = 0x80;
}

mod tx_cfg {
    pub const TXRR_ZERO: u32 = 0x00;
    pub const MAX_DMA_16B: u32 = 0x000;
    pub const MAX_DMA_32B: u32 = 0x100;
    pub const MAX_DMA_64B: u32 = 0x200;
    pub const MAX_DMA_128B: u32 = 0x300;
    pub const MAX_DMA_256B: u32 = 0x400;
    pub const MAX_DMA_512B: u32 = 0x500;
    pub const MAX_DMA_1K: u32 = 0x600;
    pub const MAX_DMA_2K: u32 = 0x700;
    pub const IFG11: u32 = 0x3000000;
}

mod rx_cfg {
    pub const AAP: u32 = 0x01;
    pub const APM: u32 = 0x02;
    pub const AM: u32 = 0x04;
    pub const AB: u32 = 0x08;
    pub const AR: u32 = 0x10;
    pub const WRAP_INHIBIT: u32 = 0x80;
    pub const MAX_DMA_16B: u32 = 0x000;
    pub const MAX_DMA_32B: u32 = 0x100;
    pub const MAX_DMA_64B: u32 = 0x200;
    pub const MAX_DMA_128B: u32 = 0x300;
    pub const MAX_DMA_256B: u32 = 0x400;
    pub const MAX_DMA_512B: u32 = 0x500;
    pub const MAX_DMA_1K: u32 = 0x600;
    pub const MAX_DMA_UNLIMITED: u32 = 0x0700;
    pub const RBLN_8K: u32 = 0x0000;
    pub const RBLN_16K: u32 = 0x0800;
    pub const RBLN_32K: u32 = 0x1000;
    pub const RBLN_64K: u32 = 0x1800;
    pub const FTH_NONE: u32 = 0xE000;
}

mod msr {
    pub const LINKB: u8 = 0x02;
    pub const RX_FLOW_CONTROL_ENABLE: u8 = 0x40;
}

mod bmcr {
    pub const SPEED: u16 = 0x2000;
    pub const AUTO_NEGOTIATE: u16 = 0x1000;
    pub const DUPLEX: u16 = 0x0100;
}

const PACKET_SIZE_MAX: usize = 0x600;
const PACKET_SIZE_MIN: usize = 0x16;

const RX_BUFFER_SIZE: usize = 32768;
const TX_BUFFER_SIZE: usize = PACKET_SIZE_MAX as usize;

struct Buffers {
    rx: DMARegion,
    rx_offset: usize,
    tx: Vec<DMARegion>,
    tx_next: usize,
}

pub struct RTL8139 {
    pci_device: Device,
    pub irq: u8,
    io_base: u16,
    buffers: Buffers,
    link_up: bool,
}
impl RTL8139 {
    /// Initialize the driver.
    ///
    /// we add space to account for overhang from the last packet - the rtl8139
    /// can optionally guarantee that packets will be contiguous by
    /// purposefully overrunning the rx buffer
    /// https://github.com/SerenityOS/serenity/blob/64536f19f04976da00037e349ee665e7550fb2ca/Kernel/Net/RTL8139NetworkAdapter.cpp#L157
    pub unsafe fn new(pci_device: Device) -> Self {
        // RTL8139
        // XXX: RTL8029 bochs workaround
        // assert_eq!((pci_device.vendor, pci_device.id), (0x10ec, 0x8139));
        // Ethernet controller
        assert_eq!((pci_device.class.0, pci_device.class.1), (2, 0));

        pci_device.enable_bus_mastering();

        let buffers = Buffers {
            rx: DMARegion::allocate(RX_BUFFER_SIZE + PACKET_SIZE_MAX),
            tx: (0..TX_BUFFER_COUNT)
                .map(|_| DMARegion::allocate(TX_BUFFER_SIZE))
                .collect::<Vec<_>>(),
            tx_next: 0,
            rx_offset: 0,
        };

        let mut device = RTL8139 {
            pci_device,
            irq: pci_device
                .get_interrupt_line()
                .expect("Missing interrupt line"),
            io_base: (pci_device.get_bar(0) & (!1u32)) as u16,
            buffers,
            link_up: false,
        };
        device.reset();
        device
    }

    fn reset(&mut self) {
        println!("Resetting RTL8139");

        self.buffers.rx_offset = 0;
        self.buffers.tx_next = 0;

        unsafe {
            let mut r_bmcr: UnsafePort<u16> = UnsafePort::new(self.io_base + reg::BMCR);
            let mut r_cfg9346: UnsafePort<u8> = UnsafePort::new(self.io_base + reg::CFG9346);
            let mut r_command: UnsafePort<u8> = UnsafePort::new(self.io_base + reg::COMMAND);
            let mut r_config1: UnsafePort<u8> = UnsafePort::new(self.io_base + reg::CONFIG1);
            let mut r_imr: UnsafePort<u16> = UnsafePort::new(self.io_base + reg::IMR);
            let mut r_isr: UnsafePort<u16> = UnsafePort::new(self.io_base + reg::ISR);
            let mut r_mar0: UnsafePort<u32> = UnsafePort::new(self.io_base + reg::MAR0);
            let mut r_mar4: UnsafePort<u32> = UnsafePort::new(self.io_base + reg::MAR4);
            let mut r_mpc: UnsafePort<u8> = UnsafePort::new(self.io_base + reg::MPC);
            let mut r_msr: UnsafePort<u8> = UnsafePort::new(self.io_base + reg::MSR);
            let mut r_rxbuf: UnsafePort<u32> = UnsafePort::new(self.io_base + reg::RXBUF);
            let mut r_rxcfg: UnsafePort<u32> = UnsafePort::new(self.io_base + reg::RXCFG);
            let mut r_txcfg: UnsafePort<u32> = UnsafePort::new(self.io_base + reg::TXCFG);

            // Reset the device to clear out all the buffers and config
            r_command.write(command::RESET);
            while r_command.read() & command::RESET != 0 {}

            // Unlock config registers
            r_cfg9346.write(cfg9346::EEM0 | cfg9346::EEM1);

            // Wake up if the device is sleeping
            // https://wiki.osdev.org/RTL8139#Turning_on_the_RTL8139
            r_config1.write(0);

            // Turn on multicast
            r_mar0.write(0xffffffff);
            r_mar4.write(0xffffffff);
            // FIXME: for some reason the line above, specifically mar4, gives
            // the following error on Qemu:
            // > Misaligned i/o to address 00000012 with size 4 for memory region rtl8139
            // For some reason, I wan't able to find this text from Qemu source.
            // Other drivers seem to be doing this as well, for instance Linux and u-boot
            // https://github.com/TritonDataCenter/syslinux/blob/master/gpxe/src/drivers/net/rtl8139.c#L346
            // https://elixir.bootlin.com/u-boot/latest/source/drivers/net/rtl8139.c#L299

            // Enable rx/tx
            r_command.write(command::RX_ENABLE | command::TX_ENABLE);

            // Set up rx buffer
            r_rxbuf.write(self.buffers.rx.phys.as_u64() as u32);

            // Reset missed packet counter
            r_mpc.write(0);

            // Options - 100mbit, full duplex, auto negotiation
            r_bmcr.write(bmcr::SPEED | bmcr::AUTO_NEGOTIATE | bmcr::DUPLEX);

            // Enable flow control
            r_msr.write(msr::RX_FLOW_CONTROL_ENABLE);

            // Configure rx: accept physical (MAC) match, multicast, and broadcast,
            // use the optional contiguous packet feature, the maximum dma transfer
            // size, a 32k buffer, and no fifo threshold
            r_rxcfg.write(
                rx_cfg::APM
                    | rx_cfg::AM
                    | rx_cfg::AB
                    | rx_cfg::WRAP_INHIBIT
                    | rx_cfg::MAX_DMA_UNLIMITED
                    | rx_cfg::RBLN_32K
                    | rx_cfg::FTH_NONE,
            );

            // Configure tx: default retry count (16), max DMA burst size of 1024
            // bytes, interframe gap time of the only allowable value. the DMA burst
            // size is important - silent failures have been observed with 2048 bytes.
            r_txcfg.write(tx_cfg::TXRR_ZERO | tx_cfg::MAX_DMA_1K | tx_cfg::IFG11);

            // Tell the chip where we want it to DMA from for outgoing packets.
            for (i, region) in self.buffers.tx.iter().enumerate() {
                let mut r_txaddr0 =
                    UnsafePort::<u32>::new(self.io_base + reg::TXADDR0 + (i as u16 * 4));
                r_txaddr0.write(region.phys.as_u64() as u32);
            }

            // Re-lock config registers
            r_cfg9346.write(cfg9346::NONE);

            // Enable rx/tx again in case they got turned off
            r_command.write(command::RX_ENABLE | command::TX_ENABLE);

            // Choose irqs, then clear any pending
            r_imr.write(IntFlags::ALL_SUPPORTED.bits());
            r_isr.write(0xffff);
        }

        println!("Reset of RTL8139 complete");
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        unsafe {
            let rx_ptr: *const u8 = self.buffers.rx.virt.as_ptr();
            let packet_start = rx_ptr.add(self.buffers.rx_offset);

            let status_bits = *(packet_start as *const u16);
            let length = *(packet_start.add(2) as *const u16);

            let status = RxStatus::from_bits_truncate(status_bits);

            log::debug!("receive status: {:?}", status);

            if status.is_empty() {
                log::warn!("receive status empty");
                return None;
            }

            if !status.contains(RxStatus::OK) {
                log::warn!("receive status not ok");
                todo!("reset?");
                // return None;
            }

            if (length as usize) < PACKET_SIZE_MIN || (length as usize) >= PACKET_SIZE_MAX {
                log::warn!("receive invalid packet length");
                todo!("reset?");
                // return None;
            }

            if status.intersects(RxStatus::ALL_ERRORS) {
                log::warn!("receive status error");
                todo!("reset?");
                // return None;
            }

            // We never have to worry about the packet wrapping around the buffer,
            // since we set RXCFG_WRAP_INHIBIT, which allows the rtl8139 to write data
            // past the end of the alloted space.
            // https://github.com/SerenityOS/serenity/blob/6545a74743ebf7b508dc517a73a647167d3b94f2/Kernel/Net/RTL8139NetworkAdapter.cpp#L356
            let packet_len = (length - 4) as usize;
            let mut packet_buffer = Vec::with_capacity(packet_len);
            packet_buffer.set_len(packet_len);
            core::ptr::copy(packet_start.add(4), packet_buffer.as_mut_ptr(), packet_len);

            // Inform card that this packed has been read
            let nic_rx_offset =
                (((self.buffers.rx_offset as u16) + length + 4 + 3) & !3) % (RX_BUFFER_SIZE as u16);
            let mut r_capr: UnsafePort<u16> = UnsafePort::new(self.io_base + reg::CAPR);
            r_capr.write((self.buffers.rx_offset - 0x10) as u16);
            self.buffers.rx_offset = (nic_rx_offset % (RX_BUFFER_SIZE as u16)) as usize;

            // Return packet
            Some(packet_buffer)
        }
    }

    /// Select next tx buffer, distributing packets evently
    fn select_buffer(&self) -> Option<usize> {
        for i in 0..TX_BUFFER_COUNT {
            let potential_buffer = (self.buffers.tx_next + i) % 4;

            let status = unsafe {
                let mut r_txstatus0 = UnsafePort::<u32>::new(
                    self.io_base + reg::TXSTATUS0 + (potential_buffer as u16 * 4),
                );
                r_txstatus0.read()
            };
            if (status & tx_status::OWN) != 0 {
                return Some(potential_buffer);
            }
        }

        None
    }

    pub fn send(&mut self, packet: &[u8]) {
        log::debug!(" send [length={}]", packet.len());

        if packet.len() > PACKET_SIZE_MAX as usize {
            panic!("RTL8139: packet too large");
        }

        let buffer_index = self
            .select_buffer()
            .expect("RTL8139: hardware send buffers are full");

        log::debug!(" Buffer {} selected", buffer_index);
        self.buffers.tx_next = (buffer_index + 1) % 4;

        let src = packet.as_ptr();
        let dst = self.buffers.tx[buffer_index].virt.as_mut_ptr();
        unsafe {
            copy_nonoverlapping(src, dst, packet.len());
            write_bytes(dst.add(packet.len()), 0, TX_BUFFER_SIZE - packet.len());

            // the rtl8139 will not actually emit packets onto the network if they're
            // smaller than 64 bytes. the rtl8139 adds a checksum to the end of each
            // packet, and that checksum is four bytes long, so we pad the packet to
            // 60 bytes if necessary to make sure the whole thing is large enough.
            // https://github.com/SerenityOS/serenity/blob/31505dde7ec5e8077a5a72e7e50d4e5d7203432d/Kernel/Net/RTL8139NetworkAdapter.cpp#L325
            let mut r_txstatus0 =
                UnsafePort::<u32>::new(self.io_base + reg::TXSTATUS0 + (buffer_index as u16 * 4));
            r_txstatus0.write(packet.len().max(60) as u32);
        }
    }

    pub fn notify_irq(&mut self) -> Vec<Vec<u8>> {
        let mut received_packets = Vec::new();

        let mut r_isr: UnsafePort<u16> = unsafe { UnsafePort::new(self.io_base + reg::ISR) };
        let mut status = IntFlags::from_bits_truncate(unsafe { r_isr.read() });

        loop {
            // No known flags on
            if !status.intersects(IntFlags::ALL_SUPPORTED) {
                break;
            }

            log::info!("IRQ status={:?}", status);

            if status.contains(IntFlags::RXOK) {
                log::info!("rx ready");
                if let Some(packet) = self.receive() {
                    received_packets.push(packet);
                }
            }

            if status.contains(IntFlags::RXERR) {
                log::warn!("rx error");
                // TODO: reset
                // self.reset();
                todo!()
            }

            if status.contains(IntFlags::TXOK) {
                log::info!("tx complete");
            }

            if status.contains(IntFlags::TXERR) {
                log::warn!("tx error");
                // TODO: reset
                // self.reset();
                todo!()
            }

            if status.contains(IntFlags::RX_BUFFER_OVERFLOW) {
                log::warn!("rx buffer overflow");
            }

            if status.contains(IntFlags::LINK_CHANGE) {
                log::warn!("link status changed");
                todo!("HANDLE THIS")
                // self.link_up = (in8(REG_MSR) & MSR_LINKB) == 0;
            }

            if status.contains(IntFlags::RX_FIFO_OVERFLOW) {
                log::info!("rx fifo overflow");
            }

            if status.contains(IntFlags::LENGTH_CHANGE) {
                log::info!("cable length change");
            }

            if status.contains(IntFlags::SYSTEM_ERROR) {
                log::warn!("system error");
                // TODO: reset
                // self.reset();
                todo!()
            }

            // Clear interrupt
            // https://wiki.osdev.org/RTL8139#ISR_Handler
            status = IntFlags::from_bits_truncate(unsafe {
                let value = r_isr.read();
                r_isr.write(value);
                value
            });
            println!("V = {:?}", status);
        }

        received_packets
    }

    pub fn mac_addr(&self) -> MacAddr {
        let mut result = [0; 6];
        for i in 0..6 {
            unsafe {
                let mut port = UnsafePort::<u8>::new(self.io_base + reg::MAC + i);
                result[i as usize] = port.read();
            }
        }
        MacAddr(result)
    }
}
