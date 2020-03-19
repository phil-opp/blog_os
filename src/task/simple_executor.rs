use super::Task;
use alloc::{collections::VecDeque, sync::Arc, task::Wake};
use core::task::{Context, Poll, Waker};

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
            let waker = DummyWaker.to_waker();
            let mut context = Context::from_waker(&waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {} // task done
                Poll::Pending => self.task_queue.push_back(task),
            }
        }
    }
}

struct DummyWaker;

impl Wake for DummyWaker {
    fn wake(self: Arc<Self>) {
        // do nothing
    }
}

impl DummyWaker {
    fn to_waker(self) -> Waker {
        Waker::from(Arc::new(self))
    }
}
