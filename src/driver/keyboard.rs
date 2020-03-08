use crate::{interrupts, print};
use core::future::Future;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

fn next_scancode() -> impl Future<Output = u8> {
    NextScancode
}

struct NextScancode;

impl Future for NextScancode {
    type Output = u8;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<u8> {
        let scancodes = interrupts::SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");
        // fast path
        if let Ok(scancode) = scancodes.pop() {
            return Poll::Ready(scancode);
        }

        interrupts::KEYBOARD_INTERRUPT_WAKER.register(&cx.waker());
        match scancodes.pop() {
            Ok(scancode) => Poll::Ready(scancode),
            Err(crossbeam_queue::PopError) => Poll::Pending,
        }
    }
}

pub async fn print_keypresses() {
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
