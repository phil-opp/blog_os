use core::ptr::Unique;
use core::mem;
use super::{Page, FrameStack};

pub type Frame = super::Frame;

pub struct DynamicFrameStack {
    head: Unique<Frame>, // TODO invariant
    length: usize,
    capacity: usize,
}

impl DynamicFrameStack {
    pub fn new(at: *mut Frame) -> DynamicFrameStack {
        DynamicFrameStack {
            head: unsafe{ Unique::new(at) },
            length: 0,
            capacity: 0,
        }
    }

    fn capacity_per_frame() -> usize {
        (super::PAGE_SIZE as usize) / mem::size_of::<Frame>()
    }
}

impl FrameStack for DynamicFrameStack {
    fn push<F>(&mut self, frame: Frame, map_to: F)
        where F: FnOnce(Page, Frame),
    {
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
            map_to(Page::containing_address(page_address), frame);
            self.capacity += Self::capacity_per_frame();
        }
    }

    fn pop<F>(&mut self, unmap_page: F) -> Option<Frame>
        where F: FnOnce(Page) -> Frame,
    {
        use core::intrinsics::offset;

        if self.length == 0 {
            // no frames left but maybe we can decrease the capacity and use that frame
            if self.capacity == 0 {
                None
            } else {
                // decrease capacity and thus free a frame used as backing store
                self.capacity -= Self::capacity_per_frame();
                let page_address = unsafe{ offset(*self.head, self.capacity as isize) } as usize;
                Some(unmap_page(Page::containing_address(page_address)))

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
}
