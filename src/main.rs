#![feature(panic_implementation)] // required for defining the panic handler
#![no_std] // don't link the Rust standard library
#![cfg_attr(not(test), no_main)] // disable all Rust-level entry points
#![cfg_attr(test, allow(dead_code, unused_macros))] // allow unused code in test mode

extern crate spin;
extern crate volatile;
#[macro_use]
extern crate lazy_static;

#[cfg(test)]
extern crate array_init;
#[cfg(test)]
extern crate std;

#[cfg(not(test))]
use core::panic::PanicInfo;

#[macro_use]
mod vga_buffer;

/// This function is the entry point, since the linker looks for a function
/// named `_start_` by default.
#[cfg(not(test))]
#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    loop {}
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_implementation]
#[no_mangle]
pub fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
