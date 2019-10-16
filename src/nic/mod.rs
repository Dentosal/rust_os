use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

mod ne2000;
mod virtio;

pub trait NIC: Send {
    /// Returns success status
    fn init(&mut self) -> bool;

    fn send(&mut self, packet: Vec<u8>);

    fn mac_addr(&self) -> [u8; 6];

    fn mac_addr_string(&self) -> String {
        let mut result = String::new();
        let mac = self.mac_addr();
        for i in 0..6 {
            result.push_str(&format!("{:02x}", mac[i]));
            if i != 5 {
                result.push(':');
            }
        }
        result
    }
}

pub struct NetworkController {
    pub driver: Option<Box<dyn NIC>>,
}
impl NetworkController {
    pub const fn new() -> NetworkController {
        NetworkController { driver: None }
    }

    pub unsafe fn init(&mut self) {
        rprintln!("Selecting NIC driver...");

        self.driver = virtio::VirtioNet::try_new();
        if self.driver.is_some() {
            rprintln!("Using VirtIO Networking");
        } else {
            self.driver = ne2000::Ne2000::try_new();
            if self.driver.is_some() {
                rprintln!("Using Ne2000");
            } else {
                rprintln!("Not suitable NIC driver found");
            }
        }

        if self.driver.is_some() {
            let ok = if let Some(ref mut driver) = self.driver {
                driver.init()
            } else {
                unreachable!()
            };

            if !ok {
                rprintln!("NIC driver initialization failed");
                self.driver = None;
            }
        }
    }

    pub unsafe fn map<T>(&mut self, f: &mut dyn FnMut(&mut Box<dyn NIC>) -> T) -> Option<T> {
        if let Some(ref mut driver) = self.driver {
            Some(f(driver))
        } else {
            None
        }
    }
}

// Create static pointer mutex with spinlock to make networking thread-safe
pub static NETWORK: Mutex<NetworkController> = Mutex::new(NetworkController::new());

pub fn init() {
    unsafe {
        let mut nw = NETWORK.lock();
        nw.init();
        nw.map(&mut |drv| {
            rprintln!("MAC ADDR: {}", drv.mac_addr_string());

            let mac_addr = drv.mac_addr();

            // drv.send(vec![
            //     // Hand crafted ARP Broadcast packet
            //     // Remember: network byte order

            //     // Ethernet header

            //     // ARP header
            //     // https://en.wikipedia.org/wiki/Address_Resolution_Protocol#Packet_structure
            //     (1u16).to_be(),     // Hardware type: Ethernet
            //     (0x800u16).to_be(), // Protocol type: IPv4
            //     6,                  // Hardware address length: 6 for ethernet address
            //     4,                  // Protocol address length: 4 for IPv4
            //     (1u16).to_be(),     // Operation: request
            //     // Sender MAC address
            //     mac_addr[0],
            //     mac_addr[1],
            //     mac_addr[2],
            //     mac_addr[3],
            //     mac_addr[4],
            //     mac_addr[5],
            //     // Target MAC address (zero for request)
            //     0,
            //     0,
            //     0,
            //     0,
            //     0,
            //     0,
            // ]);

            rprintln!("SENT PACKET");
        });
    }
}
