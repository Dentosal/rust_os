use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use spin::Mutex;

mod virtio;
mod ne2000;

pub trait NIC: Send {
    /// Returns success status
    fn init(&mut self) -> bool;

    fn send(&mut self, packet: Vec<u8>);


    fn mac_addr(&self) -> [u8; 6];

    fn mac_addr_string(&self) -> String {
        let mut result = String::new();
        let mac = self.mac_addr();
        for i in 0..6  {
            result.push_str(&format!("{:02x}", mac[i]));
            if i != 5 {
                result.push(':');
            }
        }
        result
    }
}


pub struct NetworkController {
    pub driver: Option<Box<NIC>>
}
impl NetworkController {
    pub const fn new() -> NetworkController {
        NetworkController {
            driver: None
        }
    }

    pub unsafe fn init(&mut self) {
        rprintln!("Selecting NIC driver...");

        self.driver = virtio::VirtioNet::try_new();
        if self.driver.is_some() {
            rprintln!("Using VirtIO Networking");
        }
        else {
            self.driver = ne2000::Ne2000::try_new();
            if self.driver.is_some() {
                rprintln!("Using Ne2000");
            }
            else {
                rprintln!("Not suitable NIC driver found");
            }
        }

        if self.driver.is_some() {
            let ok = if let Some(ref mut driver) = self.driver {
                driver.init()
            } else { unreachable!() };

            if !ok {
                rprintln!("NIC driver initialization failed");
                self.driver = None;
            }
        }
    }

    pub unsafe fn map<T>(&mut self, f: &mut FnMut(&mut Box<NIC>) -> T) -> Option<T> {
        if let Some(ref mut driver) = self.driver {
            Some(f(driver))
        }
        else {
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

            return;


            drv.send(vec![
                // Hand crafted IPv4 ICMP ping package
                // Remember: network byte order

                // IPv4 Header
                // https://en.wikipedia.org/wiki/IPv4#Header
                0b0010_1010, // IPV4, No Options
                0, // DSCP and ECN not used
                (32u8).to_be(), 0, // 20 bytes IPv4 header + 8 bytes ICMP header + 4 bytes of data
                0, 0, // Idenfication field, disabled
                0, 0, // Flags and fragment offset both zero
                (5u8).to_be(), // TTL: 5
                (1u8).to_be(), // Protocol: ICMP (from: https://tools.ietf.org/html/rfc790)
                0, 0, // Checksum automatically calculated by the NIC
                0, 0, 0, 0, // Source IP 0.0.0.0
                (1u8).to_be(), (1u8).to_be(), (1u8).to_be(), (1u8).to_be(), // Destination IP: 1.1.1.1

                // IPv4 Payload, ICMP Echo (ping)
                // https://en.wikipedia.org/wiki/Internet_Control_Message_Protocol#Datagram_structure
                (8u8).to_be(), 0, // (type, code) == (8, 0) => ICMP Echo request
                (8u8).to_be(), 0, // Checksum
                (0u8).to_be(), (0u8).to_be(), (0u8).to_be(), (0u8).to_be(), // Rest-of-header (filler data)
            ]);

            rprintln!("SENT PACKET");

            // drv.send(vec![
            //     // Hand crafted IPv4 udp package
            //     // Remember: network byte order

            //     // IPv4 Header
            //     // https://en.wikipedia.org/wiki/IPv4#Header
            //     0b0010_1010, // IPV4, No Options
            //     0, // DSCP and ECN not used
            //     (30u8).to_be(), 0, // 20 bytes IPv4 header + 8 bytes UDP header + 2 bytes of data
            //     0, 0, // Idenfication field, disabled
            //     0, 0, // Flags and fragment offset both zero
            //     (5u8).to_be(), // TTL: 5
            //     (17u8).to_be(), // Protocol: UDP (from: https://tools.ietf.org/html/rfc790)
            //     0, 0, // Checksum automatically calculated by the NIC
            //     0, 0, 0, 0, // Source IP 0.0.0.0
            //     (1u8).to_be(), 0, 0, (127u8).to_be(), // Destination IP: 127.0.0.1

            //     // IPv4 Payload, UDP packet
            //     // https://en.wikipedia.org/wiki/User_Datagram_Protocol#Packet_structure
            //     0, 0, 0, 0, // Source port (unused)
            //     0, 0, (0x90u8).to_be(), (0x1fu8).to_be(), // Target port (8080)
            //     (10u8).to_be(), // 8-byte header + UDP data lenght (2)
            //     0, 0, 0, 0, // UDP checksum (unused)

            //     // UDP payload
            //     (0x57u8).to_be(), (0x48u8).to_be(), // "HW"
            // ]);
        });
    }
}
