use crate::{interrupts, println};
use alloc::{boxed::Box, collections::BTreeMap, sync::Arc, task::Wake};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::SegQueue;

pub type Task = Pin<Box<dyn Future<Output = ()>>>;
type TaskQueue = SegQueue<Task>;
type TaskId = usize;
type WakeQueue = SegQueue<TaskId>;

pub struct Executor {
    task_queue: Arc<TaskQueue>,
    wake_queue: Arc<WakeQueue>,
    pending_tasks: BTreeMap<TaskId, Task>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            task_queue: Arc::new(TaskQueue::new()),
            wake_queue: Arc::new(WakeQueue::new()),
            pending_tasks: BTreeMap::new(),
        }
    }

    pub fn create_spawner(&self) -> Spawner {
        Spawner {
            task_queue: self.task_queue.clone(),
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            // perform wakeups caused by interrupts
            // the interrupt handlers can't do it themselves since wakers might execute
            // arbitrary code, e.g. allocate
            while let Ok(waker) = interrupts::interrupt_wakeups().pop() {
                waker.wake();
            }
            // wakeup waiting tasks
            while let Ok(task_id) = self.wake_queue.pop() {
                if let Some(task) = self.pending_tasks.remove(&task_id) {
                    self.task_queue.push(task);
                } else {
                    println!("WARNING: woken task not found in pending_tasks");
                }
            }
            // run ready tasks
            while let Ok(mut task) = self.task_queue.pop() {
                let waker = self.create_waker(&task).into();
                let mut context = Context::from_waker(&waker);
                match task.as_mut().poll(&mut context) {
                    Poll::Ready(()) => {} // task done
                    Poll::Pending => {
                        // add task to pending_tasks list and wait for wakeup
                        let task_id = Self::task_id(&task);
                        if self.pending_tasks.insert(task_id, task).is_some() {
                            panic!("Task with same ID already in pending_tasks queue");
                        }
                    }
                }
            }
            // wait for next interrupt if there is nothing left to do
            if self.wake_queue.is_empty() {
                unsafe { asm!("cli") };
                if self.wake_queue.is_empty() {
                    unsafe { asm!("sti; hlt") };
                } else {
                    unsafe { asm!("sti") };
                }
            }
        }
    }

    fn task_id(task: &Task) -> TaskId {
        let future_ref: &dyn Future<Output = ()> = &**task;
        future_ref as *const _ as *const () as usize
    }

    fn create_waker(&self, task: &Task) -> Arc<Waker> {
        Arc::new(Waker {
            wake_queue: self.wake_queue.clone(),
            task_id: Self::task_id(task),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Spawner {
    task_queue: Arc<TaskQueue>,
}

impl Spawner {
    pub fn spawn(&self, task: impl Future<Output = ()> + 'static) {
        self.task_queue.push(Box::pin(task))
    }
}

pub struct Waker {
    wake_queue: Arc<WakeQueue>,
    task_id: TaskId,
}

impl Wake for Waker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_queue.push(self.task_id);
    }
}
