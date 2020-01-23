use scheduler::Scheduler;

pub mod context_switch;
pub mod scheduler;
pub mod thread;

static SCHEDULER: spin::Mutex<Option<Scheduler>> = spin::Mutex::new(None);

pub fn invoke_scheduler() {
    let next = SCHEDULER
        .try_lock()
        .and_then(|mut scheduler| scheduler.as_mut().and_then(|s| s.schedule()));
    if let Some((next_id, next_stack_pointer)) = next {
        unsafe { context_switch::context_switch_to(next_id, next_stack_pointer) };
    }
}

pub fn with_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&mut Scheduler) -> T,
{
    f(SCHEDULER.lock().get_or_insert_with(Scheduler::new))
}
