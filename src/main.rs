#![feature(lang_items)] // required for defining the panic handler
#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

//extern crate rlibc;

static HELLO: &[u8] = b"Hello World!";

#[no_mangle] // don't mangle the name of this function
pub fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start_` by default
    let vga_buffer = 0xb8000 as *const u8 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}

#[lang = "panic_fmt"] // define a function that should be called on panic
#[no_mangle] // TODO required?
pub extern fn rust_begin_panic(_msg: core::fmt::Arguments,
                               _file: &'static str,
                               _line: u32,
                               _column: u32) -> ! {
    loop {}
}
