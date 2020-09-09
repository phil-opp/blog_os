use bootloader::boot_info::PixelFormat;
use core::{fmt, slice};
use font8x8::UnicodeFonts;
use spin::Mutex;
use volatile::Volatile;

pub static WRITER: Mutex<Option<Writer>> = Mutex::new(None);

pub fn init(framebuffer: &'static mut bootloader::boot_info::FrameBuffer) {
    let mut writer = Writer {
        info: framebuffer.info(),
        buffer: Volatile::new(framebuffer.buffer()),
        x_pos: 0,
        y_pos: 0,
    };
    writer.clear();

    // global writer should not be locked here
    let mut global_writer = WRITER.try_lock().unwrap();
    assert!(global_writer.is_none(), "Global writer already initialized");
    *global_writer = Some(writer);
}

pub struct Writer {
    buffer: Volatile<&'static mut [u8]>,
    info: bootloader::boot_info::FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
}

impl Writer {
    fn newline(&mut self) {
        self.y_pos += 8;
        self.carriage_return();
    }

    fn carriage_return(&mut self) {
        self.x_pos = 0;
    }

    /// Erases all text on the screen
    pub fn clear(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;
        self.buffer.fill(0);
    }

    fn width(&self) -> usize {
        self.info.horizontal_resolution
    }

    fn height(&self) -> usize {
        self.info.vertical_resolution
    }

    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                if self.x_pos >= self.width() {
                    self.newline();
                }
                if self.y_pos >= (self.height() - 8) {
                    self.clear();
                }
                let rendered = font8x8::BASIC_FONTS
                    .get(c)
                    .expect("character not found in basic font");
                self.write_rendered_char(rendered);
            }
        }
    }

    fn write_rendered_char(&mut self, rendered_char: [u8; 8]) {
        for (y, byte) in rendered_char.iter().enumerate() {
            for (x, bit) in (0..8).enumerate() {
                let on = *byte & (1 << bit) != 0;
                self.write_pixel(self.x_pos + x, self.y_pos + y, on);
            }
        }
        self.x_pos += 8;
    }

    fn write_pixel(&mut self, x: usize, y: usize, on: bool) {
        let pixel_offset = y * self.info.stride + x;
        let color = if on {
            match self.info.pixel_format {
                PixelFormat::RGB => [0x33, 0xff, 0x66, 0],
                PixelFormat::BGR => [0x66, 0xff, 0x33, 0],
                _other => [0xff, 0xff, 0xff, 0],
            }
        } else {
            [0, 0, 0, 0]
        };
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;
        self.buffer
            .index_mut(byte_offset..(byte_offset + bytes_per_pixel))
            .copy_from_slice(&color[..bytes_per_pixel]);
    }

    /// Writes the given ASCII string to the buffer.
    ///
    /// Wraps lines at `BUFFER_WIDTH`. Supports the `\n` newline character. Does **not**
    /// support strings with non-ASCII characters, since they can't be printed in the VGA text
    /// mode.
    fn write_string(&mut self, s: &str) {
        for char in s.chars() {
            self.write_char(char);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Like the `print!` macro in the standard library, but prints to the VGA text buffer.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::framebuffer::_print(format_args!($($arg)*)));
}

/// Like the `println!` macro in the standard library, but prints to the VGA text buffer.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// Prints the given formatted string to the VGA text buffer
/// through the global `WRITER` instance.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        WRITER.lock().as_mut().unwrap().write_fmt(args).unwrap();
    });
}

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}

#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}
