pub use bump::BumpAllocator;
pub use dummy::DummyAllocator;
pub use linked_list::LinkedListAllocator;
pub use bucket::BucketAllocator;

use core::alloc::{GlobalAlloc, Layout};
use spin::{Mutex, MutexGuard};

mod bump;
mod dummy;
mod linked_list;
mod bucket;

pub struct LockedAllocator<T> {
    allocator: Mutex<T>,
}

impl<T> LockedAllocator<T> {
    pub const fn new(allocator: T) -> Self {
        Self {
            allocator: Mutex::new(allocator),
        }
    }
}

impl<T> LockedAllocator<T> {
    pub fn lock(&self) -> MutexGuard<T> {
        self.allocator.lock()
    }
}

unsafe impl<T> GlobalAlloc for LockedAllocator<T>
where
    T: MutGlobalAlloc,
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocator.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.allocator.lock().dealloc(ptr, layout)
    }
}

pub trait MutGlobalAlloc {
    fn alloc(&mut self, layout: Layout) -> *mut u8;

    fn dealloc(&mut self, ptr: *mut u8, layout: Layout);
}
