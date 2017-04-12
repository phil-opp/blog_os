#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![no_std]

extern crate rlibc;

mod vga_buffer;

#[no_mangle]
pub extern fn rust_main() {
    // ATTENTION: we have a very small stack and no guard page

    vga_buffer::print_something();

    loop{}
}

#[lang = "eh_personality"] extern fn eh_personality() {}
#[lang = "panic_fmt"] #[no_mangle] pub extern fn panic_fmt() -> ! {loop{}}
