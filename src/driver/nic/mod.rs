use alloc::collections::VecDeque;
use alloc::prelude::v1::*;
use spin::Mutex;

use d7abi::fs::protocol::network::ReceivedPacket;
use d7time::Instant;

use crate::multitasking::{EventQueue, ExplicitEventId, WaitFor};
use crate::time::SYSCLOCK;

// mod ne2000;
// mod virtio;

mod rtl8139;

#[derive(Debug, Clone)]
pub struct Packet(Vec<u8>);

pub trait NIC: Send {
    /// Returns success status
    fn init(&mut self) -> bool;

    fn send(&mut self, packet: &[u8]);

    /// Notification about IRQ
    fn notify_irq(&mut self) -> Vec<Packet> {
        Vec::new()
    }

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

const EVENT_BUFFER_LIMIT: usize = 100;

pub struct NetworkController {
    /// The actual device driver
    pub driver: Option<Box<dyn NIC>>,
    /// Received network packets
    pub received_queue: EventQueue<ReceivedPacket>,
}
impl NetworkController {
    pub fn new() -> NetworkController {
        NetworkController {
            driver: None,
            received_queue: EventQueue::new("NIC", EVENT_BUFFER_LIMIT),
        }
    }

    pub unsafe fn init(&mut self) {
        log::debug!("Selecting NIC driver...");

        self.driver = rtl8139::RTL8139::try_new();
        if self.driver.is_some() {
            log::info!("Using RTL8139 Networking");
        } else {
            log::warn!("Not suitable NIC driver found");
        }

        // self.driver = virtio::VirtioNet::try_new();
        // if self.driver.is_some() {
        //     rprintln!("Using VirtIO Networking");
        // } else {
        //     self.driver = ne2000::Ne2000::try_new();
        //     if self.driver.is_some() {
        //         rprintln!("Using Ne2000");
        //     } else {
        //         rprintln!("No suitable NIC driver found");
        //     }
        // }

        if let Some(ref mut driver) = self.driver {
            let ok = driver.init();
            if !ok {
                panic!("NIC driver initialization failed");
            }
        }
    }

    fn on_receive_packet(&mut self, packet: ReceivedPacket) {
        self.received_queue.push(packet);
    }

    pub fn map<T>(&mut self, f: &mut dyn FnMut(&mut Box<dyn NIC>) -> T) -> Option<T> {
        if let Some(ref mut driver) = self.driver {
            Some(f(driver))
        } else {
            None
        }
    }
}

// Create static pointer mutex with spinlock to make networking thread-safe
lazy_static::lazy_static! {
    pub static ref NETWORK: Mutex<NetworkController> = Mutex::new(NetworkController::new());
}

/// A driver can make the interrupt handler to call this function,
/// and it will be forwarded to it
fn notify_irq() {
    // Collect timestamp as early as possible
    let timestamp = SYSCLOCK.now();

    let mut nw = NETWORK.try_lock().unwrap();
    if let Some(packets) = nw.map(&mut |nic| nic.notify_irq()) {
        for packet in packets {
            nw.on_receive_packet(ReceivedPacket {
                packet: packet.0,
                timestamp,
            });
        }
    }
}

pub fn init() {
    unsafe {
        let mut nw = NETWORK.lock();
        nw.init();
        nw.map(&mut |drv| {
            log::info!("MacAddr: {}", drv.mac_addr_string());
        });
    }
}
