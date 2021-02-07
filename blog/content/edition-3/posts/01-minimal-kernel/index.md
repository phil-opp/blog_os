+++
title = "Minimal Kernel"
weight = 1
path = "minimal-kernel"
date = 0000-01-01

[extra]
chapter = "Bare Bones"
icon = '''
<svg xmlns="http://www.w3.org/2000/svg" fill="currentColor" class="bi bi-file-earmark-binary" viewBox="0 0 16 16">
  <path d="M7.05 11.885c0 1.415-.548 2.206-1.524 2.206C4.548 14.09 4 13.3 4 11.885c0-1.412.548-2.203 1.526-2.203.976 0 1.524.79 1.524 2.203zm-1.524-1.612c-.542 0-.832.563-.832 1.612 0 .088.003.173.006.252l1.559-1.143c-.126-.474-.375-.72-.733-.72zm-.732 2.508c.126.472.372.718.732.718.54 0 .83-.563.83-1.614 0-.085-.003-.17-.006-.25l-1.556 1.146zm6.061.624V14h-3v-.595h1.181V10.5h-.05l-1.136.747v-.688l1.19-.786h.69v3.633h1.125z"/>
  <path d="M14 14V4.5L9.5 0H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2zM9.5 3A1.5 1.5 0 0 0 11 4.5h2V14a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1h5.5v2z"/>
</svg>
'''
+++

The first step in creating our own operating system kernel is to create a [bare metal] Rust executable that does not depend on an underlying operating system. For that we need to disable most of Rust's standard library and adjust various compilation settings. The result is a minimal operating system kernel that forms the base for the following posts of this series.

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

This blog is openly developed on [GitHub]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom]. The complete source code for this post can be found in the [`post-01`][post branch] branch.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## Introduction
To write an operating system kernel, we need code that does not depend on any operating system features. This means that we can't use threads, files, heap memory, the network, random numbers, standard output, or any other features requiring OS abstractions or specific hardware. Which makes sense, since we're trying to write our own OS and our own drivers.

While this means that we can't use most of the [Rust standard library], there are still a lot of Rust features that we _can_ use. For example, we can use [iterators], [closures], [pattern matching], [option] and [result], [string formatting], and of course the [ownership system]. These features make it possible to write a kernel in a very expressive, high level way without worrying about [undefined behavior] or [memory safety].

[option]: https://doc.rust-lang.org/core/option/
[result]:https://doc.rust-lang.org/core/result/
[Rust standard library]: https://doc.rust-lang.org/std/
[iterators]: https://doc.rust-lang.org/book/ch13-02-iterators.html
[closures]: https://doc.rust-lang.org/book/ch13-01-closures.html
[pattern matching]: https://doc.rust-lang.org/book/ch06-00-enums.html
[string formatting]: https://doc.rust-lang.org/core/macro.write.html
[ownership system]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
[undefined behavior]: https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs
[memory safety]: https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention

In order to create a minimal OS kernel in Rust, we start by creating an executable that can be run without an underlying operating system. Such an executable is often called a “freestanding” or “bare-metal” executable. We then make this executable compatible with the early-boot environment of the `x86_64` architecture so that we can boot it as an operating system kernel.

## Disabling the Standard Library
By default, all Rust crates link the [standard library], which depends on the operating system for features such as threads, files, or networking. It also depends on the C standard library `libc`, which closely interacts with OS services. Since our plan is to write an operating system, we cannot use any OS-dependent libraries. So we have to disable the automatic inclusion of the standard library, which we can do through the [`no_std` attribute].

[standard library]: https://doc.rust-lang.org/std/
[`no_std` attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

We start by creating a new cargo application project. The easiest way to do this is through the command line:

```
cargo new blog_os --bin --edition 2018
```

I named the project `blog_os`, but of course you can choose your own name. The `--bin` flag specifies that we want to create an executable binary (in contrast to a library) and the `--edition 2018` flag specifies that we want to use the [2018 edition] of Rust for our crate. When we run the command, cargo creates the following directory structure for us:

[2018 edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

The `Cargo.toml` contains the crate configuration, for example the crate name, the author, the [semantic version] number, and dependencies. The `src/main.rs` file contains the root module of our crate and our `main` function. You can compile your crate through `cargo build` and then run the compiled `blog_os` binary in the `target/debug` subfolder.

[semantic version]: https://semver.org/

### The `no_std` Attribute

Right now our crate implicitly links the standard library. Let's try to disable this by adding the [`no_std` attribute]:

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

When we try to build it now (by running `cargo build`), the following error occurs:

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

The reason for this error is that the [`println` macro] is part of the standard library, which we no longer include. So we can no longer print things. This makes sense, since `println` writes to [standard output], which is a special file descriptor provided by the operating system.

[`println` macro]: https://doc.rust-lang.org/std/macro.println.html
[standard output]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

So let's remove the printing and try again with an empty main function:

```rust
// main.rs

#![no_std]

fn main() {}
```

```
> cargo build
error: `#[panic_handler]` function required, but not found
error: language item required, but not found: `eh_personality`
```

Now the compiler is missing a `#[panic_handler]` function and a _language item_.

### Panic Implementation

The `panic_handler` attribute defines the function that the compiler should invoke when a [panic] occurs. The standard library provides its own panic handler function, but in a `no_std` environment we need to define it ourselves:

[panic]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// in main.rs

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

The [`PanicInfo` parameter][PanicInfo] contains the file and line where the panic happened and the optional panic message. The function should never return, so it is marked as a [diverging function] by returning the [“never” type] `!`. There is not much we can do in this function for now, so we just loop indefinitely.

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[“never” type]: https://doc.rust-lang.org/nightly/std/primitive.never.html

<div class = "note">

A side note about `loop {}`: There is currently a bug in LLVM (the code generator used by Rust) that [incorrectly optimizes away loops](https://github.com/rust-lang/rust/issues/28728) in some cases. Fortunately, this [no longer applies to empty loops](https://github.com/rust-lang/rust/pull/77972) and should also be [fixed in general](https://github.com/rust-lang/rust/issues/28728#issuecomment-766128831) soon.

</div>

After defining a panic handler, only the `eh_personality` language item error remains:

```
> cargo build
error: language item required, but not found: `eh_personality`
```

### The `eh_personality` Language Item

Language items are special functions and types that are required internally by the compiler. For example, the [`Copy`] trait is a language item that tells the compiler which types have [_copy semantics_][`Copy`]. When we look at the [implementation][copy code], we see it has the special `#[lang = "copy"]` attribute that defines it as a language item.

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

While providing custom implementations of language items is possible, it should only be done as a last resort. The reason is that language items are highly unstable implementation details and not even type checked (so the compiler doesn't even check if a function has the right argument types). Fortunately, there is a more stable way to fix the above language item error.

The [`eh_personality` language item] marks a function that is used for implementing [stack unwinding]. By default, Rust uses unwinding to run the destructors of all live stack variables in case of a [panic]. This ensures that all used memory is freed and allows the parent thread to catch the panic and continue execution. Unwinding, however, is a complicated process and requires some OS specific libraries (e.g. [libunwind] on Linux or [structured exception handling] on Windows), so we don't want to use it for our operating system.

[`eh_personality` language item]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: https://www.nongnu.org/libunwind/
[structured exception handling]: https://docs.microsoft.com/de-de/windows/win32/debug/structured-exception-handling

#### Disabling Unwinding

There are other use cases as well for which unwinding is undesirable, so Rust provides an option to [abort on panic] instead. This disables the generation of unwinding symbol information and thus considerably reduces binary size. There are multiple ways to disable unwinding, the easiest is to add the following lines to our `Cargo.toml`:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

This sets the panic strategy to `abort` for both the `dev` profile (used for `cargo build`) and the `release` profile (used for `cargo build --release`). Now the `eh_personality` language item should no longer be required.

[abort on panic]: https://github.com/rust-lang/rust/pull/32900

Now we fixed both of the above errors. However, if we try to compile it now, another error occurs:

```
> cargo build
error: requires `start` lang_item
```

Our program is missing the `start` language item, which defines the entry point.

### The `start` Language Item

One might think that the `main` function is the first function called when a program is run. However, most languages have a [runtime system], which is responsible for things such as garbage collection (e.g. in Java) or software threads (e.g. goroutines in Go). This runtime needs to be called before `main`, since it needs to initialize itself.

[runtime system]: https://en.wikipedia.org/wiki/Runtime_system

In a typical Rust binary that links the standard library, execution starts in a C runtime library called [`crt0`] (“C runtime zero”), which sets up the environment for a C application. This includes creating a [call stack] and placing the command line arguments in the right CPU registers. The C runtime then invokes the [entry point of the Rust runtime][rt::lang_start], which is marked by the `start` language item. Rust only has a very minimal runtime, which takes care of some small things such as setting up stack overflow guards or printing a backtrace on panic. The runtime then finally calls the `main` function.

[`crt0`]: https://en.wikipedia.org/wiki/Crt0
[call stack]: https://en.wikipedia.org/wiki/Call_stack
[rt::lang_start]: hhttps://github.com/rust-lang/rust/blob/0d97f7a96877a96015d70ece41ad08bb7af12377/library/std/src/rt.rs#L59-L70

Our freestanding executable does not have access to the Rust runtime and `crt0`, so we need to define our own entry point. Implementing the `start` language item wouldn't help, since it would still require `crt0`. Instead, we need to overwrite the `crt0` entry point directly.

#### Overwriting the Entry Point
To tell the Rust compiler that we don't want to use the normal entry point chain, we add the `#![no_main]` attribute.

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

You might notice that we removed the `main` function. The reason is that a `main` doesn't make sense without an underlying runtime that calls it. Instead, we are now overwriting the operating system entry point with our own `_start` function:

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

By using the `#[no_mangle]` attribute we disable the [name mangling] to ensure that the Rust compiler really outputs a function with the name `_start`. Without the attribute, the compiler would generate some cryptic `_ZN3blog_os4_start7hb173fedf945531caE` symbol to give every function an unique name. The reason for naming the function `_start` is that this is the default entry point name for most systems.

We mark the function as `extern "C"` to tell the compiler that it should use the [C calling convention] for this function (instead of the unspecified Rust calling convention). The `!` return type means that the function is diverging, i.e. not allowed to ever return. This is required because the entry point is not called by any function, but invoked directly by the operating system or bootloader. So instead of returning, the entry point should e.g. invoke the [`exit` system call] of the operating system. In our case, shutting down the machine could be a reasonable action, since there's nothing left to do if a freestanding binary returns. For now, we fulfill the requirement by looping endlessly.

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[C calling convention]: https://en.wikipedia.org/wiki/Calling_convention
[`exit` system call]: https://en.wikipedia.org/wiki/Exit_(system_call)

When we run `cargo build` now, we get an ugly _linker_ error.

## Linker Errors

The [linker] is a program that combines the generated code into an executable. Since the executable format differs between Linux, Windows, and macOS, each system has its own linker that throws a different error. The fundamental cause of the errors is the same: the default configuration of the linker assumes that our program depends on the C runtime, which it does not.

To solve the errors, we need to tell the linker that we want to build for a bare-metal target, where no underlying operating system or C runtime exist. As an alternative, it is also possible to disable the linking of the C runtime by passing a certain set of arguments to the linker.

### Linker Arguments

Linkers are very complex programs with a lot of configuration options. Each of the major operating systems (Linux, Windows, macOS) has its own linker implementation with different options, but all of them provide a way to disable the linking of the C runtime. By using these options, it is possible to create a freestanding executable that still runs on top of an existing operating system.

_This is not what we want for our kernel, so this section is only provided for completeness. Feel free to skip this section if you like._

In the subsections below, we explain the required linker arguments for each operating system. It's worth noting that creating a freestanding executable this way is probably not a good idea. The reason is that our executable still expects various things, for example that a stack is initialized when the `_start` function is called. Without the C runtime, some of these requirements might not be fulfilled, which might cause our program to fail, e.g. by causing a segmentation fault.

If you want to create a minimal binary that runs on top of an existing operating system, including `libc` and setting the `#[start]` attribute as described [here](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html) is probably a better idea.

<details>
<summary>

#### Linux
</summary>

On Linux the following linker error occurs (shortened):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: /usr/lib/gcc/../x86_64-linux-gnu/Scrt1.o: In function `_start':
          (.text+0x12): undefined reference to `__libc_csu_fini'
          /usr/lib/gcc/../x86_64-linux-gnu/Scrt1.o: In function `_start':
          (.text+0x19): undefined reference to `__libc_csu_init'
          /usr/lib/gcc/../x86_64-linux-gnu/Scrt1.o: In function `_start':
          (.text+0x25): undefined reference to `__libc_start_main'
          collect2: error: ld returned 1 exit status
```

The problem is that the linker includes the startup routine of the C runtime by default, which is also called `_start`. It requires some symbols of the C standard library `libc` that we don't include due to the `no_std` attribute, therefore the linker can't resolve these references. To solve this, we can tell the linker that it should not link the C startup routine by passing the `-nostartfiles` flag.

One way to pass linker attributes via cargo is the `cargo rustc` command. The command behaves exactly like `cargo build`, but allows to pass options to `rustc`, the underlying Rust compiler. `rustc` has the `-C link-arg` flag, which passes an argument to the linker. Combined, our new build command looks like this:

```
cargo rustc -- -C link-arg=-nostartfiles
```

Now our crate builds as a freestanding executable on Linux!

We didn't need to specify the name of our entry point function explicitly since the linker looks for a function with the name `_start` by default.
</details>

<details>
<summary>

#### Windows
</summary>

On Windows, the following linker error occurs (shortened):

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

The "entry point must be defined" error means that the linker can't find the entry point. On Windows, the default entry point name [depends on the used subsystem][windows-subsystems]. For the `CONSOLE` subsystem the linker looks for a function named `mainCRTStartup` and for the `WINDOWS` subsystem it looks for a function named `WinMainCRTStartup`. To override the default and tell the linker to look for our `_start` function instead, we can pass an `/ENTRY` argument to the linker:

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

From the different argument format we clearly see that the Windows linker is a completely different program than the Linux linker.

Now a different linker error occurs:

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

This error occurs because Windows executables can use different [subsystems][windows-subsystems]. For normal programs they are inferred depending on the entry point name: If the entry point is named `main`, the `CONSOLE` subsystem is used, and if the entry point is named `WinMain`, the `WINDOWS` subsystem is used. Since our `_start` function has a different name, we need to specify the subsystem explicitly:

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

We use the `CONSOLE` subsystem here, but the `WINDOWS` subsystem would work too. Instead of passing `-C link-arg` multiple times, we use `-C link-args` which takes a space separated list of arguments.

With this command, our executable should build successfully on Windows.
</details>

<details>
<summary>

#### macOS
</summary>

On macOS, the following linker error occurs (shortened):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

This error message tells us that the linker can't find an entry point function with the default name `main` (for some reason all functions are prefixed with a `_` on macOS). To set the entry point to our `_start` function, we pass the `-e` linker argument:

```
cargo rustc -- -C link-args="-e __start"
```

The `-e` flag specifies the name of the entry point function. Since all functions have an additional `_` prefix on macOS, we need to set the entry point to `__start` instead of `_start`.

Now the following linker error occurs:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS [does not officially support statically linked binaries] and requires programs to link the `libSystem` library by default. To override this and link a static binary, we pass the `-static` flag to the linker:

[does not officially support statically linked binaries]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

This still does not suffice, as a third linker error occurs:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

This error occurs because programs on macOS link to `crt0` (“C runtime zero”) by default. This is similar to the error we had on Linux and can be also solved by adding the `-nostartfiles` linker argument:

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Now our program should build successfully on macOS.
</details>

### Building for a Bare Metal Target

By default Rust tries to build an executable that is able to run in your current system environment. For example, if you're using Windows and an `x86_64` CPU, Rust tries to build a `.exe` Windows executable that uses `x86_64` instructions. This environment is called your "host" system.

To describe different environments, Rust uses a string called [_target triple_]. You can see the target triple for your host system by running `rustc --version --verbose`:

[_target triple_]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple

```
rustc 1.49.0 (e1884a8e3 2020-12-29)
binary: rustc
commit-hash: e1884a8e3c3e813aada8254edfa120e85bf5ffca
commit-date: 2020-12-29
host: x86_64-unknown-linux-gnu
release: 1.49.0
```

The above output is from a `x86_64` Linux system. We see that the `host` triple is `x86_64-unknown-linux-gnu`, which includes the CPU architecture (`x86_64`), the vendor (`unknown`), the operating system (`linux`), and the [ABI] (`gnu`).

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

By compiling for our host triple, the Rust compiler and the linker assume that there is an underlying operating system such as Linux or Windows that uses the C runtime by default, which causes the linker errors. So to avoid the linker errors, we can compile for a different environment with no underlying operating system.

An example for such a bare metal environment is the `thumbv7em-none-eabihf` target triple, which describes an [embedded] [ARM] system. The details are not important, all that matters is that the target triple has no underlying operating system, which is indicated by the `none` in the target triple. To be able to compile for this target, we need to add it in rustup:

[embedded]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture

```
rustup target add thumbv7em-none-eabihf
```

This downloads a pre-compiled copy of the `core` library for the target. Afterwards we can build our freestanding executable for the target:

```
cargo build --target thumbv7em-none-eabihf
```

By passing a `--target` argument we [cross compile] our executable for a bare metal target system. Since the target system has no operating system, the linker does not try to link the C runtime and our build succeeds without any linker errors.

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

## Kernel Target

We just saw that we can compile our executable for a embedded ARM system by passing a `--target` argument. Rust supports [many different target triples][platform-support], including `arm-linux-androideabi` for Android or [`wasm32-unknown-unknown` for WebAssembly](https://www.hellorust.com/setup/wasm-target/).

[platform-support]: https://doc.rust-lang.org/nightly/rustc/platform-support.html

In order to create an operating system kernel, we need to choose a target that describes the environment on a bare-metal `x86_64` system. This requires some special configuration parameters (e.g. no underlying OS), so none of the officially supported target triples fit. Fortunately, Rust allows us to define [our own target][custom-targets] through a JSON file. For example, a JSON file that describes the `x86_64-unknown-linux-gnu` target looks like this:

[custom-targets]: https://doc.rust-lang.org/nightly/rustc/targets/custom.html

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

Most fields are required by LLVM to generate code for that platform. For example, the [`data-layout`] field defines the size of various integer, floating point, and pointer types. Then there are fields that Rust uses for conditional compilation, such as `target-pointer-width`. The third kind of fields define how the crate should be built. For example, the `pre-link-args` field specifies arguments passed to the [linker].

[`data-layout`]: https://llvm.org/docs/LangRef.html#data-layout
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
    "executables": true
}
```

Note that we changed the OS in the `llvm-target` and the `os` field to `none`, because our kernel will run on bare metal.

We add the following build-related entries:

- Override the default linker:

  ```json
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
  ```

  Instead of using the platform's default linker (which might not support our custom target), we use the cross platform [LLD] linker that is shipped with Rust for linking our kernel.

  [LLD]: https://lld.llvm.org/

- Abort on panic:

  ```json
  "panic-strategy": "abort",
  ```

  This setting specifies that the target doesn't support [stack unwinding] on panic, so instead the program should abort directly. This has the same effect as the `panic = "abort"` option in our Cargo.toml, so we can remove it from there.

  [stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php

- Disable the red zone:

  ```json
  "disable-redzone": true,
  ```

  We're writing a kernel, so we'll need to handle interrupts at some point. To do that safely, we have to disable a certain stack pointer optimization called the _“red zone”_, because it would cause stack corruptions otherwise. For more information, see our separate post about [disabling the red zone].

[disabling the red zone]: @/edition-2/posts/02-minimal-rust-kernel/disable-red-zone/index.md

- Disable SIMD:

  ```json
  "features": "-mmx,-sse,+soft-float",
  ```

  The `features` field enables/disables target features. We disable the `mmx` and `sse` features by prefixing them with a minus and enable the `soft-float` feature by prefixing it with a plus. Note that there must be no spaces between different flags, otherwise LLVM fails to interpret the features string.

  The `mmx` and `sse` features determine support for [Single Instruction Multiple Data (SIMD)] instructions, which can often speed up programs significantly. However, using the large SIMD registers in OS kernels leads to performance problems. The reason is that the kernel needs to restore all registers to their original state before continuing an interrupted program. This means that the kernel has to save the complete SIMD state to main memory on each system call or hardware interrupt. Since the SIMD state is very large (512–1600 bytes) and interrupts can occur very often, these additional save/restore operations considerably harm performance. To avoid this, we disable SIMD for our kernel (not for applications running on top!).

  [Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

  A problem with disabling SIMD is that floating point operations on `x86_64` require SIMD registers by default. To solve this problem, we add the `soft-float` feature, which emulates all floating point operations through software functions based on normal integers.

  For more information, see our post on [disabling SIMD](@/edition-2/posts/02-minimal-rust-kernel/disable-simd/index.md).

After adding all the above entries, our full target specification file looks like this:

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

## Building our Kernel

To build our kernel for our new custom target we pass the path to the JSON file as `--target` argument:

```
> cargo build --target x86_64-blog_os.json

error[E0463]: can't find crate for `core`
```

It fails! The error tells us that the Rust compiler no longer finds the [`core` library]. This library contains basic Rust types such as `Result`, `Option`, and iterators, and is implicitly linked to all `no_std` crates.

[`core` library]: https://doc.rust-lang.org/nightly/core/index.html

The problem is that the core library is distributed together with the Rust compiler as a precompiled library. These precompiled versions are available through `rustup` for all officially supported targets. We already saw this above, when we [built our kernel for the `thumbv7em-none-eabihf` target](#building-for-a-bare-metal-target). For our custom target, however, we need to build the `core` library ourselves.

While `cargo` has built-in support for building the `core` library, this feature is still considered [_unstable_][cargo-unstable]. Unstable features are only available in the "nightly" release channel of Rust, not on normal stable releases. So in order to build the `core` library, we need to install a nightly version of Rust first.

[cargo-unstable]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html

### Installing Rust Nightly
Rust has three release channels: _stable_, _beta_, and _nightly_. The Rust Book explains the difference between these channels really well, so take a minute and [check it out](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html#choo-choo-release-channels-and-riding-the-trains). Apart from the availability of unstable features, there is not really a difference between nightly and stable releases. Every 6 weeks, the current nightly is released on the beta channel and the current beta is released as stable. Since we will need some unstable features for our operating system (such as building `core`), we need to install a nightly version of Rust.

The recommend tool to manage Rust installations is [rustup]. It allows you to install nightly, beta, and stable compilers side-by-side and makes it easy to update them. With rustup you can use a nightly compiler for the current directory by running:

```
rustup override set nightly
```

Alternatively, you can add a file called **`rust-toolchain`** to the project's root directory with the required Rust version:

```toml
[toolchain]
channel = "nightly"
```

After doing one of these things, both the `cargo` and `rustc` command should use a nightly version of Rust when invoked from within the current directory. You can verify that you have a nightly version installed and active by running `rustc --version`: The version number should contain `-nightly` at the end, for example:

[rustup]: https://www.rustup.rs/

```
rustc 1.51.0-nightly (04caa632d 2021-01-30)
```

<div class="note">

Note that this version number is just an example, your version should be newer. This post and the rest of the blog is regularly updated to always compile on the newest nightly version. So if something doesn't work try updating to the latest nightly by running `rustup update nightly`.

</div>

In addition to building `core`, using a nightly compiler allows us to opt-in to [various experimental features] by using so-called _feature flags_ at the top of our file. For example, we could enable the experimental [`asm!` macro] for inline assembly by adding `#![feature(asm)]` to the top of our `main.rs`. Note that such experimental features are completely unstable, which means that future Rust versions might change or remove them without prior warning. For this reason we will only use them if absolutely necessary.

[various experimental features]: https://doc.rust-lang.org/unstable-book/the-unstable-book.html
[`asm!` macro]: https://doc.rust-lang.org/unstable-book/library-features/asm.html

### The `build-std` Option

Now that we switched to nightly Rust, we are able use the [`build-std` feature] of cargo. It allows to build `core` and other standard library crates on demand, instead of using the precompiled versions shipped with the Rust installation.

[`build-std` feature]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std

To build the `core` library, we need to pass a `-Z build-std=core` argument to the `cargo build` command:

```
> cargo build --target x86_64-blog_os.json -Z build-std=core

error: "/…/rustlib/src/rust/Cargo.lock" does not exist,
unable to build with the standard library, try:
    rustup component add rust-src
```

It still fails. The problem is that cargo needs a copy of the Rust source code in order to recompile the `core` crate. The error message helpfully suggest to provide such a copy by installing the `rust-src` component.

Instead of running the suggested `rustup component add rust-src` command, we an also record the dependency on the `rust-src` component in our `rust-toolchain` file:

```toml
# in rust-toolchain

[toolchain]
channel = "nightly"
components = ["rust-src"]
```

This way, `rustup` will automatically download the required components so that no manual steps are necessary.

After installing the `rust-src` component (either manually or automatically), the build should finally succeeds:

```
> cargo build --target x86_64-blog_os.json -Z build-std=core
   Compiling core v0.0.0 (/…/rust/src/libcore)
   Compiling rustc-std-workspace-core v1.99.0 (/…/rustc-std-workspace-core)
   Compiling compiler_builtins v0.1.32
   Compiling blog_os v0.1.0 (/…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

We see that `cargo build` now builds the `core`, `compiler_builtins` (a dependency of `core`), and `rustc-std-workspace-core` (a dependency of `compiler_builtins`) libraries for our custom target.

### Memory-Related Intrinsics

The Rust compiler assumes that a certain set of built-in functions is available for all systems. Most of these functions are provided by the `compiler_builtins` crate that we just built. However, there are some memory-related functions in that crate that are not enabled by default because they are normally provided by the C library on the system. These functions include `memset`, which sets all bytes in a memory block to a given value, `memcpy`, which copies one memory block to another, and `memcmp`, which compares two memory blocks. While we didn't need any of these functions to compile our kernel right now, they will be required as soon as we add some more code to it (e.g. when copying structs around).

Since we can't link to the C library of the operating system, we need an alternative way to provide these functions to the compiler. One possible approach for this could be to implement our own `memset` etc. functions and apply the `#[no_mangle]` attribute to them (to avoid the automatic renaming during compilation). However, this is dangerous since the slightest mistake in the implementation of these functions could lead to bugs and undefined behavior. For example, you might get an endless recursion when implementing `memcpy` using a `for` loop because `for` loops implicitly call the [`IntoIterator::into_iter`] trait method, which might call `memcpy` again. So it's a good idea to reuse existing well-tested implementations instead of creating your own.

[`IntoIterator::into_iter`]: https://doc.rust-lang.org/stable/core/iter/trait.IntoIterator.html#tymethod.into_iter

Fortunately, the `compiler_builtins` crate already contains implementations for all the needed functions, they are just disabled by default to not collide with the implementations from the C library. We can enable them by passing an additional `-Z build-std-features=compiler-builtins-mem` flag to `cargo`. Like the `build-std` flag, this [`build-std-features`] flag is still unstable, so it might change in the future.

[`build-std-features`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features

The full build command now looks like this:

```
cargo build --target x86_64-blog_os.json -Z build-std=core \
    -Z build-std-features=compiler-builtins-mem
```

Behind the scenes, the new flag enables the [`mem` feature] of the `compiler_builtins` crate. The effect of this is that the `#[no_mangle]` attribute is applied to the [`memcpy` etc. implementations] of the crate, which makes them available to the linker. It's worth noting that these functions are already optimized using [inline assembly] on `x86_64`, so their performance should be much better than a custom loop-based implementation.

[`mem` feature]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/Cargo.toml#L54-L55
[`memcpy` etc. implementations]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/src/mem.rs#L12-L69
[inline assembly]: https://doc.rust-lang.org/unstable-book/library-features/asm.html

With the additional `compiler-builtins-mem` flag, our kernel now has valid implementations for all compiler-required functions, so it will continue to compile even if our code gets more complex.

## A Shorter Build Command

Our build command is quite long now, so it's a bit cumbersome to type and difficult to remember. So let's try to shorten it!

### Setting Defaults

Since we want to always pass these flags to our build command, it would make sense to set them as default. Unfortunately, Cargo currently only supports changing the default build command through [`.cargo/config.toml`] configuration files. The problem with these files is that they are applied based on the current working directory, not based on the compiled project. This leads to [various problems][cargo-config-problems], for example that the settings also apply to all crates in subdirectories. These problems make `.cargo/config.toml` files unsuitable for our use case, since the code in the next post would be broken this way.

[`.cargo/config.toml`]: https://doc.rust-lang.org/cargo/reference/config.html
[cargo-config-problems]: https://internals.rust-lang.org/t/problems-of-cargo-config-files-and-possible-solutions/12987

To fix these problems, I proposed to [move some `.cargo/config.toml` settings to `Cargo.toml`][internals-proposal] to make them crate-specific. This would allow us to set proper defaults for our kernel too. So let's hope that it is implemented soon :). Until then, we can use _aliases_ to shorten our build command.

[internals-proposal]: https://internals.rust-lang.org/t/proposal-move-some-cargo-config-settings-to-cargo-toml/13336

### Aliases

Cargo allows to define custom [command aliases], for example `cargo br` for `cargo build --release`. While these aliases are defined in a `.cargo/config.toml` file too, they apply only to the command-line invocation and don't affect the normal build process of other crates. Thus, we can use them without problems.

[command aliases]: https://doc.rust-lang.org/cargo/reference/config.html#alias

To shorten our build command using an alias, we first need to create a directory named `.cargo` in the crate's root (i.e. next to the `Cargo.toml`). In that directory, we create a new file named `config.toml` with the following content:

```toml
[alias]
kbuild = """build --target x86_64-blog_os.json -Z build-std=core \
    -Z build-std-features=compiler-builtins-mem"""
```

This defines a new `kbuild` command (for "kernel build") that expands to the long build command of our kernel. Now we can build our kernel by running just:

```
cargo kbuild
```

The name of the alias doesn't matter, so you can also name the alias `kb` if you like it even shorter. Note that overriding the built-in `build` command is not possible.

One drawback of the alias approach is that you need to define a separate alias for every cargo subcommand (e.g. [`cargo check`] or [`cargo doc`]), which you want to use. You also need to adjust your IDE (e.g. [rust-analyzer]) to use a non-standard build/check command. So this approach is clearly just a workaound until proper package-specific defaults are implemented in Cargo.

[`cargo check`]: https://doc.rust-lang.org/cargo/commands/cargo-check.html
[`cargo doc`]: https://doc.rust-lang.org/cargo/commands/cargo-doc.html
[rust-analyzer]: https://rust-analyzer.github.io/

## What's next?

In the [next post], we will learn how to turn our minimal kernel in a bootable disk image, which can then be started in the [QEMU] virtual machine and on real hardware. For this, we'll explore the boot process of `x86_64` systems and learn about the differences between UEFI and the legacy BIOS firmware.

[next post]: @/edition-3/posts/02-booting/index.md
