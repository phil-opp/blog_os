#![no_std]
#![no_main]

use core::panic::PanicInfo;

static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            // For me qemu setup is as follows:
            // - line width is 160
            // - last visible line: 24
            // - first fully visible line: 2
            // For you it might differ, so if text is not visible, play around with `line_width` and `line_offset`
            let line_width: isize = 160;
            let line_offset: isize = line_width * 23;
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
