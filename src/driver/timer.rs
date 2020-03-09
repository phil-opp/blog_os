use crate::{print, task::interrupt_wake};
use core::future::Future;
use core::{
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
};
use futures_util::task::AtomicWaker;

static TICKS: AtomicU64 = AtomicU64::new(0);
static WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the timer interrupt handler
///
/// Must not block (including spinlocks).
pub(crate) fn tick() {
    TICKS.fetch_add(1, Ordering::Release);
    if let Some(waker) = WAKER.take() {
        interrupt_wake(waker);
    }
}

fn next_tick() -> impl Future<Output = u64> {
    static NEXT_TICK: AtomicU64 = AtomicU64::new(1);

    NextTick {
        ticks: NEXT_TICK.fetch_add(1, Ordering::Release),
    }
}

struct NextTick {
    ticks: u64,
}

impl Future for NextTick {
    type Output = u64;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<u64> {
        WAKER.register(&cx.waker());
        let current_ticks = TICKS.load(Ordering::Acquire);
        if self.ticks < current_ticks {
            self.ticks += 1;
            Poll::Ready(self.ticks)
        } else {
            Poll::Pending
        }
    }
}

pub async fn timer_task() {
    loop {
        next_tick().await;
        print!(".");
    }
}
