#![no_std]
#![no_main]

use core::{convert::Infallible, panic::PanicInfo};

use bootloader_api::BootInfo;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::Point,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::{Rgb888, RgbColor},
    primitives::{Circle, PrimitiveStyle, StyledDrawable},
    text::Text,
    Drawable,
};

mod framebuffer;

bootloader_api::entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let mut display = framebuffer::Display::new(framebuffer);
        display.clear(Rgb888::RED).unwrap_or_else(infallible);
        let style = PrimitiveStyle::with_fill(Rgb888::YELLOW);
        Circle::new(Point::new(50, 50), 400)
            .draw_styled(&style, &mut display)
            .unwrap_or_else(infallible);

        let character_style = MonoTextStyle::new(&FONT_10X20, Rgb888::BLUE);
        let text = Text::new("Hello, world!", Point::new(190, 250), character_style);
        text.draw(&mut display).unwrap_or_else(infallible);
    }
    loop {}
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn infallible<T>(v: Infallible) -> T {
    match v {}
}
