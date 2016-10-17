mod table;

use mem_map::MEM_PAGE_SIZE_BYTES;
use mem_map::{Frame, FrameAllocator};

pub use self::table::ActivePageTable;

const ENTRY_COUNT: usize = 512;


pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;


#[derive(Debug, Clone, Copy)]
pub struct Page {
   index: usize,
}

impl Page {
    pub fn containing_address(address: VirtualAddress) -> Page {
        assert!(address < 0x0000_8000_0000_0000 || address >= 0xffff_8000_0000_0000, "invalid address: 0x{:x}", address);
        Page { index: address / MEM_PAGE_SIZE_BYTES }
    }

    pub fn start_address(&self) -> usize {
        self.index * MEM_PAGE_SIZE_BYTES
    }


    fn p4_index(&self) -> usize {
        (self.index >> 27) & 0o777
    }
    fn p3_index(&self) -> usize {
        (self.index >> 18) & 0o777
    }
    fn p2_index(&self) -> usize {
        (self.index >> 9) & 0o777
    }
    fn p1_index(&self) -> usize {
        (self.index >> 0) & 0o777
    }

    fn translate(&self) -> Option<Frame> {
        let p3 = unsafe { &*table::P4 }.next_table(self.p4_index());

        let huge_page = || {
            p3.and_then(|p3| {
                let p3_entry = &p3[self.p3_index()];
                // 1GiB self?
                if let Some(start_frame) = p3_entry.pointed_frame() {
                    if p3_entry.flags().contains(HUGE_PAGE) {
                        // address must be 1GiB aligned
                        assert!(start_frame.index % (ENTRY_COUNT * ENTRY_COUNT) == 0);
                        return Some(Frame {
                            index: start_frame.index + self.p2_index() * ENTRY_COUNT +
                                    self.p1_index(),
                        });
                    }
                }
                if let Some(p2) = p3.next_table(self.p3_index()) {
                    let p2_entry = &p2[self.p2_index()];
                    // 2MiB self?
                    if let Some(start_frame) = p2_entry.pointed_frame() {
                        if p2_entry.flags().contains(HUGE_PAGE) {
                            // address must be 2MiB aligned
                            assert!(start_frame.index % ENTRY_COUNT == 0);
                            return Some(Frame { index: start_frame.index + self.p1_index() });
                        }
                    }
                }
                None
            })
        };

        p3.and_then(|p3| p3.next_table(self.p3_index()))
          .and_then(|p2| p2.next_table(self.p2_index()))
          .and_then(|p1| p1[self.p1_index()].pointed_frame())
          .or_else(huge_page)
    }
}


pub struct Entry(u64);

bitflags! {
    flags EntryFlags: u64 {
        const PRESENT =         1 << 0,
        const WRITABLE =        1 << 1,
        const USER_ACCESSIBLE = 1 << 2,
        const WRITE_THROUGH =   1 << 3,
        const NO_CACHE =        1 << 4,
        const ACCESSED =        1 << 5,
        const DIRTY =           1 << 6,
        const HUGE_PAGE =       1 << 7,
        const GLOBAL =          1 << 8,
        const NO_EXECUTE =      1 << 63,
    }
}

impl Entry {
    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    pub fn set_unused(&mut self) {
        self.0 = 0;
    }

    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn pointed_frame(&self) -> Option<Frame> {
        if self.flags().contains(PRESENT) {
            Some(Frame::containing_address(self.0 as usize & 0x000fffff_fffff000))
        } else {
            None
        }
    }

    pub fn set(&mut self, frame: Frame, flags: EntryFlags) {
        assert!(frame.start_address() & !0x000fffff_fffff000 == 0);
        self.0 = (frame.start_address() as u64) | flags.bits();
    }
}


pub fn test_paging<A>(allocator: &mut A) where A: FrameAllocator {
    let page_table = unsafe { ActivePageTable::new() };

    // test it
    // address 0 is mapped
    rprintln!("Some = {:?}", page_table.translate(0));
     // second P1 entry
    rprintln!("Some = {:?}", page_table.translate(4096));
    // second P2 entry
    rprintln!("Some = {:?}", page_table.translate(512 * 4096));
    // 300th P2 entry
    rprintln!("Some = {:?}", page_table.translate(300 * 512 * 4096));
    // second P3 entry
    rprintln!("None = {:?}", page_table.translate(512 * 512 * 4096));
    // last mapped byte
    rprintln!("Some = {:?}", page_table.translate(512 * 512 * 4096 - 1));


}
