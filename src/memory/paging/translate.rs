use super::{VirtualAddress, PhysicalAddress, Page, PAGE_SIZE, ENTRY_COUNT};
use super::table::P4;
use super::entry::HUGE_PAGE;
use memory::Frame;

pub fn translate(virtual_address: VirtualAddress) -> Option<PhysicalAddress> {
    let offset = virtual_address % PAGE_SIZE;
    translate_page(Page::containing_address(virtual_address))
        .map(|frame| frame.number * PAGE_SIZE + offset)
}

fn translate_page(page: Page) -> Option<Frame> {
    let p4 = unsafe { &*P4 };

    let huge_page = || {
        p4.next_table(page.p4_index())
          .and_then(|p3| {
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

    p4.next_table(page.p4_index())
      .and_then(|p3| p3.next_table(page.p3_index()))
      .and_then(|p2| p2.next_table(page.p2_index()))
      .and_then(|p1| p1[page.p1_index()].pointed_frame())
      .or_else(huge_page)
}
