---
layout: post
title: 'Printing to Screen'
category: 'rust-os'
---
In the [previous post] we switched from assembly to [Rust], a systems programming language that provides great safety. But so far we are using unsafe features like [raw pointers] whenever we want to print to screen. In this post we will create a Rust module that provides a safe and easy-to-use interface for the VGA text buffer. It will support Rust's [formatting macros], too.

[previous post]: {{ site.url }}{{ page.previous.url }}
[Rust]: https://www.rust-lang.org/
[raw pointers]: https://doc.rust-lang.org/book/raw-pointers.html
[formatting macros]: https://doc.rust-lang.org/std/fmt/#related-macros

This post uses recent unstable features, so you need an up-to-date nighly compiler. If you have any questions, problems, or suggestions please [file an issue] or create a comment at the bottom. The code from this post is also available on [Github][code repository].

[file an issue]: https://github.com/phil-opp/phil-opp.github.io/issues
[code repository]: https://github.com/phil-opp/blog_os/tree/printing_to_screen/src

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

## A basic Rust Module
Now that we know how the VGA buffer works, we can create a Rust module to handle printing. To create a new module named `vga_buffer`, we just need to create a file named `src/vga_buffer.rs` and add a `mod vga_buffer` line to `src/lib.rs`.

### Colors
First, we represent the different colors using an enum ([full file](#TODO)):

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

### The Text Buffer
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
To ensure that `ScreenChar` is exactly 16-bits, one might be tempted to use the [repr(packed)] attribute. But Rust does not insert any padding around two `u8` values, so it's not needed here. And using `repr(packed)` is generally discouraged because it can [cause undefined behavior][repr(packed) issue].

[repr(packed)]: https://doc.rust-lang.org/nightly/nomicon/other-reprs.html#reprpacked
[repr(packed) issue]: https://github.com/rust-lang/rust/issues/27060

To actually write to screen, we now create a writer type:

```rust
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: Unique<Buffer>,
}
```
The writer will always write to the last line and shift lines up when a line is full (or on `\n`). The `column_position` field keeps track of the current position in the last row. The current foreground and background colors are specified by `color_code` and a pointer to the VGA buffer is stored in `buffer`. To make it possible to create a `static` Writer later, the `buffer` field stores an `Unique<Buffer>` instead of a plain `*mut Buffer`. [Unique] is a wrapper that implements Send/Sync and is thus usable as a `static`.

[Unique]: https://doc.rust-lang.org/nightly/core/ptr/struct.Unique.html

## Printing to Screen
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

                self.buffer().chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code: self.color_code,
                };
                self.column_position += 1;
            }
        }
    }

    fn buffer(&mut self) -> &mut Buffer {
        unsafe{ self.buffer.get_mut() }
    }

    fn new_line(&mut self) {/* TODO */}
}
```
If the byte is the [newline] byte `\n`, the writer does not print anything. Instead it calls a `new_line` method, which we'll implement later. Other bytes get printed to the screen in the second match case.

[newline]: https://en.wikipedia.org/wiki/Newline

When printing a byte, the writer checks if the current line is full. In that case, a `new_line` call is required before to wrap the line. Then it writes a new `ScreenChar` to the buffer at the current position. Finally, the current column position is advanced.

The `buffer()` auxiliary method converts the raw pointer in the `buffer` field into a safe mutable buffer reference. The unsafe block is needed because the [get_mut()] method of `Unique` is unsafe. But our `buffer()` method itself isn't marked as unsafe, so it must not introduce any unsafety (e.g. cause segfaults). To guarantee that, it's very important that the `buffer` field always points to a valid `Buffer`. It's like a contract that we must stand to every time we create a `Writer`. To ensure that it's not possible to create an invalid `Writer` from outside of the module, the struct must have at least one private field and public creation functions are not allowed either.

[get_mut()]: https://doc.rust-lang.org/nightly/core/ptr/struct.Unique.html#method.get_mut

### Try it out!
To write some characters to the screen, you can create a temporary function:

```rust
pub fn print_something() {
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::LightGreen, Color::Black),
        buffer: Unique::new(0xb8000 as *mut _),
    }

    writer.write_byte(b'H');
}
```
It just creates a new Writer that points to the VGA buffer at `0xb8000`. Then it writes the byte `b'H'` to it. The `b` prefix creates a [byte character], which represents an ASCII code point. When we call `vga_buffer::print_something` in main, a `H` should be printed in the _lower_ left corner of the screen in light green.

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
You can try it yourself in the `print_something` function. When you print strings with some special characters like `ä` or `λ`, you'll notice that they cause weird symbols on screen. That's because they are represented by multiple bytes in [UTF-8]. By converting them to bytes, we of course get strange results. But since the VGA buffer doesn't support UTF-8, it's not possible to display these characters anyway. To ensure that a string contains only ASCII characters, you can prefix a `b` to create a [Byte String].

[UTF-8]: http://www.fileformat.info/info/unicode/utf8.htm
[Byte String]: https://doc.rust-lang.org/reference.html#characters-and-strings

### Support Formatting Macros
It would be nice to support Rust's formatting macros, too. That way, we can easily print different types like integers or floats. To support them, we need to implement the [core::fmt::Write] trait. The only required method of this trait is `write_str` that looks quite similar to our `write_str` method. To implement the trait, we just need to move it into an `impl ::core::fmt::Write for Writer` block and add a return type:

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
The `Ok(())` is just a `Ok` Result containing the `()` type. We can drop the `pub` because trait methods are always public.

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

### Newlines
Right now, we just ignore newlines and characters that don't fit into the line anymore. Instead we want to move every character one line up (the top line gets deleted) and start at the beginning of the last line again. To do this, we add an implementation for the `new_line` method of `Writer`:

```rust
fn new_line(&mut self) {
    for row in 0..(BUFFER_HEIGHT-1) {
        let buffer = self.buffer();
        buffer.chars[row] = buffer.chars[row + 1]
    }
    self.clear_row(BUFFER_HEIGHT-1);
    self.column_position = 0;
}

fn clear_row(&mut self) {/* see below */}
```
We just move each line to the line above. Notice that the range notation (`..`) is exclusive the upper bound.

The `clear_row` method looks like this:

```rust
fn clear_row(&mut self, row: usize) {
    let blank = ScreenChar {
        ascii_character: b' ',
        color_code: self.color_code,
    };
    self.buffer().chars[row] = [blank; BUFFER_WIDTH];
}
```

## Providing an Interface/Printing from outside/Synchronization/...
To provide a global writer that can used as an interface from other modules, we can add a `static` writer:

```rust
pub static WRITER: Writer = Writer {
    column_position: 0,
    color_code: ColorCode::new(Color::LightGreen, Color::Black),
    buffer: Unique::new(0xb8000 as *mut _),
};
```

This won't work! You can try it yourself in the `print_something` function. The reason is that we try to take a mutable reference (`&mut`) to a immutable `static` when calling `WRITER.print_byte`.

To resolve it, we could use a [mutable static]. But then every read and write to it would be unsafe since it could easily introduce data races and other bad things. Using `static mut` is highly discouraged, there are even proposals to [remove it][remove static mut].

[mutable static]: https://doc.rust-lang.org/book/const-and-static.html#mutability
[remove static mut]: https://internals.rust-lang.org/t/pre-rfc-remove-static-mut/1437

But what are the alternatives? We could try to use a cell type like [RefCell] or even [UnsafeCell] to provide [interior mutability]. But these types aren't [Sync] (with good reason), so we can't use them in statics.

[RefCell]: https://doc.rust-lang.org/nightly/core/cell/struct.RefCell.html
[UnsafeCell]: https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html
[interior mutability]: https://doc.rust-lang.org/book/mutability.html#interior-vs.-exterior-mutability
[Sync]: https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html

To get synchronized interior mutability, users of the standard library can use [Mutex]. It provides mutual exclusion by blocking threads when the resource is already locked. But our basic kernel does not have any blocking support or even a concept of threads, so we can't use it either. However there is a really basic kind of mutex in computer science that requires no operating system features: the [spinlock]. Instead of blocking, the threads simply try to lock it again and again in a tight loop and thus burn CPU time until the mutex is free again.

[Mutex]: https://doc.rust-lang.org/nightly/std/sync/struct.Mutex.html
[spinlock]: https://en.wikipedia.org/wiki/Spinlock

To use a spinning mutex, we can add the [spin crate] as a dependency in Cargo.toml:

[spin crate]: https://crates.io/crates/spin

```toml
...
[dependencies]
rlibc = "*"
spin = "*"
```
and a `extern crate spin;` definition in `src/lib.rs`. Then we can use the spinning Mutex to provide interior mutability to our static writer:

```rust
use spin::Mutex;
...
pub static WRITER: Mutex<Writer> = Mutex::new(Writer {
    column_position: 0,
    color_code: ColorCode::new(Color::LightGreen, Color::Black),
    buffer: Unique::new(0xb8000 as *mut _),
});
```
[Mutex::new] is a const function, too, so it can be used in statics.

Now we can easily print from our main function:

[Mutex::new]: https://mvdnes.github.io/rust-docs/spinlock-rs/spin/struct.Mutex.html#method.new

```rust
pub extern fn rust_main() {
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again");
    write!(vga_buffer::WRITER.lock(), ", some numbers: {} {}", 42, 1.337);
    loop{}
}
```
Note that we need to import the `Write` trait if we want to use its functions.

## A println macro
Rust's [macro syntax] is a bit strange, so we won't try to write a macro from scratch. Instead we look at the source of the [`println!` macro] in the standard library:

[macro syntax]: https://doc.rust-lang.org/nightly/book/macros.html
[`println!` macro]: https://doc.rust-lang.org/nightly/std/macro.println!.html

```
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}
```
It just adds a `\n` and then invokes the [`print!` macro], which is defined as:

[`print!` macro]: https://doc.rust-lang.org/nightly/std/macro.print!.html

```
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
```
It calls the `_print` method in the `io` module of the current crate (`$crate`), which is `std`. The [`_print` function] in libstd is rather complicated, as it supports different `Stdout` devices.

[`_print` function]: https://doc.rust-lang.org/nightly/src/std/io/stdio.rs.html#578

To print to the VGA buffer, we just copy the `println!` macro and modify the `print!` macro to use our static `WRITER` instead of `_print`:

```
macro_rules! print {
    ($($arg:tt)*) => ({
            use core::fmt::Write;
            $crate::vga_buffer::WRITER.lock().write_fmt(format_args!($($arg)*)).unwrap();
    });
}
```
Instead of a `_print` function, we call the `write_fmt` method of our static `Writer`. Since we're using a method from the `Write` trait, we need to import it before. The additional `unwrap()` at the end panics if printing isn't successful. But since we always return `Ok` in `write_str`, that should not happen.

Notice the additional scope around the macro: It's `=> ({…})` instead of `=> (…)`. The additional `{}` avoid a silent import of the `Write` trait on macro expansion.

### Clearing the screen
We can now use `println!` to add a rather trivial function to clear the screen:

```rust
pub fn clear_screen() {
    for _ in 0..BUFFER_HEIGHT {
        println!("");
    }
}
```

## What's next?
Soon we will tackle virtual memory management and map the kernel sections correctly. This will cause many strange bugs and boot loops. To understand what's going on a real debugger is indispensable. In the [next post] we will setup [GDB] to work with QEMU.
