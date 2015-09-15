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

Bit 4 is the _bright bit_, which turns for example blue into light blue. It is unavailable in background color as the bit is used to enable blinking. However, it's possible to disable blinking through a [BIOS function][disable blinking]. Then the full 16 colors can be used as background.

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
```
We use a [C-like enum] here to explicitly specify the number for each color. Because of the `repr(u8)` attribute each enum variant is stored as an `u8`. Actually 4 bits would be sufficient, but Rust doesn't have an `u4` type.

[C-like enum]: http://rustbyexample.com/custom_types/enum/c_like.html

To represent a full color code that specifies foreground and background color, we create a [newtype] on top of `u8`:

[newtype]: https://aturon.github.io/features/types/newtype.html

```rust
struct ColorCode(u8);

impl ColorCode {
    const fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}
```
The `ColorCode` contains the full color byte, containing foreground and background color. Blinking is enabled implicitly by using a bright background color (soon we will disable blinking anyway). The `new` function is a [const function] to allow it in static initializers. As `const` functions are unstable we need to add the `const_fn` feature in `src/lib.rs`.

[const function]: https://github.com/rust-lang/rfcs/blob/master/text/0911-const-fn.md

Now we can add structures to represent a screen character and the text buffer:

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
To ensure that `ScreenChar` is exactly 16-bits, one might be tempted to use the [repr(packed)] attribute. But Rust does not insert any padding around two `u8` values, so it's not needed here. And `repr(packed)` can cause [undefined behavior][repr(packed) issue] and that's always bad.

[repr(packed)]: https://doc.rust-lang.org/nightly/nomicon/other-reprs.html#repr(packed)
[repr(packed) issue]: https://github.com/rust-lang/rust/issues/27060

To actually write to screen, we now create a writer type:

```rust
pub struct Writer<'a> {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'a mut Buffer,
}
```
The explicit lifetime tells Rust that the writer lives as long as the mutual buffer reference. Thus Rust ensures statically that the writer does not write to invalid memory.

The writer will always write to the last line and shift lines up when a line is full (or on `\n`). So the current row is always the last row and just the current column position needs to be stored. The current foreground and background colors are specified by `color_code`.

### Printing Characters
Now we can use the `Writer` to modify the buffer's characters. First we create a method to write a single ASCII byte:

```rust
impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.new_line();
            },
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }
                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code: self.color_code,
                };
                self.column_position += 1;
            }
        }
    }
    fn new_line(&mut self) {}
}
```
If the byte is the [newline] byte `\n`, the writer does not print anything. Instead it calls a `new_line` method, which we'll implement later. Other bytes get printed to the screen in the second match case.

When printing a byte, the writer checks if the current line is full. In that case, a `new_line` call is required before to wrap the line. Since the writer always writes to the last line, `row` is just the last line's index. The writer uses the mutable reference stored in `buffer` to set the screen character at `row` and `col`. Then the column position is advanced by one.

[newline]: https://en.wikipedia.org/wiki/Newline

To test it, we add can add a `test` function to the module:

```rust
pub fn test() {
    const BUFFER: *mut Buffer = 0xb8000 as *mut _;
    let buffer = unsafe{&mut *BUFFER};

    let writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::LightGreen, Color::Black),
        buffer: buffer,
    };
    writer.write_byte(b'H');
}
```
First, we create a mutable reference to the VGA text buffer at `0xb8000`. The `const` defines a raw pointer that we convert to a reference using the [`&mut *` pattern][references and raw pointers]. The `unsafe` block is needed because Rust doesn't know if the raw pointer is valid. Notice that creating a raw pointer is completely safe, only dereferencing it is `unsafe`. After creating the reference, we use it to create a new writer.

Finally, we write the byte `b'H'`. The `b` in front specifies that it's a [byte character], which represents an ASCII code point. When we call `vga_buffer::test` in main, it's printed in the _lower_ left corner of the screen in light green.

[references and raw pointers]: https://doc.rust-lang.org/book/raw-pointers.html#references-and-raw-pointers
[byte character]: https://doc.rust-lang.org/reference.html#characters-and-strings

### Printing Strings

To print whole strings, we can convert them to bytes and print them one-by-one:

```rust
pub fn write_str(&mut self, s: &str) {
    for byte in s.bytes() {
      self.write_byte(byte)
    }
}
```
You can try it yourself in the `test` function. When you try strings with some special characters like `ä` or `λ`, you'll notice that they cause weird symbols on screen. That's because they are represented by multiple bytes in [UTF-8]. By converting them to bytes, we of course get strange results. But since the VGA buffer doesn't support UTF-8, it's not possible to display these characters anyway. So let's just stick to ASCII strings for now.

[UTF-8]: http://www.fileformat.info/info/unicode/utf8.htm

## Providing an Interface

## Synchronization

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
fn new_line(&mut self) {
    for row in 0..(BUFFER_HEIGHT-1) {
        Self::buffer().chars[row] = Self::buffer().chars[row + 1]
    }
    self.clear_row(BUFFER_HEIGHT-1);
    self.column_position = 0;
}
```
We just move each line to the line above. Notice that the range notation (`..`) is exclusive the upper bound.

The `clear_row` method looks like this:

```rust
fn clear_row(&mut self, row: usize) {
    let blank = ScreenChar {
        ascii_character: b' ',
        color_code: self.color_code,
    };
    Self::buffer().chars[row] = [blank; BUFFER_WIDTH];
}
```
Now we just need to call the `new_line()` method in the 2 cases marked with `//TODO` and our writer supports newlines.

## A `println!` macro
Rust's [macro syntax] is a bit strange, so we won't try to write a macro from scratch. Instead we look at the source of the [`println!` macro] in the standard library:

```
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}
```
It just refers to the [`print!` macro] that is defined as:

```
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
```
It just calls the _print_ method in the `io` module of the current crate (`$crate`), which is `std`. The [`_print` function] is rather complicated, as it supports different `Stdout`s.

To print to the VGA buffer, we just copy both macros and replace the `io` module with the `vga_buffer` buffer in the `print!` macro:

```
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}
```
Now we can write our own `_print` function:

```rust
pub fn _print(fmt: ::core::fmt::Arguments) {
    use core::fmt::Write;
    static mut WRITER: Writer = Writer::new(Color::LightGreen, Color::Black);
    unsafe{WRITER.write_fmt(fmt)};
}
```
The function needs to be public because every `print!(…)` is expanded to `::vga_buffer::_print(…)`. It uses a `static mut` to store a writer and calls the `write_fmt` method of the `core::fmt::Write` trait (hence the import). It's highly discouraged to use `static mut`s because they introduce all kinds of data races (that's why every access is unsafe). We use it here anyway, as we have only a single thread at the moment. But we already have another data race: We can create multiple `Writer`s, that write to the same memory at `0xb8000`. So as soon as we add multithreading, we need to revisit this module again and find better solutions.

[macro syntax]: https://doc.rust-lang.org/nightly/book/macros.html
[`println!` macro]: https://doc.rust-lang.org/nightly/std/macro.println!.html
[`print!` macro]: https://doc.rust-lang.org/nightly/std/macro.print!.html

## Clearing the screen
We can now add a rather trivial last function:

```
pub fn clear_screen() {
    for _ in 0..BUFFER_HEIGHT {
        println!("");
    }
}
```

## What's next?
Soon we will tackle virtual memory management and map the kernel sections correctly. This will cause many strange bugs and boot loops. To understand what's going on a real debugger is indispensable. In the [next post] we will setup [GDB] to work with QEMU.
