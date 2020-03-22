use super::Task;
use crate::println;
use alloc::collections::{BTreeMap, VecDeque};
use conquer_once::spin::OnceCell;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use crossbeam_queue::ArrayQueue;

static WAKE_QUEUE: OnceCell<ArrayQueue<usize>> = OnceCell::uninit();

pub struct SimpleExecutor {
    task_queue: VecDeque<Task>,
    waiting_tasks: BTreeMap<usize, Task>,
}

impl SimpleExecutor {
    pub fn new() -> SimpleExecutor {
        WAKE_QUEUE.init_once(|| ArrayQueue::new(100));
        SimpleExecutor {
            task_queue: VecDeque::new(),
            waiting_tasks: BTreeMap::new(),
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
        while let Ok(task_id) = WAKE_QUEUE.get().unwrap().pop() {
            if let Some(task) = self.waiting_tasks.remove(&task_id) {
                self.task_queue.push_back(task);
            }
        }
    }

    fn run_ready_tasks(&mut self) {
        while let Some(mut task) = self.task_queue.pop_front() {
            let waker = waker(task.id());
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

fn raw_waker(task_id: usize) -> RawWaker {
    fn clone(id: *const ()) -> RawWaker {
        raw_waker(id as usize)
    }

    fn wake(id: *const ()) {
        if let Err(_) = WAKE_QUEUE.try_get().unwrap().push(id as usize) {
            println!("WARNING: WAKE_QUEUE full; dropping wakeup")
        }
    }

    fn drop(_id: *const ()) {}

    RawWaker::new(
        task_id as *const (),
        &RawWakerVTable::new(clone, wake, wake, drop),
    )
}

fn waker(task_id: usize) -> Waker {
    unsafe { Waker::from_raw(raw_waker(task_id)) }
}
