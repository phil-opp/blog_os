pub(crate) use interrupt_wakeups::interrupt_wake;

pub mod executor;
mod interrupt_wakeups;

pub fn init() {
    interrupt_wakeups::init();
}
