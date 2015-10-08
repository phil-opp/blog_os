use core::ptr::Unique;
use core::mem;
use memory::paging;

pub type Frame = super::Frame;
pub type Page = super::paging::Page;

pub trait FrameAllocator {
    fn allocate_frame(&mut self, lock: &mut paging::Lock) -> Option<Frame>;
    fn deallocate_frame(&mut self, lock: &mut paging::Lock, frame: Frame);
}

pub struct DynamicFrameStack {
    head: Unique<Frame>, // TODO invariant
    length: usize,
    capacity: usize,
}

impl DynamicFrameStack {
    pub fn new(at: Page) -> DynamicFrameStack {
        DynamicFrameStack {
            head: unsafe{ Unique::new(at.pointer() as *mut () as *mut _) },
            length: 0,
            capacity: Self::capacity_per_frame(),
        }
    }

    fn capacity_per_frame() -> usize {
        (super::PAGE_SIZE as usize) / mem::size_of::<Frame>()
    }
}

impl FrameAllocator for DynamicFrameStack {
    fn allocate_frame(&mut self, lock: &mut paging::Lock) -> Option<Frame> {
        use core::intrinsics::offset;

        if self.length == 0 {
            // no frames left but maybe we can decrease the capacity and use that frame (but keep
            // at least 1 frame because the paging logic might need some frames to map a page)
            if self.capacity <= Self::capacity_per_frame() {
                None
            } else {
                // decrease capacity and thus free a frame used as backing store
                self.capacity -= Self::capacity_per_frame();
                let page_address = unsafe{ offset(*self.head, self.capacity as isize) } as usize;
                lock.mapper(self).unmap(Page::containing_address(page_address));
                self.allocate_frame(lock)
            }
        } else {
            // pop the last frame from the stack
            self.length -= 1;
            unsafe {
                let frame = offset(*self.head, self.length as isize) as *mut _;
                Some(mem::replace(&mut *frame, mem::zeroed()))
            }
        }
    }

    fn deallocate_frame(&mut self, lock: &mut paging::Lock, frame: Frame) {
        use core::intrinsics::offset;

        if self.length < self.capacity {
            // add frame to frame stack
            unsafe {
                let new_frame = offset(*self.head, self.length as isize) as *mut _;
                mem::forget(mem::replace(&mut *new_frame, frame));
            }
            self.length += 1;
        } else {
            // frame stack is full, use passed frame to expand it
            let page_address = unsafe{ offset(*self.head, self.capacity as isize) } as usize;
            lock.mapper(self).map_to(Page::containing_address(page_address), frame, true, false);
            self.capacity += Self::capacity_per_frame();
        }
    }
}
