#![no_std]
#![no_main]
#![feature(panic_info_message)]

use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};
use core::{
    fmt::{self, Write},
    panic::PanicInfo,
};

const MESSAGE: &str = "Example panic message from panic_handler test";
const PANIC_LINE: u32 = 17; // adjust this when moving the `panic!` call

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("panic_handler... ");
    panic!(MESSAGE); // must be in line `PANIC_LINE`
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
    if location.line() != PANIC_LINE {
        fail("file line wrong");
    }
}

fn check_message(info: &PanicInfo) {
    let message = info.message().unwrap_or_else(|| fail("no message"));
    let mut compare_message = CompareMessage { equals: false };
    write!(&mut compare_message, "{}", message).unwrap_or_else(|_| fail("write failed"));
    if !compare_message.equals {
        fail("message not equal to expected message");
    }
}

/// Compares a `fmt::Arguments` instance with the `MESSAGE` string
///
/// To use this type, write the `fmt::Arguments` instance to it using the
/// `write` macro. If a message component matches `MESSAGE`, the equals
/// field is set to true.
struct CompareMessage {
    equals: bool,
}

impl fmt::Write for CompareMessage {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if s == MESSAGE {
            self.equals = true;
        }
        Ok(())
    }
}
