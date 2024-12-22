#![no_std]
#![no_main]

mod vga_buffer;
//mod interrupts;


use core::panic::PanicInfo;

static HELLO: &[u8] = b"Hello World? Hmm..";

#[no_mangle]
pub extern "C" fn _start() -> ! {

    let mut writer = vga_buffer::Writer {
        column_position: 0,
        color_code: vga_buffer::ColorCode::new(vga_buffer::Color::Red, vga_buffer::Color::LightBlue),
        buffer: unsafe { &mut *(0xb8000 as *mut vga_buffer::Buffer) },
    };

    //vga_buffer::print_something();

    let mut i: i32 = 0;

    loop {
        println!("Hello World{}", "!");
    }
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("{}", _info);
    loop {}
}
