pub use self::table::Page;

use self::table::{map_to, unmap};
use memory::frame_allocator::{Frame, FrameAllocator};

pub const PAGE_SIZE: usize = 4096;

mod table;

/// The paging lock must be unique. It is required for all page table operations and thus
/// guarantees exclusive page table access.
pub struct Lock {
    _private: (),
}

impl Lock {
    /// Creates a new paging lock. It's unsafe because only one lock can exist at a
    /// time.
    pub unsafe fn new() -> Lock {
        Lock {
            _private: (),
        }
    }

    /// Uses the passed frame to create a new page table that becomes the _current table_.
    /// All subsequent page table operations will modify it (the _current_ table) and leave the
    /// _active_ table unchanged. To activate the current table and make it the active table, use
    /// the `activate_new_table` method.
    /// This method assumes that the passed frame is identity mapped and is thus unsafe.
    pub unsafe fn begin_new_table_on_identity_mapped_frame(&mut self, frame: Frame)
    {
        table::begin_new_on_identity_mapped_frame(self, frame)
    }

    /// Activates the _current_ table. If the current table is equal to the active table, nothing
    /// changes. However, if _current_ and _active_ table are different, a new table becomes active /// and becomes the table used by the CPU.
    pub fn activate_current_table(&mut self) {
        table::activate_current()
    }

    pub fn mapper<'a, A>(&'a mut self, allocator: &'a mut A) -> Mapper<'a, A>
        where A: FrameAllocator,
    {
        Mapper {
            lock: self,
            allocator: allocator,
        }
    }
}

pub struct Mapper<'a, A> where A: 'a {
    lock: &'a mut Lock,
    allocator: &'a mut A,
}

impl<'a, A> Mapper<'a, A> where A: FrameAllocator {
    pub fn map_to(&mut self, page: Page, frame: Frame, writable: bool, executable: bool) {
        map_to(self.lock, page, frame, writable, executable, self.allocator)
    }

    pub fn map(&mut self, page: Page, writable: bool, executable: bool) {
        let frame = self.allocator.allocate_frame(&mut self.lock)
            .expect("no more frames available");
        self.map_to(page, frame, writable, executable)
    }

    pub fn unmap(&mut self, page: Page) {
        unmap(self.lock, page, self.allocator)
    }

    pub unsafe fn identity_map(&mut self, page: Page, writable: bool, executable: bool) {
        let frame = Frame {number: page.number};
        self.map_to(page, frame, writable, executable)
    }

}
