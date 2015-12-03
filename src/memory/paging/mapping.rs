use memory::Frame;
use super::Page;
use super::entry::{EntryFlags, PRESENT};
use memory::FrameAllocator;
use super::table::P4;

pub fn map<A>(page: &Page, flags: EntryFlags, allocator: &mut A)
    where A: FrameAllocator
{
    let frame = allocator.allocate_frame().expect("out of memory");
    map_to(page, frame, flags, allocator)
}

pub fn map_to<A>(page: &Page, frame: Frame, flags: EntryFlags, allocator: &mut A)
    where A: FrameAllocator
{
    let p4 = unsafe { &mut *P4 };
    let mut p3 = p4.next_table_create(page.p4_index(), allocator);
    let mut p2 = p3.next_table_create(page.p3_index(), allocator);
    let mut p1 = p2.next_table_create(page.p2_index(), allocator);

    assert!(!p1[page.p1_index()].flags().contains(PRESENT));
    p1[page.p1_index()].set(frame, flags | PRESENT);
}
