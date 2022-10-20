+++
title = "独立式可执行程序"
weight = 1
path = "zh-CN/freestanding-rust-binary"
date = 2018-02-10

[extra]
# Please update this when updating the translation
translation_based_on_commit = "096c044b4f3697e91d8e30a2e817e567d0ef21a2"
# GitHub usernames of the people that translated this post
translators = ["luojia65", "Rustin-Liu", "TheBegining"]
+++

创建一个不链接标准库的 Rust 可执行文件，将是我们迈出的第一步。无需底层操作系统的支撑，这样才能在**裸机**（[bare metal]）上运行 Rust 代码。

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

此博客在 [GitHub] 上公开开发. 如果您有任何问题或疑问，请在此处打开一个 issue。 您也可以在[底部][at the bottom]发表评论. 这篇文章的完整源代码可以在 [`post-01`] [post branch] 分支中找到。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## 简介

要编写一个操作系统内核，我们需要编写不依赖任何操作系统特性的代码。这意味着我们不能使用线程、文件、堆内存、网络、随机数、标准输出，或其它任何需要操作系统抽象和特定硬件的特性；因为我们正在编写自己的操作系统和硬件驱动。

实现这一点，意味着我们不能使用 [Rust标准库](https://doc.rust-lang.org/std/)的大部分；但还有很多 Rust 特性是我们依然可以使用的。比如说，我们可以使用[迭代器](https://doc.rust-lang.org/book/ch13-02-iterators.html)、[闭包](https://doc.rust-lang.org/book/ch13-01-closures.html)、[模式匹配](https://doc.rust-lang.org/book/ch06-00-enums.html)、[Option](https://doc.rust-lang.org/core/option/)、[Result](https://doc.rust-lang.org/core/result/index.html)、[字符串格式化](https://doc.rust-lang.org/core/macro.write.html)，当然还有[所有权系统](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)。这些功能让我们能够编写表达性强、高层抽象的操作系统，而无需关心[未定义行为](https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs)和[内存安全](https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention)。

为了用 Rust 编写一个操作系统内核，我们需要创建一个独立于操作系统的可执行程序。这样的可执行程序常被称作**独立式可执行程序**（freestanding executable）或**裸机程序**(bare-metal executable)。

在这篇文章里，我们将逐步地创建一个独立式可执行程序，并且详细解释为什么每个步骤都是必须的。如果读者只对最终的代码感兴趣，可以跳转到本篇文章的小结部分。

## 禁用标准库

在默认情况下，所有的 Rust **包**（crate）都会链接**标准库**（[standard library](https://doc.rust-lang.org/std/)），而标准库依赖于操作系统功能，如线程、文件系统、网络。标准库还与 **Rust 的 C 语言标准库实现库**（libc）相关联，它也是和操作系统紧密交互的。既然我们的计划是编写自己的操作系统，我们就需要不使用任何与操作系统相关的库——因此我们必须禁用**标准库自动引用**（automatic inclusion）。使用 [no_std 属性](https://doc.rust-lang.org/book/first-edition/using-rust-without-the-standard-library.html)可以实现这一点。

我们可以从创建一个新的 cargo 项目开始。最简单的办法是使用下面的命令：

```bash
cargo new blog_os --bin --edition 2018
```

在这里我把项目命名为 `blog_os`，当然读者也可以选择自己的项目名称。默认情况下，即使不显式指定，cargo 也会为我们添加`--bin` 选项，说明我们将要创建一个可执行文件（而不是一个库）； 另外 `--edition 2018` 参数指明了项目的包要使用 Rust 的 **2018 版次**（[2018 edition]），但在默认情况下，该参数会指向本地安装的最新版本。当我们成功执行这行指令后，cargo 为我们创建的目录结构如下：

[2018 edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

在这里，`Cargo.toml` 文件包含了包的**配置**（configuration），比如包的名称、作者、[semver版本](https://semver.org/) 和项目依赖项；`src/main.rs` 文件包含包的**根模块**（root module）和 main 函数。我们可以使用 `cargo build` 来编译这个包，然后在 `target/debug` 文件夹内找到编译好的 `blog_os` 二进制文件。

### no_std 属性

现在我们的包依然隐式地与标准库链接。为了禁用这种链接，我们可以尝试添加 [no_std 属性](https://doc.rust-lang.org/book/first-edition/using-rust-without-the-standard-library.html)：

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

看起来很顺利。当我们使用 `cargo build` 来编译的时候，却出现了下面的错误：

```rust
error: cannot find macro `println!` in this scope
 --> src\main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

出现这个错误的原因是：[println! 宏](https://doc.rust-lang.org/std/macro.println.html)是标准库的一部分，而我们的项目不再依赖于标准库。我们选择不再打印字符串。这也很好理解，因为 `println!` 将会向**标准输出**（[standard output](https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29)）打印字符，它依赖于特殊的文件描述符，而这是由操作系统提供的特性。

所以我们可以移除这行代码，使用一个空的 main 函数再次尝试编译：

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

现在我们发现，编译器缺少一个 `#[panic_handler]` 函数和一个**语言项**（language item）。

## 实现 panic 处理函数

`panic_handler` 属性定义了一个函数，它会在一个 panic 发生时被调用。标准库中提供了自己的 panic 处理函数，但在 `no_std` 环境中，我们需要定义一个自己的 panic 处理函数：

```rust
// in main.rs

use core::panic::PanicInfo;

/// 这个函数将在 panic 时被调用
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

类型为 [PanicInfo](https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html) 的参数包含了 panic 发生的文件名、代码行数和可选的错误信息。这个函数从不返回，所以他被标记为**发散函数**（[diverging function]）。发散函数的返回类型称作 **Never 类型**（["never" type](https://doc.rust-lang.org/nightly/std/primitive.never.html)），记为`!`。对这个函数，我们目前能做的很少，所以我们只需编写一个无限循环 `loop {}`。

[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions

## eh_personality 语言项

语言项是一些编译器需求的特殊函数或类型。举例来说，Rust 的 [Copy](https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html) trait 是一个这样的语言项，告诉编译器哪些类型需要遵循**复制语义**（[copy semantics](https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html)）——当我们查找 `Copy` trait 的[实现](https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299)时，我们会发现，一个特殊的 `#[lang = "copy"]` 属性将它定义为了一个语言项，达到与编译器联系的目的。

我们可以自己实现语言项，但这是下下策：目前来看，语言项是高度不稳定的语言细节实现，它们不会经过编译期类型检查（所以编译器甚至不确保它们的参数类型是否正确）。幸运的是，我们有更稳定的方式，来修复上面的语言项错误。

`eh_personality` 语言项标记的函数，将被用于实现**栈展开**（[stack unwinding](https://www.bogotobogo.com/cplusplus/stackunwinding.php)）。在使用标准库的情况下，当 panic 发生时，Rust 将使用栈展开，来运行在栈上所有活跃的变量的**析构函数**（destructor）——这确保了所有使用的内存都被释放，允许调用程序的**父进程**（parent thread）捕获 panic，处理并继续运行。但是，栈展开是一个复杂的过程，如 Linux 的 [libunwind](https://www.nongnu.org/libunwind/) 或 Windows 的**结构化异常处理**（[structured exception handling, SEH](https://docs.microsoft.com/en-us/windows/win32/debug/structured-exception-handling)），通常需要依赖于操作系统的库；所以我们不在自己编写的操作系统中使用它。

### 禁用栈展开

在其它一些情况下，栈展开并不是迫切需求的功能；因此，Rust 提供了**在 panic 时中止**（[abort on panic](https://github.com/rust-lang/rust/pull/32900)）的选项。这个选项能禁用栈展开相关的标志信息生成，也因此能缩小生成的二进制程序的长度。有许多方式能打开这个选项，最简单的方式是把下面的几行设置代码加入我们的 `Cargo.toml`：

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

这些选项能将 **dev 配置**（dev profile）和 **release 配置**（release profile）的 panic 策略设为 `abort`。`dev` 配置适用于 `cargo build`，而 `release` 配置适用于 `cargo build --release`。现在编译器应该不再要求我们提供 `eh_personality` 语言项实现。

现在我们已经修复了出现的两个错误，可以开始编译了。然而，尝试编译运行后，一个新的错误出现了：

```bash
> cargo build
error: requires `start` lang_item
```

## start 语言项

这里，我们的程序遗失了 `start` 语言项，它将定义一个程序的**入口点**（entry point）。

我们通常会认为，当运行一个程序时，首先被调用的是 `main` 函数。但是，大多数语言都拥有一个**运行时系统**（[runtime system](https://en.wikipedia.org/wiki/Runtime_system)），它通常为**垃圾回收**（garbage collection）或**绿色线程**（software threads，或 green threads）服务，如 Java 的 GC 或 Go 语言的协程（goroutine）；这个运行时系统需要在 main 函数前启动，因为它需要让程序初始化。

在一个典型的使用标准库的 Rust 程序中，程序运行是从一个名为 `crt0` 的运行时库开始的。`crt0` 意为 C runtime zero，它能建立一个适合运行 C 语言程序的环境，这包含了栈的创建和可执行程序参数的传入。在这之后，这个运行时库会调用 [Rust 的运行时入口点](https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73)，这个入口点被称作 **start语言项**（"start" language item）。Rust 只拥有一个极小的运行时，它被设计为拥有较少的功能，如爆栈检测和打印**栈轨迹**（stack trace）。这之后，这个运行时将会调用 main 函数。

我们的独立式可执行程序并不能访问 Rust 运行时或 `crt0` 库，所以我们需要定义自己的入口点。只实现一个 `start` 语言项并不能帮助我们，因为这之后程序依然要求 `crt0` 库。所以，我们要做的是，直接重写整个 `crt0` 库和它定义的入口点。

### 重写入口点

要告诉 Rust 编译器我们不使用预定义的入口点，我们可以添加 `#![no_main]` 属性。

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// 这个函数将在 panic 时被调用
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

读者也许会注意到，我们移除了 `main` 函数。原因很显然，既然没有底层运行时调用它，`main` 函数也失去了存在的必要性。为了重写操作系统的入口点，我们转而编写一个 `_start` 函数：

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

我们使用 `no_mangle` 标记这个函数，来对它禁用**名称重整**（[name mangling](https://en.wikipedia.org/wiki/Name_mangling)）——这确保 Rust 编译器输出一个名为 `_start` 的函数；否则，编译器可能最终生成名为 `_ZN3blog_os4_start7hb173fedf945531caE` 的函数，无法让链接器正确辨别。

我们还将函数标记为 `extern "C"`，告诉编译器这个函数应当使用 [C 语言的调用约定](https://en.wikipedia.org/wiki/Calling_convention)，而不是 Rust 语言的调用约定。函数名为 `_start` ，是因为大多数系统默认使用这个名字作为入口点名称。

与前文的 `panic` 函数类似，这个函数的返回值类型为`!`——它定义了一个发散函数，或者说一个不允许返回的函数。这一点很重要，因为这个入口点不会被任何函数调用，但将直接被操作系统或**引导程序**（bootloader）调用。所以作为函数返回的替代，这个入口点应该去调用，比如操作系统提供的 **exit 系统调用**（["exit" system call](https://en.wikipedia.org/wiki/Exit_(system_call))）函数。在我们编写操作系统的情况下，关机应该是一个合适的选择，因为**当一个独立式可执行程序返回时，不会留下任何需要做的事情**（there is nothing to do if a freestanding binary returns）。现在来看，我们可以添加一个无限循环，来满足对返回值类型的需求。

如果我们现在编译这段程序，会出来一大段不太好看的**链接器错误**（linker error）。

## 链接器错误

**链接器**（linker）是一个程序，它将生成的目标文件组合为一个可执行文件。不同的操作系统如 Windows、macOS、Linux，规定了不同的可执行文件格式，因此也各有自己的链接器，抛出不同的错误；但这些错误的根本原因还是相同的：链接器的默认配置假定程序依赖于C语言的运行时环境，但我们的程序并不依赖于它。

为了解决这个错误，我们需要告诉链接器，它不应该包含（include）C 语言运行环境。我们可以选择提供特定的**链接器参数**（linker argument），也可以选择编译为**裸机目标**（bare metal target）。

### 编译为裸机目标

在默认情况下，Rust 尝试适配当前的系统环境，编译可执行程序。举个例子，如果你使用 `x86_64` 平台的 Windows 系统，Rust 将尝试编译一个扩展名为 `.exe` 的 Windows 可执行程序，并使用 `x86_64` 指令集。这个环境又被称作为你的**宿主系统**（"host" system）。

为了描述不同的环境，Rust 使用一个称为**目标三元组**（target triple）的字符串。要查看当前系统的目标三元组，我们可以运行 `rustc --version --verbose`：

```
rustc 1.35.0-nightly (474e7a648 2019-04-07)
binary: rustc
commit-hash: 474e7a6486758ea6fc761893b1a49cd9076fb0ab
commit-date: 2019-04-07
host: x86_64-unknown-linux-gnu
release: 1.35.0-nightly
LLVM version: 8.0
```

上面这段输出来自一个 `x86_64` 平台下的 Linux 系统。我们能看到，`host` 字段的值为三元组 `x86_64-unknown-linux-gnu`，它包含了 CPU 架构 `x86_64` 、供应商 `unknown` 、操作系统 `linux` 和[二进制接口](https://en.wikipedia.org/wiki/Application_binary_interface) `gnu`。

Rust 编译器尝试为当前系统的三元组编译，并假定底层有一个类似于 Windows 或 Linux 的操作系统提供C语言运行环境——然而这将导致链接器错误。所以，为了避免这个错误，我们可以另选一个底层没有操作系统的运行环境。

这样的运行环境被称作裸机环境，例如目标三元组 `thumbv7em-none-eabihf` 描述了一个 ARM **嵌入式系统**（[embedded system](https://en.wikipedia.org/wiki/Embedded_system)）。我们暂时不需要了解它的细节，只需要知道这个环境底层没有操作系统——这是由三元组中的 `none` 描述的。要为这个目标编译，我们需要使用 rustup 添加它：

```
rustup target add thumbv7em-none-eabihf
```

这行命令将为目标下载一个标准库和 core 库。这之后，我们就能为这个目标构建独立式可执行程序了：

```
cargo build --target thumbv7em-none-eabihf
```

我们传递了 `--target` 参数，来为裸机目标系统**交叉编译**（[cross compile](https://en.wikipedia.org/wiki/Cross_compiler)）我们的程序。我们的目标并不包括操作系统，所以链接器不会试着链接 C 语言运行环境，因此构建过程成功会完成，不会产生链接器错误。

我们将使用这个方法编写自己的操作系统内核。我们不会编译到 `thumbv7em-none-eabihf`，而是使用描述 `x86_64` 环境的**自定义目标**（[custom target](https://doc.rust-lang.org/rustc/targets/custom.html)）。在下一篇文章中，我们将详细描述一些相关的细节。

### 链接器参数

我们也可以选择不编译到裸机系统，因为传递特定的参数也能解决链接器错误问题。虽然我们不会在后面使用到这个方法，为了教程的完整性，我们也撰写了专门的短文章，来提供这个途径的解决方案。
如有需要，请点击下方的 _"链接器参数"_ 按钮来展开可选内容。

<details>

<summary>链接器参数</summary>

在本章节中，我们讨论了Linux、Windows和macOS中遇到的链接错误，并阐述如何通过传递额外参数来解决这些错误。注意，由于不同操作系统的可执行文件内在格式不同，所以对于不同操作系统而言，所适用的额外参数也有所不同。

#### Linux

在Linux下，会触发以下链接错误（简化版）：

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

这里的问题在于，链接器默认包含了C启动例程，即构建名为 `_start` 的入口函数的地方。但其依赖一些C标准库 `libc` 中的符号，而我们已经使用 `no_std` 开关排除掉了这些符号，所以链接器报告了这些错误。要解决这个问题，我们需要通过 `-nostartfiles` 参数来告诉链接器不要使用C启动例程功能。

通过 `cargo rustc` 可以传递链接器参数，该命令和 `cargo build` 的效果完全一致，但是可以将参数传递给rust的底层编译器 `rustc`。`rustc` 支持 `-C link-arg` 参数，此参数可以传递参数给配套的链接器。那么以此推断，我们的编译语句可以这样写：

```
cargo rustc -- -C link-arg=-nostartfiles
```

现在我们编译出的程序就可以在Linux上独立运行了。

我们并不需要显式指定入口函数名，链接器默认会查找 `_start` 函数作为入口点。

#### Windows


在Windows下，会触发以下链接错误（简化版）：

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

错误信息 “entry point must be defined” 意味着链接器没有找到程序入口点。在Windows环境下，默认入口点[取决于使用的子系统][windows-subsystems]。对于 `CONSOLE` 子系统，链接器会寻找 `mainCRTStartup` 函数作为入口，而对于 `WINDOWS` 子系统，入口函数名叫做 `WinMainCRTStartup`。要复写掉入口函数名的默认设定，使其使用我们已经定义的 `_start` 函数，可以将 `/ENTRY` 参数传递给链接器：

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

显而易见，从链接参数上看，Windows平台使用的链接器和Linux平台是完全不同的。

此时可能你还会遇到这个链接错误：

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

该错误的原因是Windows平台下的可执行文件可以使用不同的[子系统][windows-subsystems]。一般而言，操作系统会如此判断：如果入口函数名叫 `main` ，则会使用 `CONSOLE` 子系统；若名叫 `WinMain` ，则会使用 `WINDOWS` 子系统。然而此时我们使用的入口函数名叫 `_start` ，两者都不是，此时就需要显式指定子系统：

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

这里我们使用了 `CONSOLE` 子系统，如果使用 `WINDOWS` 子系统其实也可以。但是多次使用 `-C link-arg` 参数大可不必，我们可以如上面一样，将一个引号包裹起来的以空格分隔的列表传递给 `-C link-arg` 参数。

现在我们编译出的程序就可以在Windows平台成功运行了。

#### macOS

在macOS下，会触发以下链接错误（简化版）：

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

该错误告诉我们链接器找不到入口函数 `main` （由于某些原因，macOS平台下，所有函数都会具有 `_` 前缀）。要重设入口函数名，我们可以传入链接器参数 `-e` ：

```
cargo rustc -- -C link-args="-e __start"
```

`-e` 参数可用于重设入口函数名。由于在macOS平台下，所有函数都具有 `_` 前缀，所以需要传入 `__start` ，而不是 `_start` 。

接下来，会出现一个新的链接错误：

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS [并未官方支持静态链接][does not officially support statically linked binaries] ，并且在默认情况下程序会链接 `libSystem` 库。要复写这个设定并进行静态链接，我们可以传入链接器参数 `-static` ：

[does not officially support statically linked binaries]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

然而问题并没有解决，链接器再次抛出了一个错误：

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

该错误的原因是macOS平台下的程序会默认链接 `crt0` （即“C runtime zero”）。 这个错误实际上和Linux平台上的错误类似，可以添加链接器参数 `-nostartfiles` 解决：

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

现在，我们的程序可以在macOS下编译成功了。

#### 统一编译命令

经过上面的章节，我们知道了在各个平台使用的编译命令是不同的，这十分不优雅。要解决这个问题，我们可以创建一个 `.cargo/config.toml` 文件，分别配置不同平台下所使用的参数：

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

对应的 `rustflags` 配置项的值可以自动被填充到 `rustc` 的运行参数中。要寻找 `.cargo/config.toml` 更多的用法，可以看一下 [官方文档](https://doc.rust-lang.org/cargo/reference/config.html)。

现在只需要运行 `cargo build` 即可在全部三个平台编译我们的程序了。

#### 我们真的需要做这些？

尽管我们可以在Linux、Windows和macOS编译出可执行程序，但这可能并非是个好主意。
因为我们的程序少了不少本该存在的东西，比如 `_start` 执行时的栈初始化。
失去了C运行时，部分基于它的依赖项很可能无法正确执行，这会造成程序出现各式各样的异常，比如segmentation fault（段错误）。

如果你希望创建一个基于已存在的操作系统的最小类库，建议引用 `libc` ，阅读 [这里](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html) 并恰当设定 `#[start]` 比较好。

</details>

## 小结

一个用 Rust 编写的最小化的独立式可执行程序应该长这样：

`src/main.rs`：

```rust
#![no_std] // 不链接 Rust 标准库
#![no_main] // 禁用所有 Rust 层级的入口点

use core::panic::PanicInfo;

#[no_mangle] // 不重整函数名
pub extern "C" fn _start() -> ! {
    // 因为编译器会寻找一个名为 `_start` 的函数，所以这个函数就是入口点
    // 默认命名为 `_start`
    loop {}
}

/// 这个函数将在 panic 时被调用
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

`Cargo.toml`：

```toml
[package]
name = "crate_name"
version = "0.1.0"
authors = ["Author Name <author@example.com>"]

# 使用 `cargo build` 编译时需要的配置
[profile.dev]
panic = "abort" # 禁用panic时栈展开

# 使用 `cargo build --release` 编译时需要的配置
[profile.release]
panic = "abort" # 禁用 panic 时栈展开
```

选用任意一个裸机目标来编译。比如对 `thumbv7em-none-eabihf`，我们使用以下命令：

```bash
cargo build --target thumbv7em-none-eabihf
```

另外，我们也可以选择以本地操作系统为目标进行编译：

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

要注意的是，现在我们的代码只是一个 Rust 编写的独立式可执行程序的一个例子。运行这个二进制程序还需要很多准备，比如在 `_start` 函数之前需要一个已经预加载完毕的栈。所以为了真正运行这样的程序，**我们还有很多事情需要做**。

## 下篇预览

下一篇文章要做的事情基于我们这篇文章的成果，它将详细讲述编写一个最小的操作系统内核需要的步骤：如何配置特定的编译目标，如何将可执行程序与引导程序拼接，以及如何把一些特定的字符串打印到屏幕上。

[next post]: @/edition-2/posts/02-minimal-rust-kernel/index.md
