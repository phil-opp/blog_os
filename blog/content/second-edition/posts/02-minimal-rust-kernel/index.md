+++
title = "A Minimal Rust Kernel"
order = 4
path = "minimal-rust-kernel"
date  = 0000-01-01
template = "second-edition/page.html"
+++

In this post we create a minimal 64-bit Rust kernel. We built upon the [freestanding Rust binary] from the previous post to create a bootable disk image, that prints something to the screen.

[freestanding Rust binary]: ./second-edition/posts/01-freestanding-rust-binary/index.md

<!-- more -->

This blog is openly developed on [Github]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom].

[Github]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments

## The Boot Process
When you turn on a computer, it begins executing firmware code that is stored in motherboard [ROM]. This code performs a [power-on self-test], detects available RAM, and pre-initializes the CPU and hardware. Afterwards it looks for a bootable disk and starts booting the operating system kernel.

[ROM]: https://en.wikipedia.org/wiki/Read-only_memory
[power-on self-test]: https://en.wikipedia.org/wiki/Power-on_self-test

On x86, there are two firmware standards: the “Basic Input/Output System“ (**[BIOS]**) and the newer “Unified Extensible Firmware Interface” (**[UEFI]**). The BIOS standard is old and outdated, but simple and well-supported on any x86 machine since the 1980s. UEFI, in contrast, is more modern and has much more features, but is more complex to set up (at least in my opinion).

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[UEFI]: https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

Currently, we only provide BIOS support, but support for UEFI is planned, too. If you'd like to help us with this, check out the [Github issue](https://github.com/phil-opp/blog_os/issues/349).

### BIOS Boot
Almost all x86 systems have support for BIOS booting, even newer UEFI-based machines (they include an emulated BIOS). This is great, because you can use the same boot logic across all machines from the last centuries. But this wide compatibility is at the same time the biggest disadvantage of BIOS booting, because it means that the CPU is put into a 16-bit compability mode called [real mode] before booting so that that arcane bootloaders from the 1980s would still work.

But let's start from the beginning:

When you turn on a computer, it loads the BIOS from some special flash memory located on the motherboard. The BIOS runs self test and initialization routines of the hardware, then it looks for bootable disks. If it finds one, the control is transferred to its _bootloader_, which is a 512-byte portion of executable code stored at the disk's beginning. Most bootloaders are larger than 512 bytes, so bootloaders are commonly split into a small first stage, which fits into 512 bytes, and a second stage, which is subsequently loaded by the first stage.

The bootloader has to determine the location of the kernel image on the disk and load it into memory. It also needs to switch the CPU from the 16-bit [real mode] first to the 32-bit [protected mode], and then to the 64-bit [long mode], where 64-bit registers and the complete main memory are available. Its third job is to query certain information (such as a memory map) from the BIOS and pass it to the OS kernel.

[real mode]: https://en.wikipedia.org/wiki/Real_mode
[protected mode]: https://en.wikipedia.org/wiki/Protected_mode
[long mode]: https://en.wikipedia.org/wiki/Long_mode
[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation

Writing a bootloader is a bit cumbersome as it requires assembly language and a lot of non insightful steps like “write this magic value to this processor register”. Therefore we don't cover bootloader creation in this post and instead provide a tool named [bootimage] that automatically appends a bootloader to your kernel.

[bootimage]: https://github.com/phil-opp/bootimage

If you are interested in building your own bootloader, check out our “[Booting]” posts, where we explain in detail how a bootloader is built.

[Booting]: TODO

### The Multiboot Standard

TODO

### UEFI

TODO

## A Minimal Kernel
Now that we roughly know how a computer boots, it's time to create our own minimal kernel. Our goal is to create a disk image that prints a green “Hello” to the screen when booted. For that we build upon the [freestanding Rust binary] from the previous post.

As you may remember, we built the freestanding binary through `cargo`, but depending on the operating system we needed different entry point names and compile flags. That's because `cargo` builds for the _host system_ by default, i.e. the system you're running on. This isn't something we want for our kernel, because a kernel that runs on top of e.g. Windows does not make much sense. Instead, we want to compile for a clearly defined _target system_.

### Target Specification
Cargo supports different target systems through the `--target` parameter. The target is decribed by a so-called _[target triple]_, which describes the CPU architecture, the vendor, the operating system, and the [ABI]. For example, the `x86_64-unknown-linux-gnu` means a `x86_64` CPU, no clear vendor and a Linux operating system with the GNU ABI. Rust supports [many different target triples][platform-support], including `arm-linux-androideabi` for Android or [`wasm32-unknown-unknown` for WebAssembly](https://www.hellorust.com/setup/wasm-target/).

[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[ABI]: https://stackoverflow.com/a/2456882
[platform-support]: https://forge.rust-lang.org/platform-support.html

For our target system, however, we require some special configuration parameters (e.g. no underlying OS), so none of the [existing target triples][platform-support] fits. Fortunately Rust allows us to define our own target through a JSON file. For example, a JSON file that describes the `x86_64-unknown-linux-gnu` target looks like this:

```json
{
    "llvm-target": "x86_64-unknown-linux-gnu",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "linux",
    "executables": true,
    "linker-flavor": "gcc",
    "pre-link-args": ["-m64"],
    "morestack": false
}
```

Most fields are required by LLVM to generate code for that platform. For example, the `data-layout` field defines the size of various integer, floating point, and pointer types. Then there are fields that Rust uses for conditional compilation, such as `target-pointer-width`. The third kind of fields define how the crate should be built. For example, the `pre-link-args` field specifies arguments passed to the [linker].

[linker]: https://en.wikipedia.org/wiki/Linker_(computing)

We also target `x86_64` systems with our kernel, so our target specification will look very similar to the above. Let's start by creating a `x86_64-blog_os.json` file (choose any name you like) with the common content:

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executables": true,
}
```

Note that we changed the OS in the `llvm-target` and the `os` field to `none`, because we will run on bare metal.

We add the following build-related entries:


```json
"linker-flavor": "ld",
"linker": "ld.lld",
```

Instead of using the platform's default linker (which might not support Linux targets), we use the cross platform [LLD] linker for linking our kernel.

[LLD]: https://lld.llvm.org/

```json
"panic": "abort",
```

This setting specifies that the target doesn't support [stack unwinding] on panic, so instead the program should abort directly. This has the same effect as the `panic = "abort"` option in our Cargo.toml, so we can remove it from there.

[stack unwinding]: http://www.bogotobogo.com/cplusplus/stackunwinding.php

```json
"disable-redzone": true,
```

We're writing a kernel, so we'll need to handle interrupts at some point. To do that safely, we have to disable a certain stack pointer optimization called the _“red zone”_, because it would cause stack corruptions otherwise. For more information, see our separate post about [disabling the red zone].

[disabling the red zone]: ./second-edition/extra/disable-red-zone/index.md

```json
"features": "-mmx,-sse,+soft-float",
```

The `features` field enables/disables target features. We disable the `mmx` and `sse` features by prefixing them with a minus and enable the `soft-float` feature by prefixing it with a plus.

The `mmx` and `sse` features determine support for [Single Instruction Multiple Data (SIMD)] instructions, which can often speed up programs significantly. However, the large SIMD registers lead to performance problems in OS kernels, because the kernel has to back them up on each hardware interrupt. To avoid this, we disable SIMD for our kernel (not for applications running on top!).

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

A problem with disabling SIMD is that floating point operations on `x86_64` require SIMD registers by default. To solve this problem, we add the `soft-float` feature, which emulates all floating point operations through software functions based on normal integers.

For more information, see our post on [disabling SIMD](./second-edition/extra/disable-simd/index.md).

#### Putting it Together
Our target specification file now looks like this:

```json
{
  "llvm-target": "x86_64-unknown-linux-gnu",
  "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
  "arch": "x86_64",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "os": "none",
  "linker-flavor": "ld",
  "linker": "ld.lld",
  "executables": true,
  "features": "-mmx,-sse,+soft-float",
  "disable-redzone": true,
  "panic": "abort"
}
```

### Building our Kernel
Compiling for our new target will use Linux conventions. I'm not quite sure why, but I assume that it's just LLVM's default. This means that we need an entry point named `_start` as described in the [previous post]:

[previous post]: ./second-edition/posts/01-freestanding-rust-binary/index.md

```rust
// src/main.rs

#![feature(lang_items)] // required for defining the panic handler
#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

#[lang = "panic_fmt"] // define a function that should be called on panic
#[no_mangle] // TODO required?
pub extern fn rust_begin_panic(_msg: core::fmt::Arguments,
    _file: &'static str, _line: u32, _column: u32) -> !
{
    loop {}
}

#[no_mangle] // don't mangle the name of this function
pub fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start_` by default
    loop {}
}
```

We can now build the kernel for our new target by passing the name of the JSON file (without the `.json` extension) as `--target`. There's is currently an [open bug][custom-target-bug] with custom target, so you also need to set the `RUST_TARGET_PATH` environment variable to the current directory, otherwise Rust might not be able to find your target. The full command is:

[custom-target-bug]: https://github.com/rust-lang/cargo/issues/4905

```
> RUST_TARGET_PATH=(pwd) cargo build --target x86_64-unknown-blog_os

error[E0463]: can't find crate for `core`
  |
  = note: the `x86_64-blog_os` target may not be installed
```

It failed! The error tells us that the Rust compiler no longer finds the core library. The [core library] is implicitly linked to all `no_std` crates and contains things such as `Result`, `Option`, and iterators.

[core library]: https://doc.rust-lang.org/nightly/core/index.html

The problem is that the core library is distributed together with the Rust compiler as a _precompiled_ library. So it is only valid for the host triple (e.g., `x86_64-unknown-linux-gnu`) but not for our custom target. If we want to compile code for other targets, we need to recompile `core` for these targets first.

#### Xargo
That's where [xargo] comes in. It is a wrapper for cargo that eases cross compilation. We can install it by executing:

[xargo]: https://github.com/japaric/xargo

```
cargo install xargo
```

Xargo depends on the rust source code, which we can install with `rustup component add rust-src`.

Xargo is “a drop-in replacement for cargo”, so every cargo command also works with `xargo`. You can do e.g. `xargo --help`, `xargo clean`, or `xargo doc`. The only difference is that the build command has additional functionality: `xargo build` will automatically cross compile the `core` library when compiling for custom targets.

Let's try it:

```bash
> RUST_TARGET_PATH=(pwd) xargo build --target x86_64-unknown-blog_os
   Compiling core v0.0.0 (file:///…/rust/src/libcore)
    Finished release [optimized] target(s) in 22.87 secs
   Compiling blog_os v0.1.0 (file:///…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

(If you're getting a linking error because LLD could not be found, see our “[Installing LLD]” guide.)

[Installing LLD]: ./second-edition/extra/installing-lld/index.md

It worked! We see that `xargo` cross-compiled the `core` library for our new custom target and then continued to compile our `blog_os` crate.

Now we are able to build our kernel for a bare metal target. However, our `_start` entry point, which will be called by the boot loader, is still empty. So let's output something to screen from it.

### Printing to Screen
The easiest way to print text to the screen at this stage is the [VGA text buffer]. It is a special memory area mapped to the VGA hardware that contains the contents displayed on screen. It normally consists of 50 lines that each contain 80 character cells. Each character cell displays an ASCII character with some foreground and background colors. The screen output looks like this:

[VGA text buffer]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode

![screen output for common ASCII characters](https://upload.wikimedia.org/wikipedia/commons/6/6d/Codepage-737.png)

We will discuss the exact layout of the VGA buffer in the next post, where we write a first small driver for it. For printing “Hello”, we just need to know that the buffer is located at address `0xb8000` and that each character cell consists of an ASCII byte and a color byte.

The implementation looks like this:

```rust
#[no_mangle]
pub fn _start(boot_info: &'static mut BootInfo) -> ! {
	let vga_buffer = 0xb8000 as *const u8 as *mut u8;
    unsafe {
        *vga_buffer.offset(0) = b'H';
        *vga_buffer.offset(1) = 0xa; // foreground color green
        *vga_buffer.offset(2) = b'e';
        *vga_buffer.offset(3) = 0xa; // foreground color green
        *vga_buffer.offset(4) = b'l';
        *vga_buffer.offset(5) = 0xa;
        *vga_buffer.offset(6) = b'l';
        *vga_buffer.offset(7) = 0xa;
        *vga_buffer.offset(8) = b'o';
        *vga_buffer.offset(9) = 0xa;
    }

	loop {}
}
```

First, we cast the integer `0xb8000` into a [raw pointer]. Then we use the [`offset`] method to write the first ten bytes individually. We write the ASCII character `b'H'` (the `b` prefix creates an single-byte ASCII character instead of a four-byte Unicode character), then we write the color `0xa` (which translates to “green foreground, black background”). We repeat the same for the other four characters.

[raw pointer]: https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

Note that there's a big [`unsafe`] block around all memory writes. The reason is that the Rust compiler can't prove that the raw pointers we create are valid. They could point anywhere and lead to data corruption. By putting them into an `unsafe` block we're basically telling the compiler that we are absolutely sure that the operations are valid. Note that an `unsafe` block does not turn off Rust's safety checks. It only allows you to do [four additional things].

[`unsafe`]: https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html
[four additional things]: https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html#unsafe-superpowers

I want to emphasize that **this is not the way we want to do things in Rust!** It's very easy to mess up when working with raw pointers inside unsafe blocks, for example, we could easily write behind the buffer's end if we're not careful.

So we want to minimize the use of `unsafe` as much as possible. Rust gives us the ability to do this by creating safe abstractions. For example, we could create a VGA buffer type that encapsulates all unsafety and ensures that it is _impossible_ to do anything wrong from the outside. This way, we would only need minimal amounts of `unsafe` and can be sure that we don't violate [memory safety].

[memory safety]: https://en.wikipedia.org/wiki/Memory_safety

We will create such a safe VGA buffer abstraction in the next post. For the rest of this post, we stick to our unsafe version to keep things simple.

### Creating a Bootimage
Now that we have an executable that does something perceptible, it is time to turn it into a bootable disk image. As we learned in the [section about booting], we need a bootloader for that, which initializes the CPU and loads our kernel.

[section about booting]: #the-boot-process

To make things easy, we created a tool named `bootimage` that automatically downloads a bootloader and combines it with the kernel executable to create a bootable disk image. To install it, execute `cargo install bootimage` in your terminal. After installing, creating a bootimage is as easy as executing `bootimage --target x86_64-unknown-blog_os`. The tool also recompiles your kernel using `xargo`, so it will automatically pick up any changes you make.

You should now see a file named `bootimage.bin` in your crate root directory. This file is a bootable disk image, so can boot it in a virtual machine or copy it to an USB drive to boot it on real hardware. (Note that this is not a CD image, which have a different format, so burning it to a CD doesn't work).

## Booting it!

- qemu
- bochs? virtualbox?
- makefile? cargo-make?

## What's next?
In the next post, we will explore the VGA text buffer in more detail and write a safe interface for it. We will also add support for the `println` macro.
