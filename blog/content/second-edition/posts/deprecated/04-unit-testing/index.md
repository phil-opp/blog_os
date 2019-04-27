+++
title = "Unit Testing"
weight = 4
path = "unit-testing"
date  = 2018-04-29

[extra]
warning_short = "Deprecated: "
warning = "This post is deprecated in favor of the [_Testing_](/testing) post and will no longer receive updates."
+++

This post explores unit testing in `no_std` executables using Rust's built-in test framework. We will adjust our code so that `cargo test` works and add some basic unit tests to our VGA buffer module.

<!-- more -->

This blog is openly developed on [GitHub]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom]. The complete source code for this post can be found in the [`post-04`][post branch] branch.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-04

<!-- toc -->

## Requirements

In this post we explore how to execute `cargo test` on the host system (as a normal Linux/Windows/macOS executable). This only works if you don't have a `.cargo/config` file that sets a default target. If you followed the [_Minimal Rust Kernel_] post before 2019-04-27, you should be fine. If you followed it after that date, you need to remove the `build.target` key from your `.cargo/config` file and explicitly pass a target argument to `cargo xbuild`.

[_Minimal Rust Kernel_]: ./second-edition/posts/02-minimal-rust-kernel/index.md

Alternatively, consider reading the new [_Testing_] post instead. It sets up a similar functionality as this post, but instead of running the tests on your host system, they are run in a realistic environment inside QEMU.

[_Testing_]: ./second-edition/posts/04-testing/index.md

## Unit Tests for `no_std` Binaries
Rust has a [built-in test framework] that is capable of running unit tests without the need to set anything up. Just create a function that checks some results through assertions and add the `#[test]` attribute to the function header. Then `cargo test` will automatically find and execute all test functions of your crate.

[built-in test framework]: https://doc.rust-lang.org/book/second-edition/ch11-00-testing.html

Unfortunately it's a bit more complicated for `no_std` applications such as our kernel. If we run `cargo test` (without adding any test yet), we get the following error:

```
> cargo test
   Compiling blog_os v0.2.0 (file:///…/blog_os)
error[E0152]: duplicate lang item found: `panic_impl`.
  --> src/main.rs:35:1
   |
35 | / fn panic(info: &PanicInfo) -> ! {
36 | |     println!("{}", info);
37 | |     loop {}
38 | | }
   | |_^
   |
   = note: first defined in crate `std`.
```

The problem is that unit tests are built for the host machine, with the `std` library included. This makes sense because they should be able to run as a normal application on the host operating system. Since the standard library has it's own `panic_handler` function, we get the above error. To fix it, we use [conditional compilation] to include our implementation of the panic handler only in non-test environments:

[conditional compilation]: https://doc.rust-lang.org/reference/attributes.html#conditional-compilation


```rust
// in src/main.rs

use core::panic::PanicInfo;

#[cfg(not(test))] // only compile when the test flag is not set
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
```

The only change is the added `#[cfg(not(test))]` attribute. The `#[cfg(…)]` attribute ensures that the annotated item is only included if the passed condition is met. The `test` configuration is set when the crate is compiled for unit tests. Through `not(…)` we negate the condition so that the language item is only compiled for non-test builds.

When we now try `cargo test` again, we get an ugly linker error:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" "-Wl,--as-needed" "-Wl,-z,noexecstack" "-m64" "-L" "/…/lib/rustlib/x86_64-unknown-linux-gnu/lib" […]
  = note: /…/blog_os-969bdb90d27730ed.2q644ojj2xqxddld.rcgu.o: In function `_start':
          /…/blog_os/src/main.rs:17: multiple definition of `_start'
          /usr/lib/gcc/x86_64-linux-gnu/5/../../../x86_64-linux-gnu/Scrt1.o:(.text+0x0): first defined here
          /usr/lib/gcc/x86_64-linux-gnu/5/../../../x86_64-linux-gnu/Scrt1.o: In function `_start':
          (.text+0x20): undefined reference to `main'
          collect2: error: ld returned 1 exit status

```

I shortened the output here because it is extremely verbose. The relevant part is at the bottom, after the second “note:”. We got two distinct errors here, “_multiple definition of `_start`_” and “_undefined reference to `main`_”.

The reason for the first error is that the test framework injects its own `main` and `_start` functions, which will run the tests when invoked. So we get two functions named `_start` when compiling in test mode, one from the test framework and the one we defined ourselves. To fix this, we need to exclude our `_start` function in that case, which we can do by marking it as `#[cfg(not(test))]`:

```rust
// in src/main.rs

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! { … }
```

The second problem is that we use the `#![no_main]` attribute for our crate, which suppresses any `main` generation, including the test `main`. To solve this, we use the [`cfg_attr`] attribute to conditionally enable the `no_main` attribute only in non-test mode:

[`cfg_attr`]: https://chrismorgan.info/blog/rust-cfg_attr.html

```rust
// in src/main.rs

#![cfg_attr(not(test), no_main)] // instead of `#![no_main]`
```

Now `cargo test` works:

```
> cargo test
   Compiling blog_os v0.2.0 (file:///…/blog_os)
    [some warnings]
    Finished dev [unoptimized + debuginfo] target(s) in 0.98 secs
     Running target/debug/deps/blog_os-1f08396a9eff0aa7

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The test framework seems to work as intended. We don't have any tests yet, but we already get a test result summary.

### Silencing the Warnings
We get a few warnings about unused imports, because we no longer compile our `_start` function. To silence such unused code warnings, we can add the following to the top of our `main.rs`:

```
#![cfg_attr(test, allow(unused_imports))]
```

Like before, the `cfg_attr` attribute sets the passed attribute if the passed condition holds. Here, we set the `allow(…)` attribute when compiling in test mode. We use the `allow` attribute to disable warnings for the `unused_import` _lint_.

Lints are classes of warnings, for example `dead_code` for unused code or `missing-docs` for missing documentation. Lints can be set to four different states:

- `allow`: no errors, no warnings
- `warn`: causes a warning
- `deny`: causes a compilation error
- `forbid`: like `deny`, but can't be overridden

Some lints are `allow` by default (such as `missing-docs`), others are `warn` by default (such as `dead_code`), and some few are even `deny` by default.. The default can be overridden by the `allow`, `warn`, `deny` and `forbid` attributes. For a list of all lints, see `rustc -W help`. There is also the [clippy] project, which provides many additional lints.

[clippy]: https://github.com/rust-lang-nursery/rust-clippy

### Including the Standard Library
Unit tests run on the host machine, so it's possible to use the complete standard library inside them. To link the standard library in test mode, we can make the `#![no_std]` attribute conditional through `cfg_attr` too:

```diff
-#![no_std]
+#![cfg_attr(not(test), no_std)]
```

## Testing the VGA Module
Now that we have set up the test framework, we can add a first unit test for our `vga_buffer` module:

```rust
// in src/vga_buffer.rs

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn foo() {}
}
```

We add the test in an inline `test` submodule. This isn't necessary, but a common way to separate test code from the rest of the module. By adding the `#[cfg(test)]` attribute, we ensure that the module is only compiled in test mode. Through `use super::*`, we import all items of the parent module (the `vga_buffer` module), so that we can test them easily.

The `#[test]` attribute on the `foo` function tells the test framework that the function is an unit test. The framework will find it automatically, even if it's private and inside a private module as in our case:

```
> cargo test
   Compiling blog_os v0.2.0 (file:///…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 2.99 secs
     Running target/debug/deps/blog_os-1f08396a9eff0aa7

running 1 test
test vga_buffer::test::foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

We see that the test was found and executed. It didn't panic, so it counts as passed.

### Constructing a Writer
In order to test the VGA methods, we first need to construct a `Writer` instance. Since we will need such an instance for other tests too, we create a separate function for it:

```rust
// in src/vga_buffer.rs

#[cfg(test)]
mod test {
    use super::*;

    fn construct_writer() -> Writer {
        use std::boxed::Box;

        let buffer = construct_buffer();
        Writer {
            column_position: 0,
            color_code: ColorCode::new(Color::Blue, Color::Magenta),
            buffer: Box::leak(Box::new(buffer)),
        }
    }

    fn construct_buffer() -> Buffer { … }
}
```

We set the initial column position to 0 and choose some arbitrary colors for foreground and background color. The difficult part is the buffer construction, it's described in detail below. We then use [`Box::new`] and [`Box::leak`] to transform the created `Buffer` into a `&'static mut Buffer`, because the `buffer` field needs to be of that type.

[`Box::new`]: https://doc.rust-lang.org/nightly/std/boxed/struct.Box.html#method.new
[`Box::leak`]: https://doc.rust-lang.org/nightly/std/boxed/struct.Box.html#method.leak

#### Buffer Construction
So how do we create a `Buffer` instance? The naive approach does not work unfortunately:

```rust
fn construct_buffer() -> Buffer {
    Buffer {
        chars: [[Volatile::new(empty_char()); BUFFER_WIDTH]; BUFFER_HEIGHT],
    }
}

fn empty_char() -> ScreenChar {
    ScreenChar {
        ascii_character: b' ',
        color_code: ColorCode::new(Color::Green, Color::Brown),
    }
}
```

When running `cargo test` the following error occurs:

```
error[E0277]: the trait bound `volatile::Volatile<vga_buffer::ScreenChar>: core::marker::Copy` is not satisfied
   --> src/vga_buffer.rs:186:21
    |
186 |             chars: [[Volatile::new(empty_char); BUFFER_WIDTH]; BUFFER_HEIGHT],
    |                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ the trait `core::marker::Copy` is not implemented for `volatile::Volatile<vga_buffer::ScreenChar>`
    |
    = note: the `Copy` trait is required because the repeated element will be copied
```

The problem is that array construction in Rust requires that the contained type is [`Copy`]. The `ScreenChar` is `Copy`, but the `Volatile` wrapper is not. There is currently no easy way to circumvent this without using [`unsafe`], but fortunately there is the [`array_init`] crate that provides a safe interface for such operations.

[`Copy`]: https://doc.rust-lang.org/core/marker/trait.Copy.html
[`unsafe`]: https://doc.rust-lang.org/book/second-edition/ch19-01-unsafe-rust.html
[`array_init`]: https://docs.rs/array-init

To use that crate, we add the following to our `Cargo.toml`:

```toml
[dev-dependencies]
array-init = "0.0.3"
```

Note that we're using the [`dev-dependencies`] table instead of the `dependencies` table, because we only need the crate for `cargo test` and not for a normal build.

[`dev-dependencies`]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#development-dependencies

Now we can fix our `construct_buffer` function:

```rust
fn construct_buffer() -> Buffer {
    use array_init::array_init;

    Buffer {
        chars: array_init(|_| array_init(|_| Volatile::new(empty_char()))),
    }
}
```

See the [documentation of `array_init`][`array_init`] for more information about using that crate.

### Testing `write_byte`
Now we're finally able to write a first unit test that tests the `write_byte` method:

```rust
// in vga_buffer.rs

mod test {
    […]

    #[test]
    fn write_byte() {
        let mut writer = construct_writer();
        writer.write_byte(b'X');
        writer.write_byte(b'Y');

        for (i, row) in writer.buffer.chars.iter().enumerate() {
            for (j, screen_char) in row.iter().enumerate() {
                let screen_char = screen_char.read();
                if i == BUFFER_HEIGHT - 1 && j == 0 {
                    assert_eq!(screen_char.ascii_character, b'X');
                    assert_eq!(screen_char.color_code, writer.color_code);
                } else if i == BUFFER_HEIGHT - 1 && j == 1 {
                    assert_eq!(screen_char.ascii_character, b'Y');
                    assert_eq!(screen_char.color_code, writer.color_code);
                } else {
                    assert_eq!(screen_char, empty_char());
                }
            }
        }
    }
}
```

We construct a `Writer`, write two bytes to it, and then check that the right screen characters were updated. When we run `cargo test`, we see that the test is executed and passes:

```
running 1 test
test vga_buffer::test::write_byte ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Try to play around a bit with this function and verify that the test fails if you change something, e.g. if you print a third byte without adjusting the `for` loop.

(If you're getting an “binary operation `==` cannot be applied to type `vga_buffer::ScreenChar`” error, you need to also derive [`PartialEq`] for `ScreenChar` and `ColorCode`).

[`PartialEq`]: https://doc.rust-lang.org/nightly/core/cmp/trait.PartialEq.html

### Testing Strings
Let's add a second unit test to test formatted output and newline behavior:

```rust
// in src/vga_buffer.rs

mod test {
    […]

    #[test]
    fn write_formatted() {
        use core::fmt::Write;

        let mut writer = construct_writer();
        writeln!(&mut writer, "a").unwrap();
        writeln!(&mut writer, "b{}", "c").unwrap();

        for (i, row) in writer.buffer.chars.iter().enumerate() {
            for (j, screen_char) in row.iter().enumerate() {
                let screen_char = screen_char.read();
                if i == BUFFER_HEIGHT - 3 && j == 0 {
                    assert_eq!(screen_char.ascii_character, b'a');
                    assert_eq!(screen_char.color_code, writer.color_code);
                } else if i == BUFFER_HEIGHT - 2 && j == 0 {
                    assert_eq!(screen_char.ascii_character, b'b');
                    assert_eq!(screen_char.color_code, writer.color_code);
                } else if i == BUFFER_HEIGHT - 2 && j == 1 {
                    assert_eq!(screen_char.ascii_character, b'c');
                    assert_eq!(screen_char.color_code, writer.color_code);
                } else if i >= BUFFER_HEIGHT - 2 {
                    assert_eq!(screen_char.ascii_character, b' ');
                    assert_eq!(screen_char.color_code, writer.color_code);
                } else {
                    assert_eq!(screen_char, empty_char());
                }
            }
        }
    }
}
```

In this test we're using the [`writeln!`] macro to print strings with newlines to the buffer. Most of the for loop is similar to the `write_byte` test and only verifies if the written characters are at the expected place. The new `if i >= BUFFER_HEIGHT - 2` case verifies that the empty lines that are shifted in on a newline have the `writer.color_code`, which is different from the initial color.

[`writeln!`]: https://doc.rust-lang.org/nightly/core/macro.writeln.html

### More Tests
We only present two basic tests here as an example, but of course many more tests are possible. For example a test that changes the writer color in between writes. Or a test that checks that the top line is correctly shifted off the screen on a newline. Or a test that checks that non-ASCII characters are handled correctly.

## Summary
Unit testing is a very useful technique to ensure that certain components have a desired behavior. Even if they cannot show the absence of bugs, they're still an useful tool for finding them and especially for avoiding regressions.

This post explained how to set up unit testing in a Rust kernel. We now have a functioning test framework and can easily add tests by adding functions with a `#[test]` attribute. To run them, a short `cargo test` suffices. We also added a few basic tests for our VGA buffer as an example how unit tests could look like.

We also learned a bit about conditional compilation, Rust's [lint system], how to [initialize arrays with non-Copy types], and the `dev-dependencies` section of the `Cargo.toml`.

[lint system]: #silencing-the-warnings
[initialize arrays with non-Copy types]: #buffer-construction

## What's next?
We now have a working unit testing framework, which gives us the ability to test individual components. However, unit tests have the disadvantage that they run on the host machine and are thus unable to test how components interact with platform specific parts. For example, we can't test the `println!` macro with an unit test because it wants to write at the VGA text buffer at address `0xb8000`, which only exists in the bare metal environment.

The next post will close this gap by creating a basic _integration test_ framework, which runs the tests in QEMU and thus has access to platform specific components. This will allow us to test the full system, for example that our kernel boots correctly or that no deadlock occurs on nested `println!` invocations.
