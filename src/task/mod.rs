use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub mod executor;
pub mod keyboard;
pub mod simple_executor;

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            future: Box::pin(future),
        }
    }

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }

    fn id(&self) -> TaskId {
        use core::ops::Deref;

        let addr = Pin::deref(&self.future) as *const _ as *const () as usize;
        TaskId(addr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(usize);
