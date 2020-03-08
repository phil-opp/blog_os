use crate::{interrupts, println};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    task::Wake,
};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::SegQueue;

pub type Task = Pin<Box<dyn Future<Output = ()>>>;
type TaskId = usize;

pub struct Executor {
    task_queue: VecDeque<Task>,
    wake_queue: Arc<SegQueue<TaskId>>,
    pending_tasks: BTreeMap<TaskId, Task>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            task_queue: VecDeque::new(),
            wake_queue: Arc::new(SegQueue::new()),
            pending_tasks: BTreeMap::new(),
        }
    }

    pub fn spawn(&mut self, task: impl Future<Output = ()> + 'static) {
        self.task_queue.push_back(Box::pin(task))
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
                    self.task_queue.push_back(task);
                } else {
                    println!("WARNING: woken task not found in pending_tasks");
                }
            }
            // run ready tasks
            while let Some(mut task) = self.task_queue.pop_front() {
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

pub struct Waker {
    wake_queue: Arc<SegQueue<TaskId>>,
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
