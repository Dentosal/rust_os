use core::ptr;

use paging::PhysicalAddress;

use d7alloc::{HEAP_START, HEAP_SIZE};

// This MUST be kept in sync with the one from src/asm_routines and plan.md
const BOOT_TMP_MMAP_BUFFER:     usize   = 0x2000;

pub const MEM_PAGE_SIZE_BYTES:      usize   = 0x1_000; // 4096
pub const MEM_PAGE_MAP_SIZE_BYTES:  usize   = 0x10_000;
pub const MEM_PAGE_MAP1_ADDRESS:    usize   = 0x70_000;
pub const MEM_PAGE_MAP2_ADDRESS:    usize   = 0x80_000;
pub const KERNEL_LOCATION:          usize   = 0x100_000;
pub const KERNEL_SIZE_LIMIT:        usize   = 0x200_000; // TODO: find a solution, or document properly
pub const MEMORY_RESERVED_BELOW:    usize   = KERNEL_LOCATION+KERNEL_SIZE_LIMIT;

// Memory frame (single allocation unit)
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    pub index: usize
}
impl Frame {
    /// Create new Frame from memory address. Rounds down.
    pub fn containing_address(address: usize) -> Frame {
        Frame {index: address / MEM_PAGE_SIZE_BYTES}
    }
    /// Create new Frame from memory map index.
    pub fn from_index(index: usize) -> Frame {
        Frame {index: index}
    }
    /// Start address of the frame
    pub fn start_address(&self) -> PhysicalAddress {
        self.index * MEM_PAGE_SIZE_BYTES
    }
    /// Clone frame
    pub fn clone(&self) -> Frame {
        Frame { index: self.index }
    }
    /// Inclusive range between frames
    pub fn range_inclusive(start: Frame, end: Frame) -> FrameIter {
        FrameIter {
            start: start,
            end: end,
        }
    }
}

/// Iterator for frames
pub struct FrameIter {
    start: Frame,
    end: Frame,
}
impl Iterator for FrameIter {
    type Item = Frame;

    fn next(&mut self) -> Option<Frame> {
        if self.start <= self.end {
            let frame = self.start.clone();
            self.start.index += 1;
            Some(frame)
        } else {
            None
        }
    }
}

// Frame allocators all work this way
pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self, frame: Frame);
}

// A simple first-fit frame allocator
// Currently we can only get one frame at a time
pub struct BitmapAllocator {
    index: usize
}

impl BitmapAllocator {
    pub const fn new() -> BitmapAllocator {
        BitmapAllocator {index: 0}
    }
    fn is_free(&self, index: usize) -> bool {
        let free    = unsafe {ptr::read_volatile((MEM_PAGE_MAP1_ADDRESS + index/8) as *mut u8)} & (1 << (index%8)) != 0; // 1: free, 0: reserved
        let usable  = unsafe {ptr::read_volatile((MEM_PAGE_MAP2_ADDRESS + index/8) as *mut u8)} & (1 << (index%8)) != 0; // 1: usable, 0: unusable

        if free && !usable { // error in free pages table
            panic!("PHYS_MEM: Page {} is incorrectly marked as free.", index);
        }
        free
    }
}
impl FrameAllocator for BitmapAllocator {
    fn allocate_frame(&mut self) -> Option<Frame> {
        // Find first free block
        for i in 0..(MEM_PAGE_MAP_SIZE_BYTES*8) { // 8 bits of information per byte
            if self.is_free(i) {
                unsafe {
                    // set frame reserved
                    ptr::write_volatile(
                        (MEM_PAGE_MAP1_ADDRESS + i/8) as *mut u8,
                        ptr::read_volatile((MEM_PAGE_MAP1_ADDRESS + i/8) as *mut u8) ^ (1 << (i%8))
                    );
                }
                return Some(Frame::from_index(i));
            }
        }
        // We could not find a free memory block
        None
    }
    fn deallocate_frame(&mut self, frame: Frame) {
        if self.is_free(frame.index) {
            panic!("PHYS_MEM: deallocate_frame: Page {} is already free.", frame.index);
        }
        unsafe {
            // set the correct bit
            ptr::write_volatile(
                (MEM_PAGE_MAP1_ADDRESS + frame.index/8) as *mut u8,
                ptr::read_volatile((MEM_PAGE_MAP1_ADDRESS + frame.index/8) as *mut u8) | (1 << (frame.index%8))
            );
        }
    }
}

fn mt_align_address(address: usize, upwards: bool) -> usize {
    if address % MEM_PAGE_SIZE_BYTES == 0 {
        address
    }
    else if upwards {
        address + MEM_PAGE_SIZE_BYTES - address % MEM_PAGE_SIZE_BYTES
    }
    else {
        address - address % MEM_PAGE_SIZE_BYTES
    }
}


pub fn create_memory_bitmap() {
    // load memory map from where out bootloader left it
    // http://wiki.osdev.org/Detecting_Memory_(x86)#BIOS_Function:_INT_0x15.2C_EAX_.3D_0xE820

    // zero out the bitmap sections
    for address in (MEM_PAGE_MAP1_ADDRESS..MEM_PAGE_MAP2_ADDRESS+MEM_PAGE_SIZE_BYTES).step_by(8) {
        unsafe {
            ptr::write_volatile(address as *mut u8, 0); // default to (reserved, unusable)
        }
    }

    let entry_count: u8 = unsafe {ptr::read_volatile(BOOT_TMP_MMAP_BUFFER as *mut u8)};
    let base = (BOOT_TMP_MMAP_BUFFER+2) as *mut u8;
    let mut memory_amount_counter_kib = 0;
    for index in 0..(entry_count as isize) {
        let entry_start:    usize   = unsafe { ptr::read_volatile(base.offset(24*index+ 0) as *mut u64) } as usize;
        let entry_size:     usize   = unsafe { ptr::read_volatile(base.offset(24*index+ 8) as *mut u64) } as usize;
        let entry_type:     u32     = unsafe { ptr::read_volatile(base.offset(24*index+16) as *mut u32) };
        let acpi_data:      u32     = unsafe { ptr::read_volatile(base.offset(24*index+20) as *mut u32) };
        // rprintln!("Section {}: {:#016x}-{:#016x}: type: {:#x}, acpi: {:#x}", index, entry_start, entry_start+entry_size, entry_type, acpi_data);

        // is this usable?
        // Types 1, 4 ok to use and acpi_data bit 0 must be set
        if (entry_type == 1 || entry_type == 4) && (acpi_data & 1) == 1 {
            // set frame data. accept only full frames
            for address in (mt_align_address(entry_start, true)..mt_align_address(entry_start+entry_size, false)).step_by(MEM_PAGE_SIZE_BYTES) {
                memory_amount_counter_kib += 1;
                if address/MEM_PAGE_SIZE_BYTES > MEM_PAGE_MAP_SIZE_BYTES*8 {
                    // Page table is full.
                    break;
                }
                if address < MEMORY_RESERVED_BELOW {
                    // these are permanently reserved for the kernel
                    continue;
                }
                if HEAP_START <= address && address < (HEAP_START+HEAP_SIZE) {
                    // these are permanently reserved for the heap
                    continue;
                }
                unsafe {
                    // set free
                    ptr::write_volatile(
                        (MEM_PAGE_MAP1_ADDRESS + (address/8)/MEM_PAGE_SIZE_BYTES) as *mut u8,
                        ptr::read_volatile((MEM_PAGE_MAP1_ADDRESS + (address/8)/MEM_PAGE_SIZE_BYTES) as *mut u8) | (1 << (address%8))
                    );
                    // set usable
                    ptr::write_volatile(
                        (MEM_PAGE_MAP2_ADDRESS + (address/8)/MEM_PAGE_SIZE_BYTES) as *mut u8,
                        ptr::read_volatile((MEM_PAGE_MAP2_ADDRESS + (address/8)/MEM_PAGE_SIZE_BYTES) as *mut u8) | (1 << (address%8))
                    );
                }
            }
        }
    }
    rprintln!("Memory size {} MiB", memory_amount_counter_kib/1024);
}

// Create static pointer mutex with spinlock to make ALLOCATOR thread-safe
// pub static ALLOCATOR: Mutex<BitmapAllocator> = Mutex::new(BitmapAllocator::new());
