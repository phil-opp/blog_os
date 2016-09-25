+++
title = "Cross Compiling: libcore"
+++

If you get an `error: can't find crate for 'core'`, you're probably compiling for a different target (e.g. you're passing the `target` option to `cargo build`). Now the compiler complains that it can't find the `core` library. This document gives a quick overview how to fix this problem. For more details, see the [rust-cross] project.

[rust-cross]: https://github.com/japaric/rust-cross

## Libcore
The core library is a dependency-free library that is added implicitly when using `#![no_std]`. It provides basic standard library features like Option or Iterator. The core library is installed together with the rust compiler (just like the std library). But the installed libcore is specific to your architecture. If you aren't working on x86_64 Linux and pass `‑‑target x86_64‑unknown‑linux‑gnu` to cargo, it can't find a x86_64 libcore. To fix this, you can either use `rustup` or `xargo`.

## rustup
Thanks to [rustup], cross-compilation for [official target triples] is pretty easy today: Just execute `rustup target add x86_64-unknown-linux-gnu`.

[rustup]: https://rustup.rs
[official target triples]: https://github.com/japaric/rust-cross#the-target-triple

## xargo
If you're using a _custom target specification_, the `rustup` method doesn't work. Instead, you can use [xargo]. Xargo is a wrapper for cargo that eases cross compilation. We can install it by executing:

```
cargo install xargo
```
If the installation fails, make sure that you have `cmake` and the OpenSSL headers installed. For more details, see the xargo's [dependency section].

[xargo]: https://github.com/japaric/xargo
[dependency section]: https://github.com/japaric/xargo#dependencies

Xargo is “a drop-in replacement for cargo”, so every cargo command also works with `xargo`. You can do e.g. `xargo --help`, `xargo clean`, or `xargo doc`. However, the `build` command gains additional functionality: `xargo build` will automatically cross compile the `core` library (and a few other libraries such as `alloc` and `collections`) when compiling for custom targets.

[xargo]: https://github.com/japaric/xargo

So if your custom target file is named `your-cool-target.json`, you can compile your code using xargo through `xargo build --target your-cool-target` (note the omitted extension).
