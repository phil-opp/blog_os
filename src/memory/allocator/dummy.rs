use core::alloc::{GlobalAlloc, Layout};

/// A dummy allocator that panics on every `alloc` or `dealloc` call.
pub struct DummyAllocator;

unsafe impl GlobalAlloc for DummyAllocator {
    /// Always panics.
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        panic!("DummyAllocator::alloc called");
    }

    /// Always panics.
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("DummyAllocator::dealloc called");
    }
}
