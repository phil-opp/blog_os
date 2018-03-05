#![feature(lang_items)] // required for defining the panic handler
#![feature(const_fn)] // allow declaring functions as const
#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

extern crate volatile;
extern crate rlibc;
extern crate spin;
#[macro_use]
extern crate lazy_static;

#[macro_use]
mod vga_buffer;

/// This function is the entry point, since the linker looks for a function
/// named `_start_` by default.
#[no_mangle] // don't mangle the name of this function
pub fn _start() -> ! {
    println!("Hello World{}", "!");

    loop {}
}

/// This function is called on panic.
#[lang = "panic_fmt"]
#[no_mangle]
pub extern fn rust_begin_panic(_msg: core::fmt::Arguments,
                               _file: &'static str,
                               _line: u32,
                               _column: u32) -> ! {
    loop {}
}
