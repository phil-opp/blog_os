use core::ptr::Unique;
use memory::{PAGE_SIZE, Frame, FrameAllocator};
use self::table::{Table, Level4};
use self::entry::*;

mod entry;
mod table;
pub mod translate;
pub mod mapping;

pub fn test<A>(frame_allocator: &mut A)
    where A: super::FrameAllocator
{
    use self::entry::PRESENT;
    mapping::map(&Page::containing_address(0xdeadbeaa000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0xdeadbeab000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0xdeadbeac000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0xdeadbead000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0xcafebeaf000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0x0), PRESENT, frame_allocator);
}

const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;

pub struct Page {
    number: usize,
}

impl Page {
    fn containing_address(address: VirtualAddress) -> Page {
        assert!(address < 0x0000_8000_0000_0000 || address >= 0xffff_8000_0000_0000,
                "invalid address: 0x{:x}",
                address);
        Page { number: address / PAGE_SIZE }
    }

    fn start_address(&self) -> VirtualAddress {
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

pub struct RecursivePageTable {
    p4: Unique<Table<Level4>>,
}

impl RecursivePageTable {
    pub unsafe fn new() -> RecursivePageTable {
        use self::table::P4;
        RecursivePageTable {
            p4: Unique::new(P4),
        }
    }

    fn p4(&self) -> &Table<Level4> {
        unsafe { self.p4.get() }
    }

    fn p4_mut(&mut self) -> &mut Table<Level4> {
        unsafe { self.p4.get_mut() }
    }

    pub fn translate(&self, virtual_address: VirtualAddress) -> Option<PhysicalAddress> {
        let offset = virtual_address % PAGE_SIZE;
        self.translate_page(Page::containing_address(virtual_address))
            .map(|frame| frame.number * PAGE_SIZE + offset)
    }

    fn translate_page(&self, page: Page) -> Option<Frame> {
        let p3 = self.p4().next_table(page.p4_index());

        let huge_page = || {
            p3.and_then(|p3| {
                let p3_entry = &p3[page.p3_index()];
                // 1GiB page?
                if let Some(start_frame) = p3_entry.pointed_frame() {
                    if p3_entry.flags().contains(HUGE_PAGE) {
                        // address must be 1GiB aligned
                        assert!(start_frame.number % (ENTRY_COUNT * ENTRY_COUNT) == 0);
                        return Some(Frame {
                            number: start_frame.number + page.p2_index() * ENTRY_COUNT +
                                    page.p1_index(),
                        });
                    }
                }
                if let Some(p2) = p3.next_table(page.p3_index()) {
                    let p2_entry = &p2[page.p2_index()];
                    // 2MiB page?
                    if let Some(start_frame) = p2_entry.pointed_frame() {
                        if p2_entry.flags().contains(HUGE_PAGE) {
                            // address must be 2MiB aligned
                            assert!(start_frame.number % ENTRY_COUNT == 0);
                            return Some(Frame { number: start_frame.number + page.p1_index() });
                        }
                    }
                }
                None
            })
        };

        p3.and_then(|p3| p3.next_table(page.p3_index()))
          .and_then(|p2| p2.next_table(page.p2_index()))
          .and_then(|p1| p1[page.p1_index()].pointed_frame())
          .or_else(huge_page)
    }

    pub fn map<A>(&mut self, page: &Page, flags: EntryFlags, allocator: &mut A)
        where A: FrameAllocator
    {
        let frame = allocator.allocate_frame().expect("out of memory");
        self.map_to(page, frame, flags, allocator)
    }

    pub fn map_to<A>(&mut self, page: &Page, frame: Frame, flags: EntryFlags, allocator: &mut A)
        where A: FrameAllocator
    {
        let mut p3 = self.p4_mut().next_table_create(page.p4_index(), allocator);
        let mut p2 = p3.next_table_create(page.p3_index(), allocator);
        let mut p1 = p2.next_table_create(page.p2_index(), allocator);

        assert!(!p1[page.p1_index()].flags().contains(PRESENT));
        p1[page.p1_index()].set(frame, flags | PRESENT);
    }

    pub fn identity_map<A>(&mut self, frame: Frame, flags: EntryFlags, allocator: &mut A)
        where A: FrameAllocator
    {
        let page = Page { number: frame.number };
        self.map_to(&page, frame, flags, allocator)
    }


    fn unmap<A>(&mut self, page: &Page, allocator: &mut A)
        where A: FrameAllocator
    {
        use x86::tlb;

        assert!(self.translate(page.start_address()).is_some());

        let p1 = self.p4_mut()
                     .next_table_mut(page.p4_index())
                     .and_then(|p3| p3.next_table_mut(page.p3_index()))
                     .and_then(|p2| p2.next_table_mut(page.p2_index()))
                     .unwrap();
        let frame = p1[page.p1_index()].pointed_frame().unwrap();
        p1[page.p1_index()].set_unused();
        unsafe { tlb::flush(page.start_address()) };
        // TODO free p(1,2,3) table if empty
        allocator.deallocate_frame(frame);
    }
}

pub struct InactivePageTable {
    p4_frame: Frame, // recursive mapped
}

impl InactivePageTable {
    pub fn create_new_on_identity_mapped_frame(&self,
                                               identity_mapped_frame: Frame)
                                               -> InactivePageTable {
        let page_address = Page { number: identity_mapped_frame.number }.start_address();
        // frame must be identity mapped
        assert!(self.read(|lock| lock.translate(page_address)) == Some(page_address));

        let table = unsafe { &mut *(page_address as *mut Table<Level4>) };
        table[511].set(Frame { number: identity_mapped_frame.number }, WRITABLE);
        InactivePageTable { p4_frame: identity_mapped_frame }
    }

    pub fn read<F, R>(&self, f: F) -> R
        where F: FnOnce(&RecursivePageTable) -> R
    {
        self.activate_temporary(|pt| f(pt))
    }

    pub fn modify<F>(&mut self, f: F)
        where F: FnOnce(&mut RecursivePageTable)
    {
        self.activate_temporary(f)
    }

    fn activate_temporary<F, R>(&self, f: F) -> R
        where F: FnOnce(&mut RecursivePageTable) -> R
    {
        use memory::paging::table::P4;

        let mut page_table = RecursivePageTable { p4: unsafe { Unique::new(P4) } };

        let backup = page_table.p4()[511].pointed_frame().unwrap();
        if backup == self.p4_frame {
            f(&mut page_table)
        } else {
            page_table.p4_mut()[511]
                .set(Frame { number: self.p4_frame.number }, PRESENT | WRITABLE);
            let ret = f(&mut page_table);
            page_table.p4_mut()[511].set(backup, PRESENT | WRITABLE);
            ret
        }
    }
}
