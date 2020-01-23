use alloc::collections::VecDeque;
use x86_64::structures::paging::{FrameAllocator, Mapper, Size4KiB};
use x86_64::VirtAddr;

pub unsafe fn context_switch(stack_pointer: VirtAddr) {
    asm!(
        "call asm_context_switch"
        :
        : "{rdi}"(stack_pointer.as_u64())
        : "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rpb", "r8", "r9", "r10",
        "r11", "r12", "r13", "r14", "r15", "rflags", "memory"
        : "intel", "volatile"
    );
}

global_asm!(
    "
    .intel_syntax noprefix

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
    let next = PAUSED_THREADS.try_lock().and_then(|mut paused_threads| {
        paused_threads
            .as_mut()
            .and_then(|threads| threads.pop_front())
    });
    if let Some(next) = next {
        unsafe { context_switch(next) };
    }
}

static PAUSED_THREADS: spin::Mutex<Option<VecDeque<VirtAddr>>> = spin::Mutex::new(None);

#[no_mangle]
fn add_paused_thread(stack_pointer: VirtAddr) {
    add_thread(stack_pointer)
}

fn add_thread(stack_pointer: VirtAddr) {
    PAUSED_THREADS
        .lock()
        .get_or_insert_with(VecDeque::new)
        .push_back(stack_pointer);
}

pub fn create_thread(
    f: fn() -> !,
    stack_size: u64,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    let mut stack = crate::memory::alloc_stack(stack_size, mapper, frame_allocator).unwrap();
    stack -= core::mem::size_of::<u64>();
    let ptr: *mut u64 = stack.as_mut_ptr();
    unsafe { ptr.write(f as u64) };
    stack -= core::mem::size_of::<u64>();
    let ptr: *mut u64 = stack.as_mut_ptr();
    let rflags = 0x200;
    unsafe { ptr.write(rflags) };
    add_thread(stack);
}

pub fn create_thread_from_closure<F>(
    f: F,
    stack_size: u64,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) where
    F: FnOnce() -> ! + 'static,
{
    use alloc::boxed::Box;
    use core::{mem, raw::TraitObject};

    let boxed: ThreadClosure = Box::new(f);
    let trait_object: TraitObject = unsafe { mem::transmute(boxed) };

    let mut stack = crate::memory::alloc_stack(stack_size, mapper, frame_allocator).unwrap();

    // push trait object
    stack -= core::mem::size_of::<*mut ()>();
    let ptr: *mut *mut () = stack.as_mut_ptr();
    unsafe { ptr.write(trait_object.data) };
    stack -= core::mem::size_of::<*mut ()>();
    let ptr: *mut *mut () = stack.as_mut_ptr();
    unsafe { ptr.write(trait_object.vtable) };

    stack -= core::mem::size_of::<u64>();
    let ptr: *mut u64 = stack.as_mut_ptr();
    unsafe { ptr.write(call_closure_entry as u64) };
    stack -= core::mem::size_of::<u64>();
    let ptr: *mut u64 = stack.as_mut_ptr();
    let rflags = 0x200;
    unsafe { ptr.write(rflags) };
    add_thread(stack);
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
