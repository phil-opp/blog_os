use super::Task;
use alloc::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use cooked_waker::IntoWaker;
use core::task::{Context, Poll};
use crossbeam_queue::ArrayQueue;

pub struct SimpleExecutor {
    task_queue: VecDeque<Task>,
    waiting_tasks: BTreeMap<usize, Task>,
    wake_queue: Arc<ArrayQueue<usize>>,
}

impl SimpleExecutor {
    pub fn new() -> SimpleExecutor {
        SimpleExecutor {
            task_queue: VecDeque::new(),
            waiting_tasks: BTreeMap::new(),
            wake_queue: Arc::new(ArrayQueue::new(100)),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }

    pub fn run(&mut self) {
        loop {
            self.handle_wakeups();
            self.run_ready_tasks();
        }
    }

    fn handle_wakeups(&mut self) {
        while let Ok(task_id) = self.wake_queue.pop() {
            if let Some(task) = self.waiting_tasks.remove(&task_id) {
                self.task_queue.push_back(task);
            }
        }
    }

    fn run_ready_tasks(&mut self) {
        while let Some(mut task) = self.task_queue.pop_front() {
            let waker = TaskWaker {
                task_id: task.id(),
                wake_queue: self.wake_queue.clone(),
            }
            .into_waker();
            let mut context = Context::from_waker(&waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {} // task done
                Poll::Pending => {
                    if self.waiting_tasks.insert(task.id(), task).is_some() {
                        panic!("Same task inserted into waiting_tasks twice");
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, IntoWaker)]
struct TaskWaker {
    task_id: usize,
    wake_queue: Arc<ArrayQueue<usize>>,
}

impl TaskWaker {
    fn wake_task(&self) {
        self.wake_queue.push(self.task_id).expect("wake queue full");
    }
}

impl cooked_waker::WakeRef for TaskWaker {
    fn wake_by_ref(&self) {
        self.wake_task();
    }
}

impl cooked_waker::Wake for TaskWaker {
    fn wake(self) {
        self.wake_task();
    }
}
