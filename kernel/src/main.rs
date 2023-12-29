#![no_std]
#![no_main]

use core::panic::PanicInfo;

use bootloader_api::BootInfo;

mod framebuffer;

bootloader_api::entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let color = framebuffer::Color {
            red: 0,
            green: 0,
            blue: 255,
        };
        for x in 0..100 {
            for y in 0..100 {
                let position = framebuffer::Position {
                    x: 20 + x,
                    y: 100 + y,
                };
                framebuffer::set_pixel_in(framebuffer, position, color);
            }
        }
    }
    loop {}
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
