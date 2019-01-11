// Ne2000 driver
// https://wiki.osdev.org/Ne2000
// https://github.com/spotify/linux/blob/master/drivers/net/ne2k-pci.c
// https://github.com/segabor/NE2K
// Register names, etc.: https://github.com/matijaspanic/NE2000/blob/master/NE2000.c

use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::string::String;
use cpuio::UnsafePort;

use super::NIC;
use pci;

#[derive(Clone)]
pub struct Ne2000 {
    pci_device: pci::Device,
    io_base: u16,
    mac_address: [u8; 6],
}
impl Ne2000 {
    fn find_pci_device() -> Option<pci::Device> {
        let mut pci_ctrl = pci::PCI.lock();
        pci_ctrl.find(|dev| {
            //	RTL-8029
            (dev.vendor, dev.id) == (0x10ec, 0x8029) &&
            // Ethernet controller
            dev.class.0 == 2 && dev.class.1 == 0
        })
    }

    pub fn try_new() -> Option<Box<NIC>> {
        let pci_device = Ne2000::find_pci_device()?;

        let io_base = (unsafe {pci_device.get_bar(0)} & !0x3) as u16;

        Some(box Ne2000 {
            pci_device,
            io_base,
            mac_address: [0; 6],
        })
    }

    unsafe fn reset_device(&self) {
        // https://wiki.osdev.org/Ne2000#Initialization_and_MAC_Address
        let mut status_register = UnsafePort::<u8>::new(self.io_base + 0x07);
        let mut reset_register = UnsafePort::<u8>::new(self.io_base + 0x1f);
        let value = reset_register.read();
        reset_register.write(value);
        while (status_register.read() & 0x80) == 0 {};
    }

    pub unsafe fn init_device(&mut self) {
        // https://wiki.osdev.org/Ne2000#Initialization_and_MAC_Address

        self.reset_device();
        let mut status_register = UnsafePort::<u8>::new(self.io_base + 0x07);
        status_register.write(0xff); // mask interrupts

        UnsafePort::<u8>::new(self.io_base + 0x00).write((1 << 5) | 1); // page 0, no DMA, stop
        UnsafePort::<u8>::new(self.io_base + 0x0E).write(0x49);         // set 16bit word access
        UnsafePort::<u8>::new(self.io_base + 0x0A).write(0);            // clear count register 1
        UnsafePort::<u8>::new(self.io_base + 0x0B).write(0);            // clear count register 2
        UnsafePort::<u8>::new(self.io_base + 0x0F).write(0);            // mask completion IRQ
        UnsafePort::<u8>::new(self.io_base + 0x07).write(0xFF);         // mask completion IRQ
        UnsafePort::<u8>::new(self.io_base + 0x0C).write(0x20);         // set monitor mode
        UnsafePort::<u8>::new(self.io_base + 0x0D).write(0x02);         // set loopback mode
        UnsafePort::<u8>::new(self.io_base + 0x0A).write(32);           // reading 32 bytes
        UnsafePort::<u8>::new(self.io_base + 0x0B).write(0);            // count high
        UnsafePort::<u8>::new(self.io_base + 0x08).write(0);            // start DMA at 0
        UnsafePort::<u8>::new(self.io_base + 0x09).write(0);            // start DMA high
        UnsafePort::<u8>::new(self.io_base + 0x00).write(0x0A);         // start read

        let mut p_rom_port = UnsafePort::<u8>::new(self.io_base + 0x10);
        let mut p_rom: [u8; 32] = [0; 32];
        for i in 0..32 {
            p_rom[i] = p_rom_port.read();
        }

        for i in 0..6  {
            self.mac_address[i] = p_rom[i];
        }

        // Set PAR0..PAR5 to listen to packets to our MAC address
        self.select_register_page(1);
        for i in 0..6 {
            UnsafePort::<u8>::new(self.io_base + 1 + i).write(self.mac_address[i as usize]);
        }
        self.select_register_page(0);

        rprintln!("INIT COMPLETE");
    }

    pub unsafe fn select_register_page(&mut self, page: u8) {
        assert!(page < 4);

        let mut ctrl_reg = UnsafePort::<u8>::new(self.io_base);
        let mut value = ctrl_reg.read();
        value &= 0x3f;
        value |= page << 6;
        ctrl_reg.write(value);
    }

    /// Trigger a transmit start
    unsafe fn trigger_send(&mut self, packet: Vec<u8>, start_page: u8) {
        let mut ctrl_reg = UnsafePort::<u8>::new(self.io_base);
        ctrl_reg.write(0x20);   //nodma, page0

        let data = ctrl_reg.read();

        UnsafePort::<u8>::new(self.io_base + 0x04).write(start_page);
        UnsafePort::<u8>::new(self.io_base + 0x06).write(packet.len() as u8);

        ctrl_reg.write(0x26); // No DMA, start transmit

    }

    pub unsafe fn send_packet(&mut self, packet: Vec<u8>) {
        // https://wiki.osdev.org/Ne2000#Sending_a_Packet
        // https://github.com/segabor/NE2K/blob/391ec05830ec8f616a60553d055f17b4242f05fe/NE2K/NE2K.lksproj/NE2K.m#L856

        let mut interrupt_reg = UnsafePort::<u8>::new(self.io_base + 0x0F);

        // Disable IRQ
        interrupt_reg.write(0);

        let send_length: usize = packet.len().min(60);

        // [self _blockOutput: pkt_len buffer: nb_map(pkt) start: tx_start_page];

        self.trigger_send(packet, 0x60);


        // Transmit timeout
        use time::sleep_ms;
        sleep_ms(2);

        // Re-enable IRQ
        interrupt_reg.write(0x3f);
    }
}
impl NIC for Ne2000 {
    fn init(&mut self) -> bool {
        unsafe { self.init_device() }
        true
    }

    fn send(&mut self, packet: Vec<u8>) {
        unsafe { self.send_packet(packet) }
    }

    fn mac_addr(&self) -> [u8; 6] {
        self.mac_address
    }
}
