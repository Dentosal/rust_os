pub const MEM_PAGE_SIZE_BYTES:      usize   = 0x1000; // 4096
pub const MEM_PAGE_MAP_SIZE_BYTES:  usize   = 0x10000;
pub const MEM_PAGE_MAP1_ADDRESS:    usize   = 0x30000;
pub const MEM_PAGE_MAP2_ADDRESS:    usize   = 0x40000;

// Memory frame (single allocation unit)
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    index: usize
}
impl Frame {
    // Create new Frame from memory address. Rounds down.
    fn from_address(address: usize) -> Frame {
        Frame {index: address / MEM_PAGE_SIZE_BYTES}
    }
    // Create new Frame from memory map index.
    fn from_index(index: usize) -> Frame {
        Frame {index: index}
    }
}

// Frame allocators all work this way
pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self, frame: Frame);
}

// A simple first-fit frame allocator
// Currently we can only get one frame at a time
pub struct BitmapAllocator;
impl BitmapAllocator {
    pub fn new() -> BitmapAllocator {
        BitmapAllocator {}
    }
    fn is_free(&self, index: usize) -> bool {
        let free    = unsafe { *((MEM_PAGE_MAP1_ADDRESS + index/8) as *mut u8) } & (1 << (index%8)) != 0; // 1: free, 0: reserved
        let usable  = unsafe { *((MEM_PAGE_MAP2_ADDRESS + index/8) as *mut u8) } & (1 << (index%8)) == 0; // 1: unusable, 0: usable
        if free && !usable { // error in free pages table
            panic!("PHYS_MEM: Page {} is incorrectly marked as free.", index);
        }
        free
    }
}
impl FrameAllocator for BitmapAllocator {
    fn allocate_frame(&mut self) -> Option<Frame> {
        // Find first free block
        for i in 0..(MEM_PAGE_MAP_SIZE_BYTES*8) {
            if self.is_free(i) {
                return Some(Frame::from_index(i));
            }
        }
        // We could not find any free memory block
        None
    }
    fn deallocate_frame(&mut self, frame: Frame) {
        if self.is_free(frame.index) {
            panic!("PHYS_MEM: deallocate_frame: Page already free {}.", frame.index);
        }
        unsafe {
            *((MEM_PAGE_MAP1_ADDRESS + frame.index/8) as *mut u8) |= 1 << (frame.index%8); // set the correct bit
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
    // load memory map from 0x1000-2, where out bootloader left it
    // http://wiki.osdev.org/Detecting_Memory_(x86)#BIOS_Function:_INT_0x15.2C_EAX_.3D_0xE820


    // zero out the bitmap sections
    for address in (MEM_PAGE_MAP1_ADDRESS..MEM_PAGE_MAP2_ADDRESS+MEM_PAGE_SIZE_BYTES).step_by(8) {
        unsafe {
            *(address as *mut u8) = 0; // default to: reserved, unusable
        }
    }

    let entry_count: u8 = unsafe {*((0x1000-2) as *mut u8)};
    let base = 0x1000 as *mut u8;
    let mut memory_amount_counter_KiB = 0;
    for index in 0..(entry_count as isize) {
        let entry_start:    usize   = unsafe { *(base.offset(24*index+ 0) as *mut u64) } as usize;
        let entry_size:     usize   = unsafe { *(base.offset(24*index+ 8) as *mut u64) } as usize;
        let entry_type:     u32     = unsafe { *(base.offset(24*index+16) as *mut u32) };
        let acpi_data:      u32     = unsafe { *(base.offset(24*index+20) as *mut u32) };
        //rprintln!("Section {}: {:#016x}-{:#016x}: type: {:#x}, acpi: {:#x}", index, entry_start, entry_start+entry_size, entry_type, acpi_data);

        // is this usable?
        // Types 1, 4 ok to use and acpi_data bit 0 must be set
        if (entry_type == 1 || entry_type == 4) && (acpi_data & 1) == 1 {
            // set frame data. accept only full frames
            for address in (mt_align_address(entry_start, true)..mt_align_address(entry_start+entry_size, false)).step_by(MEM_PAGE_SIZE_BYTES) {
                memory_amount_counter_KiB += 1;
                if address/MEM_PAGE_SIZE_BYTES > MEM_PAGE_MAP_SIZE_BYTES*8 {
                    // Page table is full.
                    break;
                }
                unsafe {
                    *((MEM_PAGE_MAP1_ADDRESS + (address/8)/MEM_PAGE_SIZE_BYTES) as *mut u8) |= 1 << (address%8); // set free
                    *((MEM_PAGE_MAP2_ADDRESS + (address/8)/MEM_PAGE_SIZE_BYTES) as *mut u8) |= 1 << (address%8); // set usable
                }
            }
        }
    }
    rprintln!("Memory size {} MiB", memory_amount_counter_KiB/1024);
}
