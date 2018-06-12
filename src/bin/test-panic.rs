#![feature(panic_implementation)]
#![feature(const_fn)]
#![no_std]
#![cfg_attr(not(test), no_main)]

#[macro_use]
extern crate blog_os;

use blog_os::exit_qemu;
#[cfg(not(test))]
use core::panic::PanicInfo;

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    panic!();
}

#[cfg(not(test))]
#[panic_implementation]
#[no_mangle]
pub fn panic(_info: &PanicInfo) -> ! {
    serial_println!("ok");

    unsafe {
        exit_qemu();
    }
    loop {}
}
