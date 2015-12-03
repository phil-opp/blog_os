use core::marker::PhantomData;

pub const fn P4(page: &Page) -> Table<Level4> {
    Table {
        table_page: Page { number: 0o_777_777_777_777 },
        target_page_number: page.number,
        _phantom: PhantomData,
    }
}

impl<L> Table<L> where L: TableLevel
{
    pub fn index_of(page: &Page) -> usize {
        Self::index_of_page_number(page.number)
    }

    fn index_of_page_number(page_number: usize) -> usize {
        let s = (L::level_number() - 1) * 9;
        (page_number >> s) & 0o777
    }

    fn index(&self) -> usize {
        Self::index_of_page_number(self.target_page_number)
    }
}

use memory::{Frame, FrameAllocator};


pub struct Table<Level> {
    table_page: Page,
    target_page_number: usize,
    _phantom: PhantomData<Level>,
}

impl<L> Table<L> where L: TableLevel
{
    fn entry(&self) -> TableEntry {
        let entry_address = self.table_page.start_address() + self.index() * ENTRY_SIZE;
        unsafe { *(entry_address as *const _) }
    }

    fn set_entry(&mut self, value: TableEntry) {
        let entry_address = self.table_page.start_address() + self.index() * ENTRY_SIZE;
        unsafe { *(entry_address as *mut _) = value }
    }

    fn zero(&mut self) {
        let page = self.table_page.start_address() as *mut [TableEntry; ENTRY_COUNT];
        unsafe { *page = [TableEntry::unused(); ENTRY_COUNT] };
    }
}

impl<L> Table<L> where L: HierachicalLevel
{
    fn next_table_internal(&self) -> Table<L::NextLevel> {
        Table {
            table_page: Page {
                number: ((self.table_page.number << 9) & 0o_777_777_777_777) | self.index(),
            },
            target_page_number: self.target_page_number,
            _phantom: PhantomData,
        }
    }

    fn next_table(&self) -> Option<Table<L::NextLevel>> {
        if self.entry().flags().contains(PRESENT) {
            Some(self.next_table_internal())
        } else {
            None
        }
    }

    fn next_table_create<A>(&mut self, allocator: &mut A) -> Table<L::NextLevel>
        where A: FrameAllocator
    {
        match self.next_table() {
            Some(table) => table,
            None => {
                let frame = allocator.allocate_frame().expect("no frames available");
                self.set_entry(TableEntry::new(frame, PRESENT | WRITABLE));
                let mut next_table = self.next_table_internal();
                next_table.zero();
                next_table
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TableEntry(u64);

impl TableEntry {
    const fn unused() -> TableEntry {
        TableEntry(0)
    }

    fn new(frame: Frame, flags: TableEntryFlags) -> TableEntry {
        let frame_addr = (frame.number << 12) & 0x000fffff_fffff000;
        TableEntry((frame_addr as u64) | flags.bits())
    }

    fn flags(&self) -> TableEntryFlags {
        TableEntryFlags::from_bits_truncate(self.0)
    }

    fn pointed_frame(&self) -> Frame {
        Frame { number: ((self.0 & 0x000fffff_fffff000) >> 12) as usize }
    }
}

bitflags! {
    flags TableEntryFlags: u64 {
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
