//! Generic VirtIO 1.0 driver
//! https://wiki.osdev.org/Virtio (Uses legacy pre-1.0 spec, not nice)
//! http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html

use core::mem;
use core::ptr;
use cpuio::UnsafePort;
use volatile;
use x86_64::structures::paging::Mapper;

use alloc::prelude::v1::Vec;

use crate::driver::pci;
use crate::memory::prelude::PhysAddr;
use crate::memory::{self, MemoryController};

mod virtq;
pub use self::virtq::*;

/// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-2830006
const FEATURE_RING_EVENT_IDX: (usize, u32) = (0, 1 << 29);
const FEATURE_VIRTIO_1: (usize, u32) = (1, 1 << 0);

#[derive(Debug, Copy, Clone)]
pub struct FeatureBits {
    fields: [u32; 2],
}
impl FeatureBits {
    pub fn new(fields: [u32; 2]) -> FeatureBits {
        FeatureBits { fields }
    }

    pub fn fields(self) -> [u32; 2] {
        self.fields
    }

    pub fn disable(&mut self, pos: (usize, u32)) {
        self.fields[pos.0] &= !(1 << pos.1);
    }

    pub fn is_enabled(self, pos: (usize, u32)) -> bool {
        self.fields[pos.0] & !(1 << pos.1) != 0
    }
}

/// https://github.com/torvalds/linux/blob/6f0d349d922ba44e4348a17a78ea51b7135965b1/include/uapi/linux/virtio_pci.h#L117
/// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-740004
/// No vndr, next, len, nor padding fields, because they are already parsed away in the pci module
/// Extra field pci_cfg_addr notes the location in the pci confuguration space for additional lookups
#[derive(Debug, Copy, Clone)]
pub struct CapabilityInfo {
    pci_cfg_addr: u8,
    pub cfg_type: u8,
    pub bar: u8,
    pub offset: u32,
    pub length: u32,
}
impl CapabilityInfo {
    pub fn cfg_type(&self) -> CapabilityType {
        match self.cfg_type {
            1 => CapabilityType::Common,
            2 => CapabilityType::Notify,
            3 => CapabilityType::Isr,
            4 => CapabilityType::Device,
            5 => CapabilityType::Pci,
            _ => CapabilityType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CapabilityType {
    Unknown = 0,
    Common = 1,
    Notify = 2,
    Isr = 3,
    Device = 4,
    Pci = 5,
}

bitflags! {
    pub struct DeviceStatus: u8 {
        const ACKNOWLEDGE   = 0b00000001;
        const DRIVER        = 0b00000010;
        const DRIVER_OK     = 0b00000100;
        const FEATURES_OK   = 0b00001000;
        const DEVICE_ERROR  = 0b01000000;
        const DRIVER_ERROR  = 0b10000000;

        const RESET = 0;
        const STATE_LOADED = Self::ACKNOWLEDGE.bits | Self::DRIVER.bits;
        const STATE_READY  = Self::STATE_LOADED.bits | Self::DRIVER_OK.bits | Self::FEATURES_OK.bits;
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum DeviceType {
    NetworkCard = 1,
    BlockDevice = 2,
    Console = 3,
    EntropySource = 4,
    MemoryBallooning = 5,
    IOMemory = 6,
    RPMSG = 7,
    SCSIHost = 8,
    Transport9p = 9,
    Mac802_11wlan = 10,
}

/// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-740004
#[repr(u64)]
enum CommonCfg {
    // Whole VirtIO device
    DeviceFeatureSelect = 0x00, // u32 rw
    DeviceFeature = 0x04,       // u32 r-
    DriverFeatureSelect = 0x08, // u32 rw
    DriverFeature = 0x0c,       // u32 rw
    MSIxConfig = 0x10,          // u16 rw
    NumQueues = 0x12,           // u16 r-
    DeviceStatus = 0x14,        //  u8 rw
    ConfigGeneration = 0x15,    //  u8 r-

    // Current VirtQueue
    QueueSelect = 0x16,     // u16 rw
    QueueSize = 0x18,       // u16 rw,  power of 2 (or 0)
    QueueMSIxVector = 0x1a, // u16 rw
    QueueEnable = 0x1c,     // u16 rw
    QueueNotifyOff = 0x1e,  // u16 r-
    QueueDesc = 0x20,       // u64 rw
    QueueAvail = 0x28,      // u64 rw
    QueueUsed = 0x30,       // u64 rw
}

pub struct VirtioDevice {
    pci_dev: pci::Device,
    capabilities: Vec<CapabilityInfo>,
    /// (notify_off_multiplier, Vec< queue_notify_off >)
    notify_offsets: (u32, Vec<u16>),
}
impl VirtioDevice {
    fn find_pci_device(device_type: DeviceType) -> Option<pci::Device> {
        let mut pci_ctrl = pci::PCI.lock();
        pci_ctrl.find(|dev| {
            //	VirtIO
            dev.vendor == 0x1af4 && (0x1000 <= dev.id && dev.id <= 0x103f)
            // Requested device type
            && dev.subsystem_id() == device_type as u16
        })
    }

    /// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-740004
    fn pci_capabilities(pci_dev: pci::Device) -> Vec<CapabilityInfo> {
        pci_dev.read_capabilities(0x09, &|dev, addr| unsafe {
            CapabilityInfo {
                pci_cfg_addr: addr,
                cfg_type: dev.read_u8(addr + 3),
                bar: dev.read_u8(addr + 4),
                offset: dev.read(addr + 8),
                length: dev.read(addr + 12),
            }
        })
    }

    pub fn try_new(device_type: DeviceType) -> Option<VirtioDevice> {
        let pci_dev = VirtioDevice::find_pci_device(device_type)?;
        let capabilities = Self::pci_capabilities(pci_dev);

        let mut dev = VirtioDevice {
            pci_dev,
            capabilities,
            notify_offsets: (0, vec![]),
        };
        dev.mem_map_io_capability(CapabilityType::Common);
        dev.mem_map_io_capability(CapabilityType::Device);
        dev.mem_map_io_capability(CapabilityType::Notify);
        dev.mem_map_io_capability(CapabilityType::Isr);
        dev.reset();
        dev.init_notifications();
        Some(dev)
    }

    fn mem_map_io_capability(&mut self, ct: CapabilityType) {
        let start = self.capability_addr(ct);
        let size = self.capability_size(ct);

        memory::configure(|mem_ctrl: &mut MemoryController| {
            use crate::memory::PhysFrame;
            use x86_64::structures::paging::PageTableFlags as Flags;

            let f_start = PhysFrame::containing_address(start);
            let f_end = PhysFrame::containing_address(PhysAddr::new((start + size).as_u64() - 1));
            for frame in PhysFrame::range_inclusive(f_start, f_end) {
                // mem_ctrl.paging(|p| {
                //     p
                //     .identity_map(
                //         frame,
                //         Flags::WRITABLE | Flags::NO_EXECUTE,
                //         &mut mem_ctrl.frame_allocator,
                //     )
                //     .expect("Could not map configuration space")
                //     .flush();
                // })
            }
        });
    }

    fn capability(&self, cfg_type: CapabilityType) -> Option<CapabilityInfo> {
        self.capabilities
            .iter()
            .find(|ci| ci.cfg_type == (cfg_type as u8))
            .cloned()
    }

    fn capability_addr(&self, ct: CapabilityType) -> PhysAddr {
        let cap = self.capability(ct).expect("No config available");
        let bar_lo = unsafe { self.pci_dev.get_bar(cap.bar) };
        let bar_hi = unsafe { self.pci_dev.get_bar(cap.bar + 1) };

        // https://wiki.osdev.org/Pci#Base_Address_Registers
        assert!(bar_lo & 1 == 0, "Memory BAR required");
        assert!(bar_lo & 0b110 == 0b100, "64bit BAR required");

        let base_addr = (bar_lo & !0xfu32) as u64 | ((bar_hi as u64) << 32);

        PhysAddr::new(base_addr + (cap.offset as u64))
    }

    fn capability_size(&self, ct: CapabilityType) -> u64 {
        self.capability(ct).expect("No config available").length as u64
    }

    /// Common config address
    fn config_addr(&self, cc: CommonCfg) -> PhysAddr {
        self.capability_addr(CapabilityType::Common) + (cc as u64)
    }

    /// Read common config variable at index
    fn read_config<T>(&mut self, cc: CommonCfg) -> T {
        unsafe { ptr::read_volatile(self.config_addr(cc).as_u64() as *const T) }
    }

    /// Write common config variable at index
    fn write_config<T>(&mut self, cc: CommonCfg, value: T) {
        unsafe { ptr::write_volatile(self.config_addr(cc).as_u64() as *mut T, value) }
    }

    /// Device specific area config address
    fn dev_config_addr(&self, cc: u64) -> PhysAddr {
        self.capability_addr(CapabilityType::Device) + cc
    }

    /// Read device specific config variable at index
    pub fn read_dev_config<T>(&mut self, cc: u64) -> T {
        unsafe { ptr::read_volatile(self.dev_config_addr(cc).as_u64() as *const T) }
    }

    /// Write device specific config variable at index
    pub fn write_dev_config<T>(&mut self, cc: u64, value: T) {
        unsafe { ptr::write_volatile(self.dev_config_addr(cc).as_u64() as *mut T, value) }
    }

    pub fn get_status(&mut self) -> DeviceStatus {
        DeviceStatus::from_bits_truncate(self.read_config::<u8>(CommonCfg::DeviceStatus))
    }

    pub fn set_status(&mut self, status: DeviceStatus) {
        self.write_config::<u8>(CommonCfg::DeviceStatus, status.bits());
        rprintln!("VirtIO: Device status: {:?}", status);
    }

    pub fn reset(&mut self) {
        self.set_status(DeviceStatus::RESET);
    }

    /// Feature bits
    pub fn features(&mut self) -> FeatureBits {
        self.write_config::<u32>(CommonCfg::DeviceFeatureSelect, 0);
        let lo = self.read_config::<u32>(CommonCfg::DeviceFeature);

        self.write_config::<u32>(CommonCfg::DeviceFeatureSelect, 1);
        let hi = self.read_config::<u32>(CommonCfg::DeviceFeature);

        FeatureBits::new([lo, hi])
    }

    /// Set feature bits
    /// # Returns
    /// Was setting features successful
    pub fn set_features(&mut self, features: FeatureBits) -> bool {
        // Check global driver features
        if !features.is_enabled(FEATURE_RING_EVENT_IDX) || !features.is_enabled(FEATURE_VIRTIO_1) {
            rprintln!("VirtIO: Device does not support VirtIO 1.0, disabled.");
            return false;
        }

        for (i, &f) in features.fields().iter().enumerate() {
            self.write_config::<u32>(CommonCfg::DriverFeatureSelect, i as u32);
            self.write_config::<u32>(CommonCfg::DriverFeature, f);
        }

        // Test if the FEATURES_OK bit can be set
        self.set_status(DeviceStatus::STATE_READY | DeviceStatus::FEATURES_OK);

        if !self.get_status().contains(DeviceStatus::FEATURES_OK) {
            rprintln!("VirtIO feature negotiation failed.");
            return false;
        }
        true
    }

    /// Number of queues
    pub fn num_queues(&mut self) -> u8 {
        self.read_config::<u8>(CommonCfg::NumQueues)
    }

    /// This is queue_notify_off in spec
    pub fn queue_notify_offset(&mut self) -> u16 {
        self.read_config::<u16>(CommonCfg::QueueNotifyOff)
    }

    /// Select current queue for confuguration access
    pub fn select_queue(&mut self, index: u16) {
        self.write_config::<u16>(CommonCfg::QueueSelect, index)
    }

    /// Queue size (max element count) for selected queue
    pub fn queue_size(&mut self) -> u16 {
        self.read_config::<u16>(CommonCfg::QueueSize)
    }

    /// Set queue address fields
    pub fn set_queue_addr(&mut self, vq: &VirtQueue) {
        let base_addr = vq.addr() as u64;
        let (a, b, _) = vq.sizes();

        self.write_config::<u64>(CommonCfg::QueueDesc, base_addr);
        self.write_config::<u64>(CommonCfg::QueueAvail, base_addr + (a as u64));
        self.write_config::<u64>(CommonCfg::QueueUsed, base_addr + ((a + b) as u64));
    }

    /// Enable or disable a queue
    pub fn set_queue_enabled(&mut self, enabled: bool) {
        self.write_config::<u16>(CommonCfg::QueueEnable, enabled as u16);
    }

    /// Notify the device about the new changes in given queue
    /// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-930005 (see 4.1.5.2)
    /// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-800004
    pub fn notify(&mut self, queue_index: u16) {
        // TODO: are there separate queue and index variables?

        let notify_target = (self.capability_addr(CapabilityType::Notify)
            + self.notify_offsets.0 as u64 * self.notify_offsets.1[queue_index as usize] as u64);

        unsafe {
            ptr::write_volatile(notify_target.as_u64() as *mut u16, queue_index);
        }
    }

    pub fn init_notifications(&mut self) {
        // Ensure this is only called once
        assert!(self.notify_offsets.0 == 0);

        // VirtIO device must have at least one notification method (by spec)
        let cap = self
            .capability(CapabilityType::Notify)
            .expect("VirtIO: No notification capability");

        // Global notification offset
        self.notify_offsets.0 = unsafe { self.pci_dev.read(cap.pci_cfg_addr + 16) };

        for index in 0..16u16 {
            self.select_queue(index);
            if self.queue_size() == 0 {
                break;
            }
            let offset = self.queue_notify_offset();
            self.notify_offsets.1.push(offset);
        }
    }

    fn init_current_queue(&mut self) -> VirtQueue {
        let queue_size = self.queue_size();

        let mut vq = VirtQueue::new(queue_size);
        vq.zero();
        self.set_queue_addr(&vq);
        self.set_queue_enabled(true);
        vq
    }

    pub fn init_queues(&mut self) -> Vec<VirtQueue> {
        let mut result = Vec::new();
        for index in 0..16u16 {
            self.select_queue(index);
            if self.queue_size() == 0 {
                break;
            }
            result.push(self.init_current_queue());
        }
        result
    }
}
