use memory::paging::entry::*;
use memory::paging::ENTRY_COUNT;

pub struct Table {
    entries: [Entry; ENTRY_COUNT],
}
