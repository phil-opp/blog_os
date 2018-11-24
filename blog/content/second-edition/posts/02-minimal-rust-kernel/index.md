+++
title = "A Minimal Rust Kernel"
order = 2
path = "minimal-rust-kernel"
date = 2018-02-10
template = "second-edition/page.html"
+++

In this post we create a minimal 64-bit Rust kernel for the x86 architecture. We build upon the [freestanding Rust binary] from the previous post to create a bootable disk image, that prints something to the screen.

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
Almost all x86 systems have support for BIOS booting, including newer UEFI-based machines that use an emulated BIOS. This is great, because you can use the same boot logic across all machines from the last centuries. But this wide compatibility is at the same time the biggest disadvantage of BIOS booting, because it means that the CPU is put into a 16-bit compatibility mode called [real mode] before booting so that archaic bootloaders from the 1980s would still work.

But let's start from the beginning:

When you turn on a computer, it loads the BIOS from some special flash memory located on the motherboard. The BIOS runs self test and initialization routines of the hardware, then it looks for bootable disks. If it finds one, the control is transferred to its _bootloader_, which is a 512-byte portion of executable code stored at the disk's beginning. Most bootloaders are larger than 512 bytes, so bootloaders are commonly split into a small first stage, which fits into 512 bytes, and a second stage, which is subsequently loaded by the first stage.

The bootloader has to determine the location of the kernel image on the disk and load it into memory. It also needs to switch the CPU from the 16-bit [real mode] first to the 32-bit [protected mode], and then to the 64-bit [long mode], where 64-bit registers and the complete main memory are available. Its third job is to query certain information (such as a memory map) from the BIOS and pass it to the OS kernel.

[real mode]: https://en.wikipedia.org/wiki/Real_mode
[protected mode]: https://en.wikipedia.org/wiki/Protected_mode
[long mode]: https://en.wikipedia.org/wiki/Long_mode
[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation

Writing a bootloader is a bit cumbersome as it requires assembly language and a lot of non insightful steps like “write this magic value to this processor register”. Therefore we don't cover bootloader creation in this post and instead provide a tool named [bootimage] that automatically prepends a bootloader to your kernel.

[bootimage]: https://github.com/rust-osdev/bootimage

If you are interested in building your own bootloader: Stay tuned, a set of posts on this topic is already planned! <!-- , check out our “_[Writing a Bootloader]_” posts, where we explain in detail how a bootloader is built. -->

#### The Multiboot Standard
To avoid that every operating system implements its own bootloader, which is only compatible with a single OS, the [Free Software Foundation] created an open bootloader standard called [Multiboot] in 1995. The standard defines an interface between the bootloader and operating system, so that any Multiboot compliant bootloader can load any Multiboot compliant operating system. The reference implementation is [GNU GRUB], which is the most popular bootloader for Linux systems.

[Free Software Foundation]: https://en.wikipedia.org/wiki/Free_Software_Foundation
[Multiboot]: https://wiki.osdev.org/Multiboot
[GNU GRUB]: https://en.wikipedia.org/wiki/GNU_GRUB

To make a kernel Multiboot compliant, one just needs to insert a so-called [Multiboot header] at the beginning of the kernel file. This makes it very easy to boot an OS in GRUB. However, GRUB and the Multiboot standard have some problems too:

[Multiboot header]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format

- They support only the 32-bit protected mode. This means that you still have to do the CPU configuration to switch to the 64-bit long mode.
- They are designed to make the bootloader simple instead of the kernel. For example, the kernel needs to be linked with an [adjusted default page size], because GRUB can't find the Multiboot header otherwise. Another example is that the [boot information], which is passed to the kernel, contains lots of architecture dependent structures instead of providing clean abstractions.
- Both GRUB and the Multiboot standard are only sparsely documented.
- GRUB needs to be installed on the host system to create a bootable disk image from the kernel file. This makes development on Windows or Mac more difficult.

[adjusted default page size]: https://wiki.osdev.org/Multiboot#Multiboot_2
[boot information]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format

Because of these drawbacks we decided to not use GRUB or the Multiboot standard. However, we plan to add Multiboot support to our [bootimage] tool, so that it's possible to load your kernel on a GRUB system too. If you're interested in writing a Multiboot compliant kernel, check out the [first edition] of this blog series.

[first edition]: ./first-edition/_index.md

### UEFI

(We don't provide UEFI support at the moment, but we would love to! If you'd like to help, please tell us in the [Github issue](https://github.com/phil-opp/blog_os/issues/349).)

## A Minimal Kernel
Now that we roughly know how a computer boots, it's time to create our own minimal kernel. Our goal is to create a disk image that prints a “Hello World!” to the screen when booted. For that we build upon the [freestanding Rust binary] from the previous post.

As you may remember, we built the freestanding binary through `cargo`, but depending on the operating system we needed different entry point names and compile flags. That's because `cargo` builds for the _host system_ by default, i.e. the system you're running on. This isn't something we want for our kernel, because a kernel that runs on top of e.g. Windows does not make much sense. Instead, we want to compile for a clearly defined _target system_.

### Target Specification
Cargo supports different target systems through the `--target` parameter. The target is described by a so-called _[target triple]_, which describes the CPU architecture, the vendor, the operating system, and the [ABI]. For example, the `x86_64-unknown-linux-gnu` target triple describes a system with a `x86_64` CPU, no clear vendor and a Linux operating system with the GNU ABI. Rust supports [many different target triples][platform-support], including `arm-linux-androideabi` for Android or [`wasm32-unknown-unknown` for WebAssembly](https://www.hellorust.com/setup/wasm-target/).

[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[ABI]: https://stackoverflow.com/a/2456882
[platform-support]: https://forge.rust-lang.org/platform-support.html

For our target system, however, we require some special configuration parameters (e.g. no underlying OS), so none of the [existing target triples][platform-support] fits. Fortunately, Rust allows us to define our own target through a JSON file. For example, a JSON file that describes the `x86_64-unknown-linux-gnu` target looks like this:

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

We also target `x86_64` systems with our kernel, so our target specification will look very similar to the one above. Let's start by creating a `x86_64-blog_os.json` file (choose any name you like) with the common content:

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
"linker-flavor": "ld.lld",
"linker": "rust-lld",
```

Instead of using the platform's default linker (which might not support Linux targets), we use the cross platform [LLD] linker that is shipped with Rust for linking our kernel.

[LLD]: https://lld.llvm.org/

```json
"panic-strategy": "abort",
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
  "llvm-target": "x86_64-unknown-none",
  "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
  "arch": "x86_64",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "os": "none",
  "executables": true,
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
  "panic-strategy": "abort",
  "disable-redzone": true,
  "features": "-mmx,-sse,+soft-float"
}
```

### Building our Kernel
Compiling for our new target will use Linux conventions (I'm not quite sure why, I assume that it's just LLVM's default). This means that we need an entry point named `_start` as described in the [previous post]:

[previous post]: ./second-edition/posts/01-freestanding-rust-binary/index.md

```rust
// src/main.rs

#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}
```

Note that the entry point needs to be called `_start` regardless of your host OS. The Windows and macOS entry points from the previous post should be deleted.

We can now build the kernel for our new target by passing the name of the JSON file as `--target`:

```
> cargo build --target x86_64-blog_os.json

error[E0463]: can't find crate for `core` OR
error[E0463]: can't find crate for `compiler_builtins`
```

It fails! The error tells us that the Rust compiler no longer finds the `core` or the `compiler_builtins` library. Both libraries are implicitly linked to all `no_std` crates. The [`core` library] contains basic Rust types such as `Result`, `Option`, and iterators, whereas the [`compiler_builtins` library] provides various lower level functions expected by LLVM, such as `memcpy`.

[`core` library]: https://doc.rust-lang.org/nightly/core/index.html
[`compiler_builtins` library]: https://github.com/rust-lang-nursery/compiler-builtins

The problem is that the core library is distributed together with the Rust compiler as a _precompiled_ library. So it is only valid for supported host triples (e.g., `x86_64-unknown-linux-gnu`) but not for our custom target. If we want to compile code for other targets, we need to recompile `core` for these targets first.

#### Cargo xbuild
That's where [`cargo xbuild`] comes in. It is a wrapper for `cargo build` that automatically cross-compiles `core` and other built-in libraries. We can install it by executing:

[`cargo xbuild`]: https://github.com/rust-osdev/cargo-xbuild

```
cargo install cargo-xbuild
```

The command depends on the rust source code, which we can install with `rustup component add rust-src`.

Now we can rerun the above command with `xbuild` instead of `build`:

```
> cargo xbuild --target x86_64-blog_os.json
   Compiling core v0.0.0 (file:///…/rust/src/libcore)
    Finished release [optimized] target(s) in 52.75 secs
   Compiling compiler_builtins v0.1.0 (file:///…/rust/src/libcompiler_builtins)
    Finished release [optimized] target(s) in 3.92 secs
   Compiling alloc v0.0.0 (/tmp/xargo.9I97eR3uQ3Cq)
    Finished release [optimized] target(s) in 7.61s
   Compiling blog_os v0.1.0 (file:///…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

We see that `cargo xbuild` cross-compiles the `core`, `compiler_builtin`, and `alloc` libraries for our new custom target. Since these libraries use a lot of unstable features internally, this only works with a [nightly Rust compiler]. Afterwards, `cargo xbuild` successfully compiles our `blog_os` crate.

[nightly Rust compiler]: ./second-edition/posts/01-freestanding-rust-binary/index.md#installing-rust-nightly

Now we are able to build our kernel for a bare metal target. However, our `_start` entry point, which will be called by the boot loader, is still empty. So let's output something to screen from it.

### Printing to Screen
The easiest way to print text to the screen at this stage is the [VGA text buffer]. It is a special memory area mapped to the VGA hardware that contains the contents displayed on screen. It normally consists of 25 lines that each contain 80 character cells. Each character cell displays an ASCII character with some foreground and background colors. The screen output looks like this:

[VGA text buffer]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode

![screen output for common ASCII characters](https://upload.wikimedia.org/wikipedia/commons/6/6d/Codepage-737.png)

We will discuss the exact layout of the VGA buffer in the next post, where we write a first small driver for it. For printing “Hello World!”, we just need to know that the buffer is located at address `0xb8000` and that each character cell consists of an ASCII byte and a color byte.

The implementation looks like this:

```rust
static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
	let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

	loop {}
}
```

First, we cast the integer `0xb8000` into a [raw pointer]. Then we [iterate] over the bytes of the [static] `HELLO` [byte string]. We use the [`enumerate`] method to additionally get a running variable `i`. In the body of the for loop, we use the [`offset`] method to write the string byte and the corresponding color byte (`0xb` is a light cyan).

[iterate]: https://doc.rust-lang.org/book/second-edition/ch13-02-iterators.html
[static]: https://doc.rust-lang.org/book/first-edition/const-and-static.html#static
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate
[byte string]: https://doc.rust-lang.org/reference/tokens.html#byte-string-literals
[raw pointer]: https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

Note that there's an [`unsafe`] block around all memory writes. The reason is that the Rust compiler can't prove that the raw pointers we create are valid. They could point anywhere and lead to data corruption. By putting them into an `unsafe` block we're basically telling the compiler that we are absolutely sure that the operations are valid. Note that an `unsafe` block does not turn off Rust's safety checks. It only allows you to do [four additional things].

[`unsafe`]: https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html
[four additional things]: https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html#unsafe-superpowers

I want to emphasize that **this is not the way we want to do things in Rust!** It's very easy to mess up when working with raw pointers inside unsafe blocks, for example, we could easily write behind the buffer's end if we're not careful.

So we want to minimize the use of `unsafe` as much as possible. Rust gives us the ability to do this by creating safe abstractions. For example, we could create a VGA buffer type that encapsulates all unsafety and ensures that it is _impossible_ to do anything wrong from the outside. This way, we would only need minimal amounts of `unsafe` and can be sure that we don't violate [memory safety]. We will create such a safe VGA buffer abstraction in the next post.

[memory safety]: https://en.wikipedia.org/wiki/Memory_safety

### Creating a Bootimage
Now that we have an executable that does something perceptible, it is time to turn it into a bootable disk image. As we learned in the [section about booting], we need a bootloader for that, which initializes the CPU and loads our kernel.

[section about booting]: #the-boot-process

Instead of writing our own bootloader, which is a project on its own, we use the [`bootloader`] crate. This crate implements a basic BIOS bootloader without any C dependencies, just Rust and inline assembly. To use it for booting our kernel, we need to add a dependency on it:

[`bootloader`]: https://crates.io/crates/bootloader

```toml
# in Cargo.toml

[dependencies]
bootloader = "0.3.4"
```

Adding the bootloader as dependency is not enough to actually create a bootable disk image. The problem is that we need to combine the bootloader with the kernel after it has been compiled, but cargo has no support for additional build steps after successful compilation (see [this issue][post-build script] for more information).

[post-build script]: https://github.com/rust-lang/cargo/issues/545

To solve this problem, we created a tool named `bootimage` that first compiles the kernel and bootloader, and then combines them to create a bootable disk image. To install the tool, execute the following command in your terminal:

```
cargo install bootimage --version "^0.5.0"
```

The `^0.5.0` is a so-called [_caret requirement_], which means "version `0.5.0` or a later compatible version". So if we find a bug and publish version `0.5.1` or `0.5.2`, cargo would automatically use the latest version, as long as it is still a version `0.5.x`. However, it wouldn't choose version `0.6.0`, because it is not considered as compatible. Note that dependencies in your `Cargo.toml` are caret requirements by default, so the same rules are applied to our bootloader dependency.

[_caret requirement_]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#caret-requirements

After installing the `bootimage` tool, creating a bootable disk image is as easy as executing:

```
> bootimage build --target x86_64-blog_os.json
```

You see that the tool recompiles your kernel using `cargo xbuild`, so it will automatically pick up any changes you make. Afterwards it compiles the bootloader, which might take a while. Like all crate dependencies it is only built once and then cached, so subsequent builds will be much faster. Finally, `bootimage` combines the bootloader and your kernel to a bootable disk image.

After executing the command, you should see a bootable disk image named `bootimage-blog_os.bin` in your `target/x86_64-blog_os/debug` directory. You can boot it in a virtual machine or copy it to an USB drive to boot it on real hardware. (Note that this is not a CD image, which have a different format, so burning it to a CD doesn't work).

#### How does it work?
The `bootimage` tool performs the following steps behind the scenes:

- It compiles our kernel to an [ELF] file.
- It compiles the bootloader dependency as a standalone executable.
- It appends the bytes of the kernel ELF file to the bootloader.

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
[rust-osdev/bootloader]: https://github.com/rust-osdev/bootloader

When booted, the bootloader reads and parses the appended ELF file. It then maps the program segments to virtual addresses in the page tables, zeroes the `.bss` section, and sets up a stack. Finally, it reads the entry point address (our `_start` function) and jumps to it.

#### Bootimage Configuration
The `bootimage` tool can be configured through a `[package.metadata.bootimage]` table in the `Cargo.toml` file. We can add a `default-target` option so that we no longer need to pass the `--target` argument:

```toml
# in Cargo.toml

[package.metadata.bootimage]
default-target = "x86_64-blog_os.json"
```

Now we can omit the `--target` argument and just run `bootimage build`.

## Booting it!
We can now boot the disk image in a virtual machine. To boot it in [QEMU], execute the following command:

[QEMU]: https://www.qemu.org/

```
> qemu-system-x86_64 -drive format=raw,file=bootimage-blog_os.bin
warning: TCG doesn't support requested feature: CPUID.01H:ECX.vmx [bit 5]
```

![QEMU showing "Hello World!"](qemu.png)

Alternatively, you can invoke the `run` subcommand of the `bootimage` tool:

```
> bootimage run
```

By default it invokes the exact same QEMU command as above. Additional QEMU options can be passed after a `--`. For example, `bootimage run -- --help` will show the QEMU help. It's also possible to change the default command through an `run-command` key in the `package.metadata.bootimage` table in the `Cargo.toml`. For more information see the `--help` output or the [Readme file].

[Readme file]: https://github.com/rust-osdev/bootimage/blob/master/Readme.md

### Real Machine
It is also possible to write it to an USB stick and boot it on a real machine:

```
> dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

Where `sdX` is the device name of your USB stick. **Be careful** to choose the correct device name, because everything on that device is overwritten.

## What's next?
In the next post, we will explore the VGA text buffer in more detail and write a safe interface for it. We will also add support for the `println` macro.
