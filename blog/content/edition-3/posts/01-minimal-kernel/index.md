+++
title = "Minimal Kernel"
weight = 1
path = "minimal-kernel"
date = 0000-01-01
draft = true

[extra]
chapter = "Bare Bones"
icon = '''
<svg xmlns="http://www.w3.org/2000/svg" fill="currentColor" class="bi bi-file-earmark-binary" viewBox="0 0 16 16">
  <path d="M7.05 11.885c0 1.415-.548 2.206-1.524 2.206C4.548 14.09 4 13.3 4 11.885c0-1.412.548-2.203 1.526-2.203.976 0 1.524.79 1.524 2.203zm-1.524-1.612c-.542 0-.832.563-.832 1.612 0 .088.003.173.006.252l1.559-1.143c-.126-.474-.375-.72-.733-.72zm-.732 2.508c.126.472.372.718.732.718.54 0 .83-.563.83-1.614 0-.085-.003-.17-.006-.25l-1.556 1.146zm6.061.624V14h-3v-.595h1.181V10.5h-.05l-1.136.747v-.688l1.19-.786h.69v3.633h1.125z"/>
  <path d="M14 14V4.5L9.5 0H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2zM9.5 3A1.5 1.5 0 0 0 11 4.5h2V14a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1h5.5v2z"/>
</svg>
'''
+++

The first step in creating our own operating system kernel is to create a [bare metal] Rust executable that does not depend on an underlying operating system.
For that we need to disable most of Rust's standard library and adjust various compilation settings.
The result is a minimal operating system kernel that forms the base for the following posts of this series.

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

This blog is openly developed on [GitHub].
If you have any problems or questions, please open an issue there.
You can also leave comments [at the bottom].
The complete source code for this post can be found in the [`post-3.1`][post branch] branch.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-3.1

<!-- toc -->

## Introduction
Kernels are the heart of an operating system.
They provide all the fundamental building blocks that are required for building higher-level programs.
Typical building blocks are threads, files, heap memory, timers, or sockets.
Other important tasks of a kernel are the isolation of different programs and the multiplexing of resources.

When writing an operating system kernel, we need to provide all of these building blocks ourselves.
This means that we can't use most of the [Rust standard library].
However, there are still a lot of Rust features that we _can_ use.
For example, we can use [iterators], [closures], [pattern matching], [`Option`] and [`Result`], [string formatting], and of course the [ownership system].
These features make it possible to write a kernel in a very expressive, high level way and worry less about [undefined behavior] or [memory safety].

[`Option`]: https://doc.rust-lang.org/core/option/
[`Result`]: https://doc.rust-lang.org/core/result/
[Rust standard library]: https://doc.rust-lang.org/std/
[iterators]: https://doc.rust-lang.org/book/ch13-02-iterators.html
[closures]: https://doc.rust-lang.org/book/ch13-01-closures.html
[pattern matching]: https://doc.rust-lang.org/book/ch06-00-enums.html
[string formatting]: https://doc.rust-lang.org/core/macro.write.html
[ownership system]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
[undefined behavior]: https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs
[memory safety]: https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention

In this post, we create a minimal OS kernel that can be run without an underlying operating system.
Such an executable is often called a “freestanding” or “bare-metal” executable.
We then make this executable compatible with the early-boot environment of the `x86_64` architecture so that we can boot it as an operating system kernel.

## Disabling the Standard Library
By default, all Rust crates link the [standard library], which depends on the operating system for features such as threads, files, or networking.
It also depends on the C standard library `libc`, which closely interacts with OS services.
Since our plan is to write an operating system, we cannot use any OS-dependent libraries.
So we have to disable the automatic inclusion of the standard library, which we can do through the [`no_std` attribute].

[standard library]: https://doc.rust-lang.org/std/
[`no_std` attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

We start by creating a new cargo application project.
The easiest way to do this is through the command line:

```
cargo new kernel --bin --edition 2021
```

We name the project `kernel` here, but of course you can choose your own name.
The `--bin` flag specifies that we want to create an executable binary (in contrast to a library) and the `--edition 2021` flag specifies that we want to use the [2021 edition] of Rust for our crate.
When we run the command, cargo creates the following directory structure for us:

[2021 edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2021/index.html

```
kernel
├── Cargo.toml
└── src
    └── main.rs
```

The `Cargo.toml` contains the crate configuration, for example the crate name, the [semantic version] number, and dependencies.
The `src/main.rs` file contains the root module of our crate and our `main` function.
You can compile your crate through `cargo build` and then run the compiled `kernel` binary in the `target/debug` subfolder.

[semantic version]: https://semver.org/

### The `no_std` Attribute

Right now our crate implicitly links the standard library.
Let's try to disable this by adding the [`no_std` attribute]:

```rust,hl_lines=3
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

When we try to build it now (by running `cargo build`), the following errors occur:

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^

error: `#[panic_handler]` function required, but not found

error: language item required, but not found: `eh_personality`
[...]
```

The reason for the first error is that the [`println` macro] is part of the standard library, which we no longer include.
So we can no longer print things.
This makes sense, since `println` writes to [standard output], which is a special file descriptor provided by the operating system.

[`println` macro]: https://doc.rust-lang.org/std/macro.println.html
[standard output]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

So let's remove the printing and try again with an empty main function:

```rust,hl_lines=5
// main.rs

#![no_std]

fn main() {}
```

```
❯ cargo build
error: `#[panic_handler]` function required, but not found

error: language item required, but not found: `eh_personality`
[...]
```

The `println` error is gone, but the compiler is still missing a `#[panic_handler]` function and a _language item_.

### Panic Implementation

The `panic_handler` attribute defines the function that the compiler should invoke when a [panic] occurs.
The standard library provides its own panic handler function, but in a `no_std` environment we need to define one ourselves:

[panic]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust,hl_lines=3 9-13
// in main.rs

use core::panic::PanicInfo;

#![no_std]

fn main() {}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

The [`PanicInfo` parameter][PanicInfo] contains the file and line where the panic happened and the optional panic message.
The handler function should never return, so it is marked as a [diverging function] by returning the [“never” type] `!`.
There is not much we can do in this function for now, so we just loop indefinitely.

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[“never” type]: https://doc.rust-lang.org/nightly/std/primitive.never.html

After defining a panic handler, only the `eh_personality` language item error remains:

```
❯ cargo build
error: language item required, but not found: `eh_personality`
  |
  = note: this can occur when a binary crate with `#![no_std]` is compiled for a
    target where `eh_personality` is defined in the standard library
  = help: you may be able to compile for a target that doesn't need `eh_personality`,
    specify a target with `--target` or in `.cargo/config`
```

### Disabling Unwinding

Language items are special functions and types that are required internally by the compiler.
They are normally provided by the standard library, which we disabled using the `#![no_std]` attribute.

The [`eh_personality` language item] marks a function that is used for implementing [stack unwinding].
By default, Rust uses unwinding to run the destructors of all live stack variables in case of a [panic].
This ensures that all used memory is freed and allows the parent thread to catch the panic and continue execution.
Unwinding, however, is a complex process and requires some OS-specific libraries, such as [libunwind] on Linux or [structured exception handling] on Windows.

[`eh_personality` language item]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: https://www.nongnu.org/libunwind/
[structured exception handling]: https://docs.microsoft.com/de-de/windows/win32/debug/structured-exception-handling

While unwinding is very useful, it also has some drawbacks.
For example, it increases the size of the compiled executable because it requires additional context at runtime.
Because of these drawbacks, Rust provides an option to [abort on panic] instead.

[abort on panic]: https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html#unwinding-the-stack-or-aborting-in-response-to-a-panic

We already use a custom panic handler that never returns, so we don't need unwinding for our kernel.
By disabling it, the `eh_personality` language item won't be required anymore.

There are multiple ways to set the panic strategy, the easiest is to use [cargo profiles]:

[cargo profiles]: https://doc.rust-lang.org/cargo/reference/profiles.html

```toml,hl_lines=3-7
# in Cargo.toml

[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

This sets the panic strategy to `abort` for both the `dev` profile (used for `cargo build`) and the `release` profile (used for `cargo build --release`).
Now the `eh_personality` language item should no longer be required.

When we try to compile our kernel now, a new error occurs:

```
❯ cargo build
error: requires `start` lang_item
```

Our kernel is missing the `start` language item, which defines the _entry point_ of the executable.


## Setting the Entry Point

The [entry point] of a program is the function that is called when the executable is started.
One might think that the `main` function is the first function called, however, most languages have a [runtime system], which is responsible for things such as garbage collection (e.g. in Java) or software threads (e.g. goroutines in Go).
This runtime needs to be called before `main`, since it needs to initialize itself.

[entry point]: https://en.wikipedia.org/wiki/Entry_point
[runtime system]: https://en.wikipedia.org/wiki/Runtime_system

In a typical Rust binary that links the standard library, execution starts in a C runtime library called [`crt0`] (“C runtime zero”), which sets up the environment for a C application.
This includes creating a [call stack] and placing the command line arguments in the right CPU registers.
The C runtime then invokes the [entry point of the Rust runtime][rt::lang_start], which is marked by the `start` language item.
Rust only has a very minimal runtime, which takes care of some small things such as setting up stack overflow guards or printing a backtrace on panic.
The runtime then finally calls the `main` function.

[`crt0`]: https://en.wikipedia.org/wiki/Crt0
[call stack]: https://en.wikipedia.org/wiki/Call_stack
[rt::lang_start]: hhttps://github.com/rust-lang/rust/blob/0d97f7a96877a96015d70ece41ad08bb7af12377/library/std/src/rt.rs#L59-L70

Since we're building an operating system kernel that should run without any underlying operating system, we don't want our kernel to depend on any Rust or C runtime.
To remove these dependencies, we need to do two things:

1. Instruct the compiler that we want to build for a bare-metal target environment. This removes the dependency on the C library.
2. Disable the Rust main function to remove the Rust runtime.

### Bare-Metal Target

By default Rust tries to build an executable that is able to run in your current system environment.
For example, if you're using Windows and an `x86_64` CPU, Rust tries to build a `.exe` Windows executable that uses `x86_64` instructions.
This environment is called your "host" system.

To describe different environments, Rust uses a string called [_target triple_].
You can see the target triple for your host system by running `rustc --version --verbose`:

[_target triple_]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple

```
rustc 1.68.1 (8460ca823 2023-03-20)
binary: rustc
commit-hash: 8460ca823e8367a30dda430efda790588b8c84d3
commit-date: 2023-03-20
host: x86_64-unknown-linux-gnu
release: 1.68.1
LLVM version: 15.0.6
```

The above output is from a `x86_64` Linux system.
We see that the `host` triple is `x86_64-unknown-linux-gnu`, which includes the CPU architecture (`x86_64`), the vendor (`unknown`), the operating system (`linux`), and the [ABI] (`gnu`).

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

By compiling for our host triple, the Rust compiler and the linker assume that there is an underlying operating system such as Linux or Windows that uses the C runtime by default, which requires the `start` language item.
To avoid the runtimes, we can compile for a different environment with no underlying operating system.

#### The `x86_64-unknown-none` Target

Rust supports a [variety of target systems][platform-support], including some bare-metal targets.
For example, the `thumbv7em-none-eabihf` target triple can be used to compile for an [embedded] [ARM] system with a `Cortex M4F` CPU, as used in the [Rust Embedded Book].

[platform-support]: https://doc.rust-lang.org/rustc/platform-support.html
[embedded]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture
[Rust Embedded Book]: https://docs.rust-embedded.org/book/intro/index.html

Our kernel should run on a bare-metal `x86_64` system, so the suitable target triple is [`x86_64-unknown-none`].
The `-none` suffix indicates that there is no underlying operating system.
To be able to compile for this target, we need to add it using [`rustup`]:

[`x86_64-unknown-none`]: https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html
[`rustup`]: https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html

```
rustup target add x86_64-unknown-none
```

This downloads a pre-compiled copy of the `core` library for the target.
Afterwards, we can [cross compile] our executable for a bare metal environment by passing a `--target` argument:

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

```
❯ cargo build --target x86_64-unknown-none
   Compiling kernel v0.1.0 (/<...>/kernel)
error: requires `start` lang_item
```

We still get the error about a missing `start` language item because we're still depending on the Rust runtime. To remove that dependency, we can use the `#[no_main]` attribute.

### The `#[no_main]` Attribute

To tell the Rust compiler that we don't want to use the normal entry point chain, we add the `#![no_main]` attribute.

```rust,hl_lines=4
// main.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

You might notice that we removed the `main` function.
The reason is that a `main` doesn't make sense without an underlying runtime that calls it.
Instead, we are now overwriting the operating system entry point with our own `_start` function:

```rust,hl_lines=3-6
// in main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

By using the `#[no_mangle]` attribute we disable the [name mangling] to ensure that the Rust compiler really outputs a function with the name `_start`.
Without the attribute, the compiler would generate some cryptic `_ZN3kernel4_start7hb173fedf945531caE` symbol to give every function an unique name.
The reason for naming the function `_start` is that this is the default entry point name for most systems.

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling

We mark the function as `extern "C"` to tell the compiler that it should use the [C calling convention] for this function (instead of the unspecified Rust calling convention).

[C calling convention]: https://en.wikipedia.org/wiki/Calling_convention

Like in our panic handler, the `!` return type means that the function is diverging, i.e. not allowed to ever return.
This is required because the entry point is not called by any function, but invoked directly by the operating system or bootloader.
So instead of returning, the entry point should e.g. invoke the [`exit` system call] of the operating system.
In our case, shutting down the machine could be a reasonable action, since there's nothing left to do if a freestanding binary returns.
For now, we fulfill the requirement by looping endlessly.

[`exit` system call]: https://en.wikipedia.org/wiki/Exit_(system_call)

When we run `cargo build --target x86_64-unknown-none` now, it should finally compile without any errors:

```
❯ cargo build --target x86_64-unknown-none
   Compiling kernel v0.1.0 (/<...>/kernel)
    Finished dev [unoptimized + debuginfo] target(s) in 0.25s
```

We successfully created a minimal bare-metal kernel executable! The compiled executable can be found at `target/x86_64-unknown-none/debug/kernel`.
There is no `.exe` extension even if you're on Windows because the `x86_64-unknown-none` target uses UNIX standards.

To build the kernel with optimizations, we can run:

```
cargo build --target x86_64-unknown-none --release
```

The compiled executable is placed at `target/x86_64-unknown-none/release/kernel` in this case.

In the next post we will cover how to turn this kernel into a bootable disk image that can be run in a virtual machine or on real hardware.
In the rest of this post, we will introduce some tools for examining our kernel executable.
These tools are very useful for debugging future issues, so it's good to know about them.

## Useful Tools

In this section, we will examine our kernel executable using the [`objdump`], [`nm`], and [`size`] tools.

[`objdump`]: https://www.man7.org/linux/man-pages/man1/objdump.1.html
[`nm`]: https://man7.org/linux/man-pages/man1/nm.1.html
[`size`]: https://man7.org/linux/man-pages/man1/size.1.html

If you're on a UNIX system, you might already have the above tools installed.
Otherwise (and on Windows), you can use the LLVM binutils shipped by `rustup` through the [`cargo-binutils`] crate.
To install it, run **`cargo install cargo-binutils`** and **`rustup component add llvm-tools-preview`**.
Afterwards, you can run the tools through `rust-nm`, `rust-objdump`, and `rust-strip`.

[`cargo-binutils`]: https://github.com/rust-embedded/cargo-binutils

### `nm`

We defined a `_start` function as the entry point of our kernel.
To verify that it is properly exposed in the executable, we can run `nm` to list all the symbols defined in the executable:

```
❯ rust-nm target/x86_64-unknown-none/debug/kernel
0000000000201120 T _start
```

If we comment out the `_start` function or if we remove the `#[no_mangle]` attribute, the `_start` symbol is no longer there after recompiling:

```
❯ rust-nm target/x86_64-unknown-none/debug/kernel
```

This way we can ensure that we set the `_start` function correctly.

### `objdump`

The `objdump` tool can inspect different parts of executables that use the [ELF file format]. This is the file format that the `x86_64-unknown-none` target uses, so we can use `objdump` to inspect our kernel executable.

[ELF file format]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format

#### File Headers

Among other things, the ELF [file header] specifies the target architecture and the entry point address of the executable files.
To print the file header, we can use `objdump -f`:

[file header]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#File_header

```
❯ rust-objdump -f target/x86_64-unknown-none/debug/kernel

target/x86_64-unknown-none/debug/kernel:	file format elf64-x86-64
architecture: x86_64
start address: 0x0000000000001210
```

As expected, our kernel targets the `x86_64` CPU architecture.
The start address specifies the memory address of our `_start` function.
Here the function name `_start` becomes important.
If we rename the function to something else (e.g., `_start_here`) and recompile, we see that no start address is set in the ELF file anymore:

```bash,hl_lines=5
❯ rust-objdump -f target/x86_64-unknown-none/debug/kernel

target/x86_64-unknown-none/debug/kernel:	file format elf64-x86-64
architecture: x86_64
start address: 0x0000000000000000
```

#### Sections

Using `objdump -h`, we can print the various sections of our kernel executable:

```
❯ rust-objdump -h target/x86_64-unknown-none/debug/kernel

target/x86_64-unknown-none/debug/kernel:	file format elf64-x86-64

Sections:
Idx Name            Size     VMA              Type
  0                 00000000 0000000000000000
  1 .dynsym         00000018 00000000000001c8
  2 .gnu.hash       0000001c 00000000000001e0
  3 .hash           00000010 00000000000001fc
  4 .dynstr         00000001 000000000000020c
  5 .text           00000004 0000000000001210 TEXT
  6 .dynamic        000000a0 0000000000002218
  7 .debug_abbrev   0000010c 0000000000000000 DEBUG
  8 .debug_info     000005ce 0000000000000000 DEBUG
  9 .debug_aranges  00000040 0000000000000000 DEBUG
 10 .debug_ranges   00000030 0000000000000000 DEBUG
 11 .debug_str      00000492 0000000000000000 DEBUG
 12 .debug_pubnames 000000bc 0000000000000000 DEBUG
 13 .debug_pubtypes 0000036c 0000000000000000 DEBUG
 14 .debug_frame    00000050 0000000000000000 DEBUG
 15 .debug_line     00000059 0000000000000000 DEBUG
 16 .comment        00000013 0000000000000000
 17 .symtab         00000060 0000000000000000
 18 .shstrtab       000000ce 0000000000000000
 19 .strtab         00000022 0000000000000000
 ```

The `.text` section contains the program code, the other sections are not important right now.
The section dump is useful for debugging, for example for checking which section a pointer points to.

Most of the sections only contain debug information and are not needed for execution.
We can remove this debug information using `rust-strip`:

```
❯ rust-strip target/x86_64-unknown-none/debug/kernel
❯ rust-objdump -h target/x86_64-unknown-none/debug/kernel

target/x86_64-unknown-none/debug/kernel:	file format elf64-x86-64

Sections:
Idx Name          Size     VMA              Type
  0               00000000 0000000000000000
  1 .dynsym       00000018 00000000000001c8
  2 .gnu.hash     0000001c 00000000000001e0
  3 .hash         00000010 00000000000001fc
  4 .dynstr       00000001 000000000000020c
  5 .text         00000004 0000000000001210 TEXT
  6 .dynamic      000000a0 0000000000002218
  7 .shstrtab     00000034 0000000000000000
```

#### Disassembling

Sometimes we need to check the [assembly code] that certain functions compile to.
We can use the `objdump -d` command to print the `.text` section of an executable in assembly language:

[assembly code]: https://en.wikipedia.org/wiki/X86_assembly_language

```
❯ rust-objdump -d target/x86_64-unknown-none/debug/kernel

target/x86_64-unknown-none/debug/kernel:	file format elf64-x86-64

Disassembly of section .text:

0000000000001210 <_start>:
    1210: eb 00                        	jmp	0x1212 <_start+0x2>
    1212: eb fe                        	jmp	0x1212 <_start+0x2>
```

We see that our `_start` function consists of just two [`jmp` instructions], which jump to the given address.
The first `jmp` command jumps to the second `jmp` command at address `1212`.
The second `jmp` command jumps to itself again, thereby representing the infinite loop that we've written in our `_start` function.

[`jmp` instructions]: https://www.felixcloutier.com/x86/jmp

As you probably noticed, the first `jmp` command is not really needed.
Such inefficiencies can happen in debug builds because the compiler does not optimize them.
If we disassemble the optimized release build, we see that the compiler indeed removed the unneeded `jmp`:

```
❯ cargo build --target x86_64-unknown-none --release
❯ rust-objdump -d target/x86_64-unknown-none/release/kernel

target/x86_64-unknown-none/release/kernel:	file format elf64-x86-64

Disassembly of section .text:

0000000000001210 <_start>:
    1210: eb fe                        	jmp	0x1210 <_start>
```

We will use continue to use the above tools in future posts, as they're quite useful for debugging issues.

## What's next?

In the [next post], we will learn how to turn our minimal kernel in a bootable disk image, which can then be started in the [QEMU] virtual machine and on real hardware.
For this, we'll explore the boot process of `x86_64` systems and learn about the differences between UEFI and the legacy BIOS firmware.

[next post]: @/edition-3/posts/02-booting/index.md
[QEMU]: https://www.qemu.org/
