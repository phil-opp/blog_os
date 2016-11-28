use memory::paging::{self, Page, PageIter, ActivePageTable};
use memory::{PAGE_SIZE, FrameAllocator};
use core::nonzero::NonZero;

pub fn new_stack_allocator(page_range: PageIter) -> StackAllocator {
    StackAllocator { range: page_range }
}

pub struct StackAllocator {
    range: PageIter,
}

impl StackAllocator {
    pub fn alloc_stack<FA: FrameAllocator>(&mut self,
                                           active_table: &mut ActivePageTable,
                                           frame_allocator: &mut FA,
                                           size_in_pages: usize)
                                           -> Result<StackPointer, ()> {
        if size_in_pages == 0 {
            return Err(());
        }

        let _guard_page = self.range.next().ok_or(())?;

        let stack_start = self.range.next().ok_or(())?;
        let stack_end = if size_in_pages == 1 {
            stack_start
        } else {
            self.range.nth(size_in_pages - 1).ok_or(())?
        };

        for page in Page::range_inclusive(stack_start, stack_end) {
            active_table.map(page, paging::WRITABLE, frame_allocator);
        }

        let top_of_stack = stack_end.start_address() + PAGE_SIZE;
        StackPointer::new(top_of_stack).ok_or(())
    }
}

#[derive(Debug)]
pub struct StackPointer(NonZero<usize>);

impl StackPointer {
    fn new(ptr: usize) -> Option<StackPointer> {
        match ptr {
            0 => None,
            ptr => Some(StackPointer(unsafe { NonZero::new(ptr) })),
        }
    }
}
