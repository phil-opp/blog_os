use super::Task;
use alloc::collections::VecDeque;
use core::{
    ptr,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

pub struct SimpleExecutor {
    task_queue: VecDeque<Task>,
}

impl SimpleExecutor {
    pub fn new() -> SimpleExecutor {
        SimpleExecutor {
            task_queue: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }

    pub fn run(&mut self) {
        while let Some(mut task) = self.task_queue.pop_front() {
            let waker = waker();
            let mut context = Context::from_waker(&waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {} // task done
                Poll::Pending => self.task_queue.push_back(task),
            }
        }
    }
}

fn raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        raw_waker()
    }

    RawWaker::new(
        ptr::null(),
        &RawWakerVTable::new(clone, no_op, no_op, no_op),
    )
}

fn waker() -> Waker {
    unsafe { Waker::from_raw(raw_waker()) }
}
