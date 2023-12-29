use bootloader_api::info::{FrameBuffer, PixelFormat};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{self, Point},
    pixelcolor::{Rgb888, RgbColor},
    Pixel,
};

pub struct Display {
    framebuffer: &'static mut FrameBuffer,
    width: usize,
    height: usize,
}

impl Display {
    pub fn new(framebuffer: &'static mut FrameBuffer) -> Display {
        Self {
            width: framebuffer.info().width,
            height: framebuffer.info().height,
            framebuffer,
        }
    }

    fn draw_pixel(&mut self, coordinates: Point, color: Rgb888) {
        // ignore any pixels that are out of bounds.
        let position = match (coordinates.x.try_into(), coordinates.y.try_into()) {
            (Ok(x), Ok(y)) if x < self.width && y < self.height => Position { x, y },
            _ => return, // ignore out-of-bounds pixel
        };
        let color = Color {
            red: color.r(),
            green: color.g(),
            blue: color.b(),
        };
        set_pixel_in(self.framebuffer, position, color);
    }
}

impl DrawTarget for Display {
    type Color = Rgb888;

    /// Drawing operations can never fail.
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coordinates, color) in pixels.into_iter() {
            self.draw_pixel(coordinates, color);
        }
        Ok(())
    }
}

impl geometry::OriginDimensions for Display {
    fn size(&self) -> geometry::Size {
        geometry::Size::new(
            self.width.try_into().unwrap(),
            self.height.try_into().unwrap(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

pub fn set_pixel_in(framebuffer: &mut FrameBuffer, position: Position, color: Color) {
    let info = framebuffer.info();

    // calculate offset to first byte of pixel
    let byte_offset = {
        // use stride to calculate pixel offset of target line
        let line_offset = position.y * info.stride;
        // add x position to get the absolute pixel offset in buffer
        let pixel_offset = line_offset + position.x;
        // convert to byte offset
        pixel_offset * info.bytes_per_pixel
    };

    // set pixel based on color format
    let pixel_buffer = &mut framebuffer.buffer_mut()[byte_offset..];
    match info.pixel_format {
        PixelFormat::Rgb => {
            pixel_buffer[0] = color.red;
            pixel_buffer[1] = color.green;
            pixel_buffer[2] = color.blue;
        }
        PixelFormat::Bgr => {
            pixel_buffer[0] = color.blue;
            pixel_buffer[1] = color.green;
            pixel_buffer[2] = color.red;
        }
        PixelFormat::U8 => {
            // use a simple average-based grayscale transform
            let gray = color.red / 3 + color.green / 3 + color.blue / 3;
            pixel_buffer[0] = gray;
        }
        other => panic!("unknown pixel format {other:?}"),
    }
}
