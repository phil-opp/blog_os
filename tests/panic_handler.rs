#![no_std]
#![no_main]
#![feature(panic_info_message)]

use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};
use core::{
    fmt::{self, Write},
    panic::PanicInfo,
};

const MESSAGE: &str = "Example panic message from panic_handler test";
static mut PANIC_LINE: u32 = 0;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("panic_handler... ");
    // The PANIC_LINE assignment and the panic macro invocation have
    // to be on the same line:
    unsafe { PANIC_LINE = line!(); } panic!(MESSAGE);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    check_location(info);
    check_message(info);

    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

fn fail(error: &str) -> ! {
    serial_println!("[failed]");
    serial_println!("{}", error);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

fn check_location(info: &PanicInfo) {
    let location = info.location().unwrap_or_else(|| fail("no location"));
    if location.file() != file!() {
        fail("file name wrong");
    }
    unsafe {
        if location.line() != PANIC_LINE {
            fail("file line wrong");
        }
    }
}

fn check_message(info: &PanicInfo) {
    let message = info.message().unwrap_or_else(|| fail("no message"));
    let mut compare_message = CompareMessage { expected: MESSAGE };
    write!(&mut compare_message, "{}", message).unwrap_or_else(|_| fail("write failed"));
    if compare_message.expected.len() != 0 {
        fail("message shorter than expected message");
    }
}

/// Compares a `fmt::Arguments` instance with the `MESSAGE` string
///
/// To use this type, write the `fmt::Arguments` instance to it using the
/// `write` macro. If the message component matches `MESSAGE`, the `expected`
/// field is the empty string.
struct CompareMessage {
    expected: &'static str,
}

impl fmt::Write for CompareMessage {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.expected.starts_with(s) {
            self.expected = &self.expected[s.len()..];
        } else {
            fail("message not equal to expected message");
        }
        Ok(())
    }
}
