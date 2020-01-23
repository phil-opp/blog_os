use alloc::collections::VecDeque;
use x86_64::structures::paging::{FrameAllocator, Mapper, Size4KiB};
use x86_64::VirtAddr;
use core::mem;

static SCHEDULER: spin::Mutex<Option<Scheduler>> = spin::Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackPointer(VirtAddr);

impl StackPointer {
    unsafe fn new(pointer: VirtAddr) -> Self {
        StackPointer(pointer)
    }

    pub fn allocate(
        stack_size: u64,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<Self, ()> {
        crate::memory::alloc_stack(stack_size, mapper, frame_allocator).map(Self)
    }

    unsafe fn push_to_stack<T>(&mut self, value: T) {
        self.0 -= core::mem::size_of::<T>();
        let ptr: *mut T = self.0.as_mut_ptr();
        ptr.write(value);
    }

    fn as_u64(&self) -> u64 {
        self.0.as_u64()
    }
}

pub struct Thread {
    id: ThreadId,
    stack_pointer: StackPointer,
}

impl Thread {
    pub unsafe fn new(entry_point: fn() -> !, stack_top: StackPointer) -> Self {
        use core::sync::atomic::{AtomicU64, Ordering};
        static NextThreadId: AtomicU64 = AtomicU64::new(1);

        let mut stack_pointer = stack_top;
        Self::set_up_stack(&mut stack_pointer, entry_point);

        Thread {
            id: ThreadId(NextThreadId.fetch_add(1, Ordering::SeqCst)),
            stack_pointer,
        }
    }

    pub unsafe fn new_from_closure<F>(closure: F, stack_top: StackPointer) -> Self
    where
        F: FnOnce() -> ! + Send + Sync + 'static,
    {
        use alloc::boxed::Box;
        use core::{mem, raw::TraitObject};

        let boxed: ThreadClosure = Box::new(closure);
        let trait_object: TraitObject = unsafe { mem::transmute(boxed) };

        // push trait object
        let mut stack_pointer = stack_top;
        unsafe { stack_pointer.push_to_stack(trait_object.data) };
        unsafe { stack_pointer.push_to_stack(trait_object.vtable) };

        let entry_point = call_closure_entry as unsafe fn() -> !;
        unsafe { Self::new(mem::transmute(entry_point), stack_pointer) }
    }

    pub fn create(
        entry_point: fn() -> !,
        stack_size: u64,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<Self, ()> {
        let stack_top = StackPointer::allocate(stack_size, mapper, frame_allocator)?;
        Ok(unsafe { Self::new(entry_point, stack_top) })
    }

    pub fn create_from_closure<F>(
        entry_point: F,
        stack_size: u64,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<Self, ()>
    where
        F: FnOnce() -> ! + Send + Sync + 'static,
    {
        let stack_top = StackPointer::allocate(stack_size, mapper, frame_allocator)?;
        Ok(unsafe { Self::new_from_closure(entry_point, stack_top) })
    }

    fn set_up_stack(stack_top: &mut StackPointer, entry_point: fn() -> !) {
        unsafe { stack_top.push_to_stack(entry_point) };
        let rflags: u64 = 0x200;
        unsafe { stack_top.push_to_stack(rflags) };
    }
}

struct Scheduler {
    current_thread_id: ThreadId,
    paused_threads: VecDeque<Thread>,
}

impl Scheduler {
    fn new() -> Self {
        Scheduler {
            current_thread_id: ThreadId(0),
            paused_threads: VecDeque::new(),
        }
    }

    fn next_thread(&mut self) -> Option<Thread> {
        self.paused_threads.pop_front()
    }

    fn add_paused_thread(&mut self, stack_pointer: StackPointer, new_thread_id: ThreadId) {
        let thread_id = mem::replace(&mut self.current_thread_id, new_thread_id);
        let thread = Thread { id: thread_id, stack_pointer};
        self.paused_threads.push_back(thread);
    }

    fn add_new_thread(&mut self, thread: Thread) {
        self.paused_threads.push_back(thread);
    }

    pub fn current_thread_id(&self) -> ThreadId {
        self.current_thread_id
    }
}

pub unsafe fn context_switch(thread: Thread) {
    asm!(
        "call asm_context_switch"
        :
        : "{rdi}"(thread.stack_pointer.as_u64()), "{rsi}"(thread.id.0)
        : "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rpb", "r8", "r9", "r10",
        "r11", "r12", "r13", "r14", "r15", "rflags", "memory"
        : "intel", "volatile"
    );
}

global_asm!(
    "
    .intel_syntax noprefix

    // asm_context_switch(stack_pointer: u64, thread_id: u64)
    asm_context_switch:
        pushfq

        mov rax, rsp
        mov rsp, rdi

        mov rdi, rax
        call add_paused_thread

        popfq
        ret
"
);

pub fn scheduler() {
    let next = SCHEDULER
        .try_lock()
        .and_then(|mut scheduler| scheduler.as_mut().and_then(|s| s.next_thread()));
    if let Some(next) = next {
        unsafe { context_switch(next) };
    }
}

static PAUSED_THREADS: spin::Mutex<Option<VecDeque<VirtAddr>>> = spin::Mutex::new(None);

#[no_mangle]
pub extern "C" fn add_paused_thread(stack_pointer: u64, new_thread_id: u64) {
    let stack_pointer = StackPointer(VirtAddr::new(stack_pointer));
    let new_thread_id = ThreadId(new_thread_id);

    SCHEDULER.lock().get_or_insert_with(Scheduler::new).add_paused_thread(stack_pointer, new_thread_id);
}

pub fn create_thread(
    entry_point: fn() -> !,
    stack_size: u64,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), ()> {
    let thread = Thread::create(entry_point, stack_size, mapper, frame_allocator)?;
    SCHEDULER.lock().get_or_insert_with(Scheduler::new).add_new_thread(thread);
    Ok(())
}

pub fn create_thread_from_closure<F>(
    closure: F,
    stack_size: u64,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), ()> where
    F: FnOnce() -> ! + 'static + Send + Sync,
{
    let thread = Thread::create_from_closure(closure, stack_size, mapper, frame_allocator)?;
    SCHEDULER.lock().get_or_insert_with(Scheduler::new).add_new_thread(thread);
    Ok(())
}

type ThreadClosure = alloc::boxed::Box<dyn FnOnce() -> !>;

#[no_mangle]
unsafe fn call_closure(data: *mut (), vtable: *mut ()) -> ! {
    use core::{mem, raw::TraitObject};

    let trait_object = TraitObject { data, vtable };
    let f: ThreadClosure = mem::transmute(trait_object);
    f()
}

#[naked]
unsafe fn call_closure_entry() -> ! {
    asm!("
        pop rsi
        pop rdi
        call call_closure
    " ::: "mem" : "intel", "volatile");
    unreachable!();
}
