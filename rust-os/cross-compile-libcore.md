---
layout: page
title: "Cross Compiling: libcore"
category: "rust-os"
---
So you're getting an ``error: can't find crate for `core` [E0463]`` when using `--target x86_64-unknown-linux-gnu`. That means that you're not running Linux or not using using a x86_64 processor.

**If you have an x86_64 processor and want a quick fix**, try it with `x86_64-pc-windows-gnu` or `x86_64-apple-darwin` (or simply omit the explicit `--target`).

The idiomatic alternative and the only option for non x86_64 CPUs is described below. Note that you need to [cross compile binutils], too.
[cross compile binutils]: {{ site.url }}/rust-os/cross-compile-binutils.html

## Libcore
The core library is a dependency-free library that is added implicitly when using `#![no_std]`. It provides basic standard library features like Option or Iterator. The core library is installed together with the rust compiler (just like the std library). But the installed libcore is specific to your architecture. If you aren't working on x86_64 Linux and pass `‑‑target x86_64‑unknown‑linux‑gnu` to cargo, it can't find a x86_64 libcore. To fix this, you can either download it or build it using cargo.

## Download it
You need to download the 64-bit Linux Rust build corresponding to your installed nightly. You can either just update to the current nightly and download the current nightly source [here][Rust downloads]. Or you retrieve your installed version through `rustc --version` and search the corresponding subfolder [here](http://static.rust-lang.org/dist/).
[Rust downloads]: https://www.rust-lang.org/downloads.html

After extracting it and you need to copy the `x86_64-unknown-linux-gnu` folder in `rust-std-x86_64-unknown-linux-gnu/lib/rustlib` to your local Rust installation. For multirust, the right target folder is `~/.multirust/toolchains/nightly/lib/rustlib`. That's it!

## Build it using cargo
The alternative is to use cargo to build libcore. But this variant has one big disadvantage: You have to modify each crate you depend on because it needs to use the same libcore. So you can't just add a crates.io dependency anymore, you need to fork and modify it first.

If you want to build libcore anyway, you need its source code. You can either clone the [rust repository] \(makes updates easy) or manually [download the Rust source][Rust downloads] \(faster and less memory).
[rust repository]: https://github.com/rust-lang/rust

Now we create a new cargo project named `core`, but delete its `src` folder:

```bash
cargo new core
rm -r core/src
```

Then we create a symbolic link named `src` to the `rust/src/libcore` of the Rust source code:

```bash
ln -s ../rust/src/libcore core/src
```

To use our new libcore crate (instead of the one installed together with rust) in our OS, we need to add it as a local dependency in the `Cargo.toml`:

```toml
...
[dependencies.core]
path = "core"
```
Now cargo compiles libcore for all Rust targets automatically.
