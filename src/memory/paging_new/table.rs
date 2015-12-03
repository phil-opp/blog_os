use super::{ENTRY_COUNT, Page};
use super::entry::{Entry, PRESENT, HUGE_PAGE};
use super::levels::{TableLevel, HierachicalLevel, Level4};
use core::ops::{Index, IndexMut};
use core::marker::PhantomData;

pub const P4: *const Table<Level4> = 0xffffffff_fffff000 as *const _;

pub struct Table<L: TableLevel> {
    entries: [Entry; ENTRY_COUNT],
    _phantom: PhantomData<L>,
}

impl<L> Index<usize> for Table<L> where L: TableLevel
{
    type Output = Entry;

    fn index(&self, index: usize) -> &Entry {
        &self.entries[index]
    }
}

impl<L> IndexMut<usize> for Table<L> where L: TableLevel
{
    fn index_mut(&mut self, index: usize) -> &mut Entry {
        &mut self.entries[index]
    }
}

impl<L> Table<L> where L: TableLevel
{
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }
}

impl<L> Table<L> where L: HierachicalLevel
{
    pub fn next_table(&self, index: usize) -> Option<&Table<L::NextLevel>> {
        self.next_table_address(index).map(|t| unsafe { &*(t as *const _) })
    }

    pub fn next_table_mut(&mut self, index: usize) -> Option<&mut Table<L::NextLevel>> {
        self.next_table_address(index).map(|t| unsafe { &mut *(t as *mut _) })
    }

    fn next_table_address(&self, index: usize) -> Option<usize> {
        let entry_flags = self[index].flags();
        if entry_flags.contains(PRESENT) && !entry_flags.contains(HUGE_PAGE) {
            let table_page = Page::containing_address(self as *const _ as usize);
            assert!(table_page.number >= 0o_777_000_000_000);
            let next_table_page = Page {
                number: ((table_page.number << 9) & 0o_777_777_777_777) | index,
            };
            Some(next_table_page.start_address())
        } else {
            None
        }
    }
}
