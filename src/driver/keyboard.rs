use crate::{print, println, task::interrupt_wake};
use conquer_once::spin::OnceCell;
use core::future::Future;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

static WAKER: AtomicWaker = AtomicWaker::new();
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

pub fn init() {
    SCANCODE_QUEUE
        .try_init_once(|| ArrayQueue::new(10))
        .expect("failed to init scancode queue");
}

/// Called by the keyboard interrupt handler
///
/// Must not block (including spinlocks).
pub(crate) fn keyboard_scancode(scancode: u8) {
    let scancode_queue = SCANCODE_QUEUE
        .try_get()
        .expect("scancode queue not initialized");
    if let Err(_) = scancode_queue.push(scancode) {
        println!("WARNING: dropping keyboard input");
    }
    if let Some(waker) = WAKER.take() {
        interrupt_wake(waker);
    }
}

fn next_scancode() -> impl Future<Output = u8> {
    NextScancode
}

struct NextScancode;

impl Future for NextScancode {
    type Output = u8;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<u8> {
        let scancodes = SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");
        // fast path
        if let Ok(scancode) = scancodes.pop() {
            return Poll::Ready(scancode);
        }

        WAKER.register(&cx.waker());
        match scancodes.pop() {
            Ok(scancode) => Poll::Ready(scancode),
            Err(crossbeam_queue::PopError) => Poll::Pending,
        }
    }
}

pub async fn keyboard_task() {
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);

    loop {
        if let Ok(Some(key_event)) = keyboard.add_byte(next_scancode().await) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}
