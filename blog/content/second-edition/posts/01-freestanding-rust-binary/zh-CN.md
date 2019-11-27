+++
title = "独立的Rust二进制"
weight = 1
path = "freestanding-rust-binary"
date = 2018-02-10

+++

创建我们自己的操作系统内核的第一步是创建一个不链接标准库的Rust可执行文件。 这使得无需基础操作系统即可在[裸机][bare metal]上运行Rust代码。

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

此博客在[GitHub]上公开开发. 如果您有任何问题或疑问，请在此处打开一个问题。 您也可以在[底部][at the bottom]发表评论. 这篇文章的完整源代码可以在[`post-01`] [post branch]分支中找到。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## 介绍
要编写操作系统内核，我们需要不依赖于任何操作系统功能的代码。 这意味着我们不能使用线程，文件，堆内存，网络，随机数，标准输出或任何其他需要操作系统抽象或特定硬件的功能。这很有意义，因为我们正在尝试编写自己的OS和我们的驱动程序。

这意味着我们不能使用大多数[Rust标准库][Rust standard library]，但是我们可以使用很多Rust功能。 例如，我们可以使用[迭代器][iterators]，[闭包][closures]，[模式匹配][pattern matching]，[option]和[result]，[string formatting]，当然也可以使用[所有权系统][ownership system]。 这些功能使以一种非常有表现力的高级方式编写内核成为可能，而无需担心[不确定的行为][undefined behavior]或[内存安全性][memory safety]。

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

为了在Rust中创建OS内核，我们需要创建一个无需底层操作系统即可运行的可执行文件。 此类可执行文件通常称为“独立式”或“裸机”可执行文件。

这篇文章描述了创建一个独立的Rust二进制文件的必要步骤，并解释了为什么需要这些步骤。 如果您仅对一个最小的示例感兴趣，可以 **[跳转到摘要](＃summary)**。

## 禁用标准库
默认情况下，所有Rust crate都链接[标准库][standard library]，该库取决于操作系统的线程，文件或网络等功能。 它还依赖于C标准库“ libc”，该库与OS服务紧密交互。 由于我们的计划是编写一个操作系统，因此我们不能使用任何依赖于OS的库。因此，我们必须通过[`no_std` 属性][`no_std` attribute]禁用自动包含标准库。

[standard library]: https://doc.rust-lang.org/std/
[`no_std` attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

我们首先创建一个新的货物应用项目。 最简单的方法是通过命令行：

```
cargo new blog_os --bin --edition 2018
```

我将项目命名为`blog_os`，但是您当然可以选择自己的名字。 --bin标志指定我们要创建一个可执行二进制文件（与库相反），而--edition 2018标志指定我们要为crate使用Rust的[2018版][2018 edition]。 当我们运行命令时，cargo为我们创建以下目录结构：

[2018 edition]: https://rust-lang-nursery.github.io/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

在`Cargo.toml`包含crate构造，例如crate名称，作者，[语义化版本][semantic version]号码，和依赖关系。 `src/main.rs`文件包含crate的根模块和main函数。您可以通过`cargo build`来编译crate，然后在`target/debug`子文件夹中运行已编译的`blog_os`二进制文件。
[semantic version]: http://semver.org/

### `no_std` 属性

现在，我们的crate隐式链接了标准库。 让我们尝试通过添加[`no_std` 属性]禁用此功能：

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

当我们尝试立即构建它（通过运行`cargo build`）时，会发生以下错误：

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

发生此错误的原因是[`println`宏]是标准库的一部分，我们不再包含这个库。 因此我们无法再打印东西。这是有道理的，因为`println`写入[标准输出][standard output]，这是操作系统提供的特殊文件描述符。

[`println` macro]: https://doc.rust-lang.org/std/macro.println.html
[standard output]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

因此，让我们删除打印件，然后使用空的main函数重试：

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

现在，编译器缺少`#[panic_handler]`函数和_language item_。

## Panic 实现

`panic_handler`属性定义了发生[panic]时编译器应调用的函数。标准库提供了自己的应急处理函数，但是在`no_std`环境中，我们需要自己定义它：

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

[`PanicInfo`参数][PanicInfo]包含发生异常的文件和行以及可选的异常消息。该函数永远不应该返回，因此通过返回[“never” type] `!`将其标记为[diverging function]。 目前，我们无法在此函数中执行太多操作，因此我们只是做无限循环。

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[“never” type]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## `eh_personality` 语言项

语言项是编译器内部所需的特殊功能和类型。例如，[`Copy`]特征是一种语言项目，它告诉编译器哪些类型具有 [_copy语义_][`Copy`]。当我们查看[实现][copy code]时，我们看到它具有特殊的`#[lang = "copy"]`属性，将该属性定义为语言项。

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

提供自己的语言项目实现是可能的，但这只能作为最后的选择。 原因是语言项是高度不稳定的实现细节，甚至没有类型检查（因此编译器甚至不检查函数是否具有正确的参数类型）。幸运的是，有更稳定的方法来修复上述语言项错误。

`eh_personality`语言项标记了用于实现[堆栈展开][stack unwinding]的功能。默认情况下，Rust使用展开来运行所有活动堆栈变量的析构函数，以防出现[panic]情况。 这样可以确保释放所有使用的内存，并允许父线程捕获紧急情况并继续执行。但是，展开是一个复杂的过程，需要某些特定于操作系统的库（例如，Linux上的[libunwind]或Windows上的[结构化异常处理][structured exception handling]），因此我们不想在操作系统中使用它。

[stack unwinding]: http://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: http://www.nongnu.org/libunwind/
[structured exception handling]: https://msdn.microsoft.com/en-us/library/windows/desktop/ms680657(v=vs.85).aspx

### 禁用展开

还有其他一些用例，不希望展开，因此Rust提供了[中止异常][abort on panic]的选项。 这禁用了展开符号信息的生成，因此大大减小了二进制大小。我们可以在多个地方禁用展开功能。 最简单的方法是将以下几行添加到我们的`Cargo.toml`:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

这会将`dev`配置文件（用于`cargo build`）和`release`配置文件（用于`cargo build --release`）的应急策略设置为`abort`。 现在，不再需要`eh_personality`语言项目了。

[abort on panic]: https://github.com/rust-lang/rust/pull/32900

现在，我们修复了以上两个错误。 但是，如果我们现在尝试对其进行编译，则会发生另一个错误：

```
> cargo build
error: requires `start` lang_item
```

我们的程序缺少定义入口点的`start`语言项。

## `start` 属性

可能会认为`main`函数是运行程序时调用的第一个函数。 但是，大多数语言都有一个[运行时系统][runtime system]，它负责诸如垃圾回收（例如Java）或软件线程（例如Go中的goroutines）之类的事情。 这个运行时需要在`main`之前调用，因为它需要初始化自己。

[runtime system]: https://en.wikipedia.org/wiki/Runtime_system

在链接标准库的典型Rust二进制文件中，执行从名为`crt0`（“ C运行时零”）的C运行时库开始，该运行时库为C应用程序设置了环境。这包括创建堆栈并将参数放在正确的寄存器中。 然后，C运行时调用[Rust运行时的入口点][rt::lang_start]，该入口由`start`语言项标记。Rust的运行时非常短，它可以处理一些小事情，例如设置堆栈溢出防护或在紧急情况下打印回溯。 然后，运行时最终调用`start`函数。

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

Our freestanding executable does not have access to the Rust runtime and `crt0`, so we need to define our own entry point. Implementing the `start` language item wouldn't help, since it would still require `crt0`. Instead, we need to overwrite the `crt0` entry point directly.

### Overwriting the Entry Point
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

By using the `#[no_mangle]` attribute we disable the [name mangling] to ensure that the Rust compiler really outputs a function with the name `_start`. Without the attribute, the compiler would generate some cryptic `_ZN3blog_os4_start7hb173fedf945531caE` symbol to give every function an unique name. The attribute is required because we need to tell the name of the entry point function to the linker in the next step.

We also have to mark the function as `extern "C"` to tell the compiler that it should use the [C calling convention] for this function (instead of the unspecified Rust calling convention). The reason for naming the function `_start` is that this is the default entry point name for most systems.

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[C calling convention]: https://en.wikipedia.org/wiki/Calling_convention

The `!` return type means that the function is diverging, i.e. not allowed to ever return. This is required because the entry point is not called by any function, but invoked directly by the operating system or bootloader. So instead of returning, the entry point should e.g. invoke the [`exit` system call] of the operating system. In our case, shutting down the machine could be a reasonable action, since there's nothing left to do if a freestanding binary returns. For now, we fulfill the requirement by looping endlessly.

[`exit` system call]: https://en.wikipedia.org/wiki/Exit_(system_call)

When we run `cargo build` now, we get an ugly _linker_ error.

## Linker Errors

The linker is a program that combines the generated code into an executable. Since the executable format differs between Linux, Windows, and macOS, each system has its own linker that throws a different error. The fundamental cause of the errors is the same: the default configuration of the linker assumes that our program depends on the C runtime, which it does not.

To solve the errors, we need to tell the linker that it should not include the C runtime. We can do this either by passing a certain set of arguments to the linker or by building for a bare metal target.

### Building for a Bare Metal Target

By default Rust tries to build an executable that is able to run in your current system environment. For example, if you're using Windows on `x86_64`, Rust tries to build a `.exe` Windows executable that uses `x86_64` instructions. This environment is called your "host" system.

To describe different environments, Rust uses a string called [_target triple_]. You can see the target triple for your host system by running `rustc --version --verbose`:

[_target triple_]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple

```
rustc 1.35.0-nightly (474e7a648 2019-04-07)
binary: rustc
commit-hash: 474e7a6486758ea6fc761893b1a49cd9076fb0ab
commit-date: 2019-04-07
host: x86_64-unknown-linux-gnu
release: 1.35.0-nightly
LLVM version: 8.0
```

The above output is from a `x86_64` Linux system. We see that the `host` triple is `x86_64-unknown-linux-gnu`, which includes the CPU architecture (`x86_64`), the vendor (`unknown`), the operating system (`linux`), and the [ABI] (`gnu`).

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

By compiling for our host triple, the Rust compiler and the linker assume that there is an underlying operating system such as Linux or Windows that use the C runtime by default, which causes the linker errors. So to avoid the linker errors, we can compile for a different environment with no underlying operating system.

An example for such a bare metal environment is the `thumbv7em-none-eabihf` target triple, which describes an [embedded] [ARM] system. The details are not important, all that matters is that the target triple has no underlying operating system, which is indicated by the `none` in the target triple. To be able to compile for this target, we need to add it in rustup:

[embedded]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture

```
rustup target add thumbv7em-none-eabihf
```

This downloads a copy of the standard (and core) library for the system. Now we can build our freestanding executable for this target:

```
cargo build --target thumbv7em-none-eabihf
```

By passing a `--target` argument we [cross compile] our executable for a bare metal target system. Since the target system has no operating system, the linker does not try to link the C runtime and our build succeeds without any linker errors.

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

This is the approach that we will use for building our OS kernel. Instead of `thumbv7em-none-eabihf`, we will use a [custom target] that describes a `x86_64` bare metal environment. The details will be explained in the next post.

[custom target]: https://doc.rust-lang.org/rustc/targets/custom.html

### Linker Arguments

Instead of compiling for a bare metal system, it is also possible to resolve the linker errors by passing a certain set of arguments to the linker. This isn't the approach that we will use for our kernel, therefore this section is optional and only provided for completeness. Click on _"Linker Arguments"_ below to show the optional content.

<details>

<summary>Linker Arguments</summary>

In this section we discuss the linker errors that occur on Linux, Windows, and macOS, and explain how to solve them by passing additional arguments to the linker. Note that the executable format and the linker differ between operating systems, so that a different set of arguments is required for each system.

#### Linux

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

#### Windows

On Windows, a different linker error occurs (shortened):

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

#### macOS

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

[does not officially support statically linked binaries]: https://developer.apple.com/library/content/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

This still not suffices, as a third linker error occurs:

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

#### Unifying the Build Commands

Right now we have different build commands depending on the host platform, which is not ideal. To avoid this, we can create a file named `.cargo/config` that contains the platform specific arguments:

```toml
# in .cargo/config

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

The `rustflags` key contains arguments that are automatically added to every invocation of `rustc`. For more information on the `.cargo/config` file check out the [official documentation](https://doc.rust-lang.org/cargo/reference/config.html).

Now our program should be buildable on all three platforms with a simple `cargo build`.

#### Should You Do This?

While it's possible to build a freestanding executable for Linux, Windows, and macOS, it's probably not a good idea. The reason is that our executable still expects various things, for example that a stack is initialized when the `_start` function is called. Without the C runtime, some of these requirements might not be fulfilled, which might cause our program to fail, e.g. through a segmentation fault.

If you want to create a minimal binary that runs on top of an existing operating system, including `libc` and setting the `#[start]` attribute as described [here](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html) is probably a better idea.

</details>

## Summary

A minimal freestanding Rust binary looks like this:

`src/main.rs`:

```rust
#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use core::panic::PanicInfo;

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

`Cargo.toml`:

```toml
[package]
name = "crate_name"
version = "0.1.0"
authors = ["Author Name <author@example.com>"]

# the profile used for `cargo build`
[profile.dev]
panic = "abort" # disable stack unwinding on panic

# the profile used for `cargo build --release`
[profile.release]
panic = "abort" # disable stack unwinding on panic
```

To build this binary, we need to compile for a bare metal target such as `thumbv7em-none-eabihf`:

```
cargo build --target thumbv7em-none-eabihf
```

Alternatively, we can compile it for the host system by passing additional linker arguments:

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Note that this is just a minimal example of a freestanding Rust binary. This binary expects various things, for example that a stack is initialized when the `_start` function is called. **So it probably for any real use of such a binary, more steps are required**.

## What's next?

The [next post] explains the steps needed for turning our freestanding binary into a minimal operating system kernel. This includes creating a custom target, combining our executable with a bootloader, and learning how to print something to the screen.

[next post]: @/second-edition/posts/02-minimal-rust-kernel/index.md
