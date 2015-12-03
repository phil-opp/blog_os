use core::marker::PhantomData;
use super::{VirtualAddress, Page, ENTRY_COUNT};
use super::table::{Entry, Table, PRESENT};
use super::levels::{TableLevel, HierachicalLevel, Level4, Level3, Level2, Level1};

pub fn P4_entry(address: VirtualAddress) -> EntryRef<Level4> {
    let p4_page = Page { number: 0o_777_777_777_777 };
    let p4 = p4_page.start_address() as *mut Table;
    EntryRef {
        target_address: address,
        table: p4,
        _phantom: PhantomData,
    }
}

pub struct EntryRef<Level> {
    target_address: VirtualAddress,
    table: *mut Table,
    _phantom: PhantomData<Level>,
}

impl<L> EntryRef<L> where L: HierachicalLevel
{
    pub fn next_level(&self) -> Option<EntryRef<L::NextLevel>> {
        if self.entry().flags().contains(PRESENT) {
            let next_table_page = {
                let table_page = Page::containing_address(self.table as usize);
                let index = table_index::<L>(self.target_address);
                Page { number: ((table_page.number << 9) & 0o_777_777_777_777) | index }
            };
            let next_table = next_table_page.start_address() as *mut Table;
            Some(EntryRef {
                target_address: self.target_address,
                table: next_table,
                _phantom: PhantomData,
            })
        } else {
            None
        }
    }

    fn entry(&self) -> &Entry {
        unsafe { &(*self.table).0[table_index::<L>(self.target_address)] }
    }
}

fn table_index<L>(address: VirtualAddress) -> usize
    where L: TableLevel
{
    let shift = 12 + (L::level_number() - 1) * 9;
    (address >> shift) & 0o777
}
