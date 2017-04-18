pub use self::entry::*;
pub use self::mapper::Mapper;
use core::ops::{Deref, DerefMut};
use core::ptr::Unique;
use memory::{PAGE_SIZE, Frame, FrameAllocator};
use self::table::{Table, Level4};
use self::temporary_page::TemporaryPage;

mod entry;
mod table;
mod temporary_page;
mod mapper;

const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;

#[derive(Debug, Clone, Copy)]
pub struct Page {
   number: usize,
}

impl Page {
    pub fn containing_address(address: VirtualAddress) -> Page {
        assert!(address < 0x0000_8000_0000_0000 ||
            address >= 0xffff_8000_0000_0000,
            "invalid address: 0x{:x}", address);
        Page { number: address / PAGE_SIZE }
    }

    fn start_address(&self) -> usize {
        self.number * PAGE_SIZE
    }

    fn p4_index(&self) -> usize {
        (self.number >> 27) & 0o777
    }
    fn p3_index(&self) -> usize {
        (self.number >> 18) & 0o777
    }
    fn p2_index(&self) -> usize {
        (self.number >> 9) & 0o777
    }
    fn p1_index(&self) -> usize {
        (self.number >> 0) & 0o777
    }
}

pub struct ActivePageTable {
    mapper: Mapper,
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
    unsafe fn new() -> ActivePageTable {
        ActivePageTable {
            mapper: Mapper::new(),
        }
    }

    pub fn with<F>(&mut self,
                   table: &mut InactivePageTable,
                   temporary_page: &mut temporary_page::TemporaryPage, // new
                   f: F)
        where F: FnOnce(&mut Mapper)
    {
        use x86_64::instructions::tlb;
        use x86_64::registers::control_regs;

        {
            let backup = Frame::containing_address(
                control_regs::cr3().0 as usize);

            // map temporary_page to current p4 table
            let p4_table = temporary_page.map_table_frame(backup.clone(), self);

            // overwrite recursive mapping
            self.p4_mut()[511].set(table.p4_frame.clone(), PRESENT | WRITABLE);
            tlb::flush_all();

            // execute f in the new context
            f(self);

            // restore recursive mapping to original p4 table
            p4_table[511].set(backup, PRESENT | WRITABLE);
            tlb::flush_all();
        }

        temporary_page.unmap(self);
    }

}

pub struct InactivePageTable {
    p4_frame: Frame,
}

impl InactivePageTable {
    pub fn new(frame: Frame,
               active_table: &mut ActivePageTable,
               temporary_page: &mut TemporaryPage)
               -> InactivePageTable {
        {
            let table = temporary_page.map_table_frame(frame.clone(),
                active_table);
            // now we are able to zero the table
            table.zero();
            // set up recursive mapping for the table
            table[511].set(frame.clone(), PRESENT | WRITABLE);
        }
        temporary_page.unmap(active_table);

        InactivePageTable { p4_frame: frame }
    }
}
