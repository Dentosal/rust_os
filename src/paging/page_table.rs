use core::ops::{Deref, DerefMut};

use mem_map::Frame;

use super::entry::*;
use super::page::*;
use super::mapper::Mapper;
use super::tlb;

pub struct ActivePageTable {
    mapper: Mapper
}

impl Deref for ActivePageTable {
    type Target = Mapper;
    fn deref(&self) -> &Mapper {
        &self.mapper
    }
}

impl DerefMut for ActivePageTable {
    fn deref_mut(&mut self) -> &mut Mapper {
        &mut self.mapper
    }
}

impl ActivePageTable {
    pub unsafe fn new() -> ActivePageTable {
        ActivePageTable {
            mapper: Mapper::new()
        }
    }

    pub fn with<F>(&mut self, table: &mut InactivePageTable, temporary_page: &mut TemporaryPage, f: F) where F: FnOnce(&mut Mapper) {
        {
            let backup = Frame::containing_address(unsafe { register!(cr3) } as usize);

            // map temporary_page to current p4 table
            let p4_table = temporary_page.map_table_frame(backup.clone(), self);

            // overwrite recursive mapping
            self.p4_mut()[511].set(table.p4_frame.clone(), PRESENT | WRITABLE);
            unsafe {tlb::flush_all();}

            // execute f in the new context
            f(self);

            // restore recursive mapping to original p4 table
            p4_table[511].set(backup, PRESENT | WRITABLE);
            unsafe {tlb::flush_all();}
        }

        temporary_page.unmap(self);
    }

    pub fn switch(&mut self, new_table: InactivePageTable) -> InactivePageTable {
        let old_table = InactivePageTable {p4_frame: Frame::containing_address(unsafe { register!(cr3) } as usize)};
        unsafe {
            register!(cr3, new_table.p4_frame.start_address());
        }
        old_table
    }
}

pub struct InactivePageTable {
    p4_frame: Frame,
}

impl InactivePageTable {
    pub fn new(frame: Frame, active_table: &mut ActivePageTable, temporary_page: &mut TemporaryPage) -> InactivePageTable {
        {
            let table = temporary_page.map_table_frame(frame.clone(), active_table);
            table.zero();
            table[511].set(frame.clone(), PRESENT | WRITABLE); // set up recursive mapping for the table
        }
        temporary_page.unmap(active_table);
        InactivePageTable { p4_frame: frame }
    }
}
