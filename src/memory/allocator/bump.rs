use super::MutGlobalAlloc;
use core::alloc::Layout;
use x86_64::align_up;

pub struct BumpAllocator {
    heap_start: u64,
    heap_end: u64,
    next: u64,
}

impl BumpAllocator {
    pub const fn new(heap_start: u64, heap_end: u64) -> Self {
        Self {
            heap_start,
            heap_end,
            next: heap_start,
        }
    }
}

impl MutGlobalAlloc for BumpAllocator {
    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let alloc_start = align_up(self.next, layout.align() as u64);
        let alloc_end = alloc_start.saturating_add(layout.size() as u64);

        if alloc_end >= self.heap_end {
            // out of memory
            return 0 as *mut u8;
        }

        self.next = alloc_end;
        alloc_start as *mut u8
    }

    fn dealloc(&mut self, _ptr: *mut u8, _layout: Layout) {
        panic!("BumpAllocator::dealloc called");
    }
}
