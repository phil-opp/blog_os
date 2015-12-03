use core::marker::PhantomData;

pub const fn P4(page: &Page) -> Table<Level4> {
    Table {
        table_page: Page { number: 0o_777_777_777_777 },
        target_page_number: page.number,
        _phantom: PhantomData,
    }
}


pub fn translate(virtual_address: usize) -> Option<PhysicalAddress> {
    let page = Page::containing_address(virtual_address);
    let offset = virtual_address % PAGE_SIZE;

    let frame_number = {
        let p3 = match P4(&page).next_table() {
            None => return None,
            Some(t) => t,
        };

        if p3.entry().flags().contains(PRESENT | HUGE_PAGE) {
            // 1GiB page (address must be 1GiB aligned)
            let start_frame_number = p3.entry().pointed_frame().number;
            assert!(start_frame_number % (ENTRY_COUNT * ENTRY_COUNT) == 0);
            start_frame_number + Table::<Level2>::index_of(&page) * ENTRY_COUNT +
            Table::<Level1>::index_of(&page)
        } else {
            // 2MiB or 4KiB page
            let p2 = match p3.next_table() {
                None => return None,
                Some(t) => t,
            };

            if p2.entry().flags().contains(PRESENT | HUGE_PAGE) {
                // 2MiB page (address must be 2MiB aligned)
                let start_frame_number = p2.entry().pointed_frame().number;
                assert!(start_frame_number % ENTRY_COUNT == 0);
                start_frame_number + Table::<Level2>::index_of(&page)
            } else {
                // standard 4KiB page
                let p1 = match p2.next_table() {
                    None => return None,
                    Some(t) => t,
                };
                p1.entry().pointed_frame().number
            }
        }
    };
    Some(frame_number * PAGE_SIZE + offset)
}


pub fn map_to<A>(page: &Page, frame: Frame, flags: TableEntryFlags, allocator: &mut A)
    where A: FrameAllocator
{
    let mut p3 = P4(page).next_table_create(allocator);
    let mut p2 = p3.next_table_create(allocator);
    let mut p1 = p2.next_table_create(allocator);

    assert!(!p1.entry().flags().contains(PRESENT));
    p1.set_entry(TableEntry::new(frame, flags));
}

trait TableLevel{
    fn level_number() -> usize;
}
pub enum Level1 {}
pub enum Level2 {}
pub enum Level3 {}
pub enum Level4 {}

impl TableLevel for Level4 {
    fn level_number() -> usize {
        4
    }
}
impl TableLevel for Level3 {
    fn level_number() -> usize {
        3
    }
}
impl TableLevel for Level2 {
    fn level_number() -> usize {
        2
    }
}
impl TableLevel for Level1 {
    fn level_number() -> usize {
        1
    }
}

trait HierachicalLevel: TableLevel {
    type NextLevel: TableLevel;
}

impl HierachicalLevel for Level4 {
    type NextLevel = Level3;
}

impl HierachicalLevel for Level3 {
    type NextLevel = Level2;
}

impl HierachicalLevel for Level2 {
    type NextLevel = Level1;
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

pub const PAGE_SIZE: usize = 4096;
const ENTRY_SIZE: usize = 8;
const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;


pub struct Page {
    number: usize,
}

impl Page {
    fn containing_address(address: VirtualAddress) -> Page {
        match address {
            addr if addr < 0o_400_000_000_000_0000 => Page { number: addr / PAGE_SIZE },
            addr if addr >= 0o177777_400_000_000_000_0000 => {
                Page { number: (address / PAGE_SIZE) & 0o_777_777_777_777 }
            }
            _ => panic!("invalid address: 0x{:x}", address),
        }
    }

    pub fn start_address(&self) -> VirtualAddress {
        if self.number >= 0x800000000 {
            // sign extension necessary
            (self.number << 12) | 0xffff_000000000000
        } else {
            self.number << 12
        }
    }
}

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
