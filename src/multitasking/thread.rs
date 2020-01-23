use crate::memory::{alloc_stack, StackBounds};
use crate::multitasking::context_switch::Stack;
use alloc::boxed::Box;
use x86_64::{
    structures::paging::{mapper, FrameAllocator, Mapper, Size4KiB},
    VirtAddr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadId(u64);

impl ThreadId {
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    fn new() -> Self {
        use core::sync::atomic::{AtomicU64, Ordering};
        static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);
        ThreadId(NEXT_THREAD_ID.fetch_add(1, Ordering::SeqCst))
    }
}

#[derive(Debug)]
pub struct Thread {
    id: ThreadId,
    stack_pointer: Option<VirtAddr>,
    stack_bounds: Option<StackBounds>,
}

impl Thread {
    pub fn create(
        entry_point: fn() -> !,
        stack_size: u64,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<Self, mapper::MapToError> {
        let stack_bounds = alloc_stack(stack_size, mapper, frame_allocator)?;
        let mut stack = unsafe { Stack::new(stack_bounds.end()) };
        stack.set_up_for_entry_point(entry_point);
        Ok(Self::new(stack.get_stack_pointer(), stack_bounds))
    }

    pub fn create_from_closure<F>(
        closure: F,
        stack_size: u64,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<Self, mapper::MapToError>
    where
        F: FnOnce() -> ! + 'static + Send + Sync,
    {
        let stack_bounds = alloc_stack(stack_size, mapper, frame_allocator)?;
        let mut stack = unsafe { Stack::new(stack_bounds.end()) };
        stack.set_up_for_closure(Box::new(closure));
        Ok(Self::new(stack.get_stack_pointer(), stack_bounds))
    }

    fn new(stack_pointer: VirtAddr, stack_bounds: StackBounds) -> Self {
        Thread {
            id: ThreadId::new(),
            stack_pointer: Some(stack_pointer),
            stack_bounds: Some(stack_bounds),
        }
    }

    pub(super) fn create_root_thread() -> Self {
        Thread {
            id: ThreadId(0),
            stack_pointer: None,
            stack_bounds: None,
        }
    }

    pub fn id(&self) -> ThreadId {
        self.id
    }

    pub(super) fn stack_pointer(&mut self) -> &mut Option<VirtAddr> {
        &mut self.stack_pointer
    }
}
