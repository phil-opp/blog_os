use alloc::collections::VecDeque;
use lazy_static::lazy_static;
use x86_64::structures::paging::{FrameAllocator, Mapper, Size4KiB};
use x86_64::VirtAddr;

global_asm!(include_str!("multitasking/context_switch.s"));

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

pub fn scheduler() {
    let next = PAUSED_THREADS.try_lock().and_then(|mut t| t.pop_front());
    if let Some(next) = next {
        unsafe { context_switch(next) };
    }
}

lazy_static! {
    static ref PAUSED_THREADS: spin::Mutex<VecDeque<VirtAddr>> = spin::Mutex::new(VecDeque::new());
}

#[no_mangle]
fn add_paused_thread(stack_pointer: VirtAddr) {
    add_thread(stack_pointer)
}

fn add_thread(stack_pointer: VirtAddr) {
    PAUSED_THREADS.lock().push_back(stack_pointer);
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
    unsafe { add_thread(stack) };
}
