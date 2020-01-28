use scheduler::Scheduler;

pub mod context_switch;
pub mod scheduler;
pub mod thread;

static SCHEDULER: spin::Mutex<Option<Scheduler>> = spin::Mutex::new(None);

#[repr(u64)]
pub enum SwitchReason {
    Paused,
    Yield,
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
    synchronous_context_switch(SwitchReason::Exit).expect("can't exit last thread");
    unreachable!("finished thread continued");
}

pub fn yield_now() {
    let _ = synchronous_context_switch(SwitchReason::Yield);
}

fn synchronous_context_switch(reason: SwitchReason) -> Result<(), ()> {
    let next = with_scheduler(|s| s.schedule());
    match next {
        Some((next_stack_pointer, prev_thread_id)) => unsafe {
            context_switch::context_switch_to(next_stack_pointer, prev_thread_id, reason);
            Ok(())
        },
        None => Err(()),
    }
}

pub fn with_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&mut Scheduler) -> T,
{
    f(SCHEDULER.lock().get_or_insert_with(Scheduler::new))
}
