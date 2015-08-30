---
layout: post
title: 'Printing to Screen'
category: 'rust-os'
---
TODO Introduction

## The VGA Text Buffer
The text buffer starts at physical address `0xb8000` and contains the characters displayed on screen. It has 80 rows and 25 columns. Each screen character has the following format:

Bit(s) | Value
------ | ----------------
0-7    | ASCII code point
8-11   | Foreground color
12-14  | Background color
15     | Blink
The following colors are available:

Number | Color      | Number + Bright Bit | Bright Color
------ | ---------- | ------------------- | -------------
0x0    | Black      | 0x8                 | Dark Gray
0x1    | Blue       | 0x9                 | Light Blue
0x2    | Green      | 0xa                 | Light Green
0x3    | Cyan       | 0xb                 | Light Cyan
0x4    | Red        | 0xc                 | Light Red
0x5    | Magenta    | 0xd                 | Light Magenta
0x6    | Brown      | 0xe                 | Yellow
0x7    | Light Gray | 0xf                 | White
Bit 4 is the _bright bit_, which is unavailable in background color as it's the blink bit. But it's possible to disable blinking through a [BIOS function][disable blinking] and use the full 16 colors as background.

[disable blinking]: http://www.ctyme.com/intr/rb-0117.htm

## Creating a Rust Module
Let's create the Rust module `vga_buffer`. Therefor we create a file named `src/vga_buffer.rs` and add a `mod vga_buffer` line to `src/lib.rs`. Now we can create an enum for the colors ([full file](#TODO)):

```rust
#[repr(u8)]
pub enum Color {
    Black      = 0,
    Blue       = 1,
    ...
    Yellow     = 14,
    White      = 15,
}

struct ColorCode(u8);

impl ColorCode {
    const fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}
```
We use the `repr(u8)` attribute to represent each variant of `Color` as an `u8`. The `ColorCode` contains the full color byte, containing foreground and background color. The `new` function is a [const function] to allow it in static initializers. It's unstable so we need to add the `const_fn` feature in `src/lib.rs`. We ignore the blink bit here to keep the code short. Now we can represent a screen character and the text buffer:

```rust
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```
Now we can represent the actual screen writer:

```rust
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
}

impl Writer {
    pub const fn new(foreground: Color, background: Color) -> Writer {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(foreground, background),
        }
    }
}
```
It's `pub` because it's part of the public interface and `const` to allow it in static initializers. The plan is to write always to the last line and shift lines up when a line is full (or on `\n`). So we just need to store the current column position and the current `ColorCode`.

### Writing to screen
Now we can create a `write_byte` function:

```rust
impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        const NEWLINE: u8 = '\n' as u8;
        const BUFFER: *mut Buffer = 0xb8000 as *mut _;

        match byte {
            NEWLINE => {}, // TODO
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    //TODO
                }
                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                Self::buffer().chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code: self.color_code,
                };
                self.column_position += 1;
            }
        }
    }
}
```
Some Notes:

- We write a new `ScreenChar` to the current field in the buffer and increase the column position
- We take `&mut self` as we modify the column position
- `Self` refers to the `Writer` type

The `buffer()` function just converts the raw pointer to a mutable reference. We need the `unsafe` block because Rust doesn't know if the pointer is valid. It looks like this:

```rust
impl Writer {
    fn buffer() -> &'static mut Buffer {
        const BUFFER: *mut Buffer = 0xb8000 as *mut _;
        unsafe{&mut *BUFFER}
    }
}
```
Now we can test it in `main`:

```rust
pub extern fn main() {
    use vga_buffer::{Writer, Color};
    ...
    let mut writer = Writer::new(Color::Blue, Color::LightGreen);
    writer.write_byte(b'H');
}
```
The `b'H'` is a [byte character] and represents the ASCII code point for `H`. It should be printed in the _lower_ left corner of the screen on `make run`.

To print whole strings, we can convert them to bytes[^utf8-problems] and print them one-by-one:

```rust
pub fn write_str(&mut self, s: &str) {
    for byte in s.bytes() {
      self.write_byte(byte)
    }
}
```
[byte character]: https://doc.rust-lang.org/reference.html#characters-and-strings
[^utf8-problems]: This approach works well for strings that contain only ASCII characters. For other Unicode characters, however, we get weird symbols on the screen. But they can't be printed in the VGA text buffer anyway.

## Support Formatting Macros
It would be nice to support Rust's formatting macros, too. That way, we can easily print different types like integers or floats. To support them, we need to implement the [core::fmt::Write] trait. The only required method of this trait is `write_str` and looks quite similar to our `write_str` method. We just need to move it into an `impl ::core::fmt::Write for Writer` block and add a return type:

```rust
impl ::core::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        for byte in s.bytes() {
          self.write_byte(byte)
        }
        Ok(())
    }
}
```
The `Ok(())` is just the `Ok` Result containing the `()` type. We can drop the `pub` because trait methods are always public.

Now we can use Rust's built-in `write!`/`writeln!` formatting macros:

```rust
...
let mut writer = Writer::new(Color::Blue, Color::LightGreen);
writer.write_byte(b'H');
writer.write_str("ello! ");
write!(writer, "The numbers are {} and {}", 42, 1.0/3.0);
```
Now you should see a `Hello! The numbers are 42 and 0.3333333333333333` in strange colors at the bottom of the screen.

[core::fmt::Write]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

## Newlines
Right now, we just ignore newlines and characters that don't fit into the line anymore. Instead we want to move every character one line up (the top line gets deleted) and start at the beginning of the last line again. To do this, we add a `new_line` method to `Writer`:

```rust
```
Blablabla

Now we just need to call in the 2 cases marked with `//TODO` and our writer supports newlines.
