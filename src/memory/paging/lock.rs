use memory::{Frame, FrameAllocator};
use memory::paging::{VirtualAddress, PhysicalAddress, Page, PAGE_SIZE, ENTRY_COUNT};
use memory::paging::table::{Table, P4, Level4};
use memory::paging::entry::{EntryFlags, PRESENT, WRITABLE, HUGE_PAGE};
use x86::tlb;

pub struct PageTable {
    p4_frame: Frame, // recursive mapped
}

impl PageTable {
    pub fn create_new_on_identity_mapped_frame(&self, identity_mapped_frame: Frame) -> PageTable {
        let page_address = Page { number: identity_mapped_frame.number }.start_address();
        // frame must be identity mapped
        assert!(self.read(|lock| lock.translate(page_address)) == Some(page_address));

        let table = unsafe { &mut *(page_address as *mut Table<Level4>) };
        table[511].set(Frame { number: identity_mapped_frame.number }, WRITABLE);
        PageTable { p4_frame: identity_mapped_frame }
    }

    pub fn read<F, R>(&self, f: F) -> R
        where F: FnOnce(&Lock) -> R
    {
        let p4_address = 0o177777_777_777_777_777_7770 as *mut usize;
        let backup = unsafe { *p4_address };
        let ret;
        if Frame::containing_address(backup) == self.p4_frame {
            ret = f(&Lock { _private: () });
        } else {
            unsafe { *p4_address = (self.p4_frame.number << 12) | 0b11 };
            ret = f(&Lock { _private: () });
            unsafe { *p4_address = backup };
        }
        ret
    }

    pub fn modify<F>(&mut self, f: F)
        where F: FnOnce(&mut Lock)
    {
        let p4_address = 0o177777_777_777_777_777_7770 as *mut usize;
        let backup = unsafe { *p4_address };
        if Frame::containing_address(backup) == self.p4_frame {
            f(&mut Lock { _private: () });
        } else {
            unsafe { *p4_address = (self.p4_frame.number << 12) | 0b11 };
            f(&mut Lock { _private: () });
            unsafe { *p4_address = backup };
        }
    }
}

pub struct Lock {
    _private: (),
}

impl Lock {
    pub fn translate(&self, virtual_address: VirtualAddress) -> Option<PhysicalAddress> {
        let offset = virtual_address % PAGE_SIZE;
        self.translate_page(Page::containing_address(virtual_address))
            .map(|frame| frame.number * PAGE_SIZE + offset)
    }

    fn translate_page(&self, page: Page) -> Option<Frame> {
        let p4 = unsafe { &*P4 };

        let huge_page = || {
            p4.next_table(page.p4_index())
              .and_then(|p3| {
                  // 1GiB page?
                  if p3[page.p3_index()].flags().contains(HUGE_PAGE | PRESENT) {
                      let start_frame_number = p3[page.p3_index()].pointed_frame().number;
                      // address must be 1GiB aligned
                      assert!(start_frame_number % (ENTRY_COUNT * ENTRY_COUNT) == 0);
                      return Some(start_frame_number + page.p2_index() * ENTRY_COUNT +
                                  page.p1_index());
                  }
                  if let Some(p2) = p3.next_table(page.p3_index()) {
                      // 2MiB page?
                      if p2[page.p2_index()].flags().contains(HUGE_PAGE | PRESENT) {
                          let start_frame_number = p2[page.p2_index()].pointed_frame().number;
                          // address must be 2MiB aligned
                          assert!(start_frame_number % ENTRY_COUNT == 0);
                          return Some(start_frame_number + page.p1_index());
                      }
                  }
                  None
              })
              .map(|start_frame_number| Frame { number: start_frame_number })
        };

        p4.next_table(page.p4_index())
          .and_then(|p3| p3.next_table(page.p3_index()))
          .and_then(|p2| p2.next_table(page.p2_index()))
          .map(|p1| p1[page.p1_index()].pointed_frame())
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
        let p4 = unsafe { &mut *P4 };
        let mut p3 = p4.next_table_create(page.p4_index(), allocator);
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
        assert!(self.translate(page.start_address()).is_some());

        let p4 = unsafe { &mut *P4 };
        let p1 = p4.next_table_mut(page.p4_index())
                   .and_then(|p3| p3.next_table_mut(page.p3_index()))
                   .and_then(|p2| p2.next_table_mut(page.p2_index()))
                   .unwrap();

        assert!(!p1[page.p1_index()].flags().contains(PRESENT));
        let frame = p1[page.p1_index()].pointed_frame();
        p1[page.p1_index()].set_unused();
        unsafe { tlb::flush(page.start_address()) };
        // TODO free p(1,2,3) table if empty
        allocator.deallocate_frame(frame);
    }
}
