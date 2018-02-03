#![feature(lang_items)] // required for defining the panic handler
#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

#[no_mangle] // don't mangle the name of this function
pub fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start_` by default
    let vga_buffer = 0xb8000 as *const u8 as *mut u8;
    unsafe {
        *vga_buffer.offset(0) = b'H';
        *vga_buffer.offset(1) = 0xa; // foreground color green
        *vga_buffer.offset(2) = b'e';
        *vga_buffer.offset(3) = 0xa; // foreground color green
        *vga_buffer.offset(4) = b'l';
        *vga_buffer.offset(5) = 0xa;
        *vga_buffer.offset(6) = b'l';
        *vga_buffer.offset(7) = 0xa;
        *vga_buffer.offset(8) = b'o';
        *vga_buffer.offset(9) = 0xa;
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
