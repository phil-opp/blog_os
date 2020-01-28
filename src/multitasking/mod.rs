use scheduler::Scheduler;

pub mod context_switch;
pub mod scheduler;
pub mod thread;

static SCHEDULER: spin::Mutex<Option<Scheduler>> = spin::Mutex::new(None);

#[repr(u64)]
pub enum SwitchReason {
    Paused,
    Blocked,
    Exit,
}

pub fn invoke_scheduler() {
    let next = SCHEDULER
        .try_lock()
        .and_then(|mut scheduler| scheduler.as_mut().and_then(|s| s.schedule()));
    if let Some((next_stack_pointer, prev_thread_id)) = next {
        unsafe {
            context_switch::context_switch_to(
                next_stack_pointer,
                prev_thread_id,
                SwitchReason::Paused,
            )
        };
    }
}

pub fn exit_thread() -> ! {
    let next = with_scheduler(|s| s.schedule());
    match next {
        Some((next_stack_pointer, prev_thread_id)) => {
            unsafe {
                context_switch::context_switch_to(
                    next_stack_pointer,
                    prev_thread_id,
                    SwitchReason::Exit,
                )
            }
            unreachable!("finished thread continued")
        }
        None => panic!("can't exit last thread"),
    }
}

pub fn with_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&mut Scheduler) -> T,
{
    f(SCHEDULER.lock().get_or_insert_with(Scheduler::new))
}
