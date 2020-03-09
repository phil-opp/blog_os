use crate::println;
use conquer_once::spin::OnceCell;
use core::task::Waker;
use crossbeam_queue::ArrayQueue;

static INTERRUPT_WAKEUPS: OnceCell<ArrayQueue<Waker>> = OnceCell::uninit();

pub fn init() {
    INTERRUPT_WAKEUPS
        .try_init_once(|| ArrayQueue::new(10))
        .expect("failed to init interrupt wakeup queue");
}

/// Queues a waker for waking in an interrupt-safe way
pub(crate) fn interrupt_wake(waker: Waker) {
    if let Err(_) = interrupt_wakeups().push(waker) {
        println!("WARNING: dropping interrupt wakeup");
    }
}

pub(super) fn interrupt_wakeups() -> &'static ArrayQueue<Waker> {
    INTERRUPT_WAKEUPS
        .try_get()
        .expect("interrupt wakeup queue not initialized")
}
