use crate::{interrupts, print};
use core::future::Future;
use core::{
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

fn next_tick(current_tick: u64) -> impl Future<Output = u64> {
    NextTick {
        ticks: current_tick,
    }
}

struct NextTick {
    ticks: u64,
}

impl Future for NextTick {
    type Output = u64;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<u64> {
        let current_ticks = interrupts::TIMER_TICKS.load(Ordering::Acquire);

        if self.ticks < current_ticks {
            self.ticks += 1;
            return Poll::Ready(self.ticks);
        }

        interrupts::TIMER_INTERRUPT_WAKER.register(&cx.waker());
        let current_ticks = interrupts::TIMER_TICKS.load(Ordering::Acquire);
        if self.ticks < current_ticks {
            self.ticks += 1;
            Poll::Ready(self.ticks)
        } else {
            Poll::Pending
        }
    }
}

pub async fn print_ticks() {
    let mut current_ticks = interrupts::TIMER_TICKS.load(Ordering::Acquire);

    loop {
        current_ticks = next_tick(current_ticks).await;
        print!(".");
    }
}
