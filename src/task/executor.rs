use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;

pub struct Executor {
    task_queue: Arc<ArrayQueue<TaskId>>,
    waiting_tasks: BTreeMap<TaskId, Task>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            task_queue: Arc::new(ArrayQueue::new(100)),
            waiting_tasks: BTreeMap::new(),
            waker_cache: BTreeMap::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        self.add_waiting(task);
        self.task_queue.push(task_id).expect("task_queue full");
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    fn add_waiting(&mut self, task: Task) {
        if self.waiting_tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in waiting_tasks");
        }
    }

    fn run_ready_tasks(&mut self) {
        while let Ok(task_id) = self.task_queue.pop() {
            let mut task = match self.waiting_tasks.remove(&task_id) {
                Some(task) => task,
                None => continue,
            };
            if !self.waker_cache.contains_key(&task_id) {
                self.waker_cache.insert(task_id, self.create_waker(task_id));
            }
            let waker = self.waker_cache.get(&task_id).expect("should exist");
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove cached waker
                    self.waker_cache.remove(&task_id);
                }
                Poll::Pending => self.add_waiting(task),
            }
        }
    }

    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts::{self, enable_interrupts_and_hlt};

        interrupts::disable();
        if self.task_queue.is_empty() {
            enable_interrupts_and_hlt();
        } else {
            interrupts::enable();
        }
    }

    fn create_waker(&self, task_id: TaskId) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue: self.task_queue.clone(),
        }))
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task_queue full");
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}
