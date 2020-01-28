use super::{with_scheduler, SwitchReason};
use crate::multitasking::thread::ThreadId;
use alloc::boxed::Box;
use core::mem;
use core::raw::TraitObject;
use x86_64::VirtAddr;

pub struct Stack {
    pointer: VirtAddr,
}

impl Stack {
    pub unsafe fn new(stack_pointer: VirtAddr) -> Self {
        Stack {
            pointer: stack_pointer,
        }
    }

    pub fn get_stack_pointer(self) -> VirtAddr {
        self.pointer
    }

    pub fn set_up_for_closure(&mut self, closure: Box<dyn FnOnce() -> !>) {
        let trait_object: TraitObject = unsafe { mem::transmute(closure) };
        unsafe { self.push(trait_object.data) };
        unsafe { self.push(trait_object.vtable) };

        self.set_up_for_entry_point(call_closure_entry);
    }

    pub fn set_up_for_entry_point(&mut self, entry_point: fn() -> !) {
        unsafe { self.push(entry_point) };
        let rflags: u64 = 0x200;
        unsafe { self.push(rflags) };
    }

    unsafe fn push<T>(&mut self, value: T) {
        self.pointer -= core::mem::size_of::<T>();
        let ptr: *mut T = self.pointer.as_mut_ptr();
        ptr.write(value);
    }
}

pub unsafe fn context_switch_to(
    new_stack_pointer: VirtAddr,
    prev_thread_id: ThreadId,
    switch_reason: SwitchReason,
) {
    asm!(
        "call asm_context_switch"
        :
        : "{rdi}"(new_stack_pointer), "{rsi}"(prev_thread_id), "{rdx}"(switch_reason as u64)
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

#[no_mangle]
pub extern "C" fn add_paused_thread(
    paused_stack_pointer: VirtAddr,
    paused_thread_id: ThreadId,
    switch_reason: SwitchReason,
) {
    with_scheduler(|s| s.add_paused_thread(paused_stack_pointer, paused_thread_id, switch_reason));
}

#[naked]
fn call_closure_entry() -> ! {
    unsafe {
        asm!("
        pop rsi
        pop rdi
        call call_closure
    " ::: "mem" : "intel", "volatile")
    };
    unreachable!();
}

// no_mangle required because of https://github.com/rust-lang/rust/issues/68136
#[no_mangle]
extern "C" fn call_closure(data: *mut (), vtable: *mut ()) -> ! {
    let trait_object = TraitObject { data, vtable };
    let f: Box<dyn FnOnce() -> !> = unsafe { mem::transmute(trait_object) };
    f()
}
