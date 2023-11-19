#![no_std]
#![no_main]

use core::panic::PanicInfo;

static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            let line_offset: isize = 160 * 24;
            let char_offset_within_line: isize = i as isize * 2;
            let color_offset_within_line: isize = i as isize * 2 + 1;
            let char_offset = char_offset_within_line + line_offset;
            let color_offset = color_offset_within_line + line_offset;


            *vga_buffer.offset(char_offset) = byte;
            *vga_buffer.offset(color_offset) = 0xb;
        }
    }

    loop {}
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
