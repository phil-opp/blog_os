use super::{VirtualAddress, PhysicalAddress, Page, PAGE_SIZE, ENTRY_COUNT};
use super::table::{Table, P4};
use super::entry::{PRESENT, HUGE_PAGE};
use memory::Frame;


pub fn translate(virtual_address: usize) -> Option<PhysicalAddress> {
    let page = Page::containing_address(virtual_address);
    let offset = virtual_address % PAGE_SIZE;

    let p4 = unsafe { &*P4 };

    let huge_page = || {
        p4.next_table(page.p4_index())
          .and_then(|p3| {
              // 1GiB page?
              if p3[page.p3_index()].flags().contains(HUGE_PAGE | PRESENT) {
                  let start_frame_number = p3[page.p3_index()].pointed_frame().number;
                  // address must be 1GiB aligned
                  assert!(start_frame_number % (ENTRY_COUNT * ENTRY_COUNT) == 0);
                  return Some(start_frame_number + page.p2_index() * ENTRY_COUNT + page.p1_index());
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
      .map(|frame| frame.number * PAGE_SIZE + offset)
}
