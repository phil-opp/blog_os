+++
title = "最小化内核"
weight = 2
path = "zh-CN/minimal-rust-kernel"
date = 2018-02-10

+++

在这篇文章中，我们将基于**x86架构**（the x86 architecture），使用Rust语言，编写一个最小化的64位内核。我们将从上一章中构建的独立式可执行程序开始，构建自己的内核；它将向显示器打印字符串，并能被打包为一个能够引导启动的**磁盘映像**（disk image）。

[freestanding Rust binary]: @/second-edition/posts/01-freestanding-rust-binary/index.md

<!-- more -->

This blog is openly developed on [GitHub]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom]. The complete source code for this post can be found in the [`post-02`][post branch] branch.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-02

<!-- toc -->

## 引导启动

当我们启动电脑时，主板[ROM](https://en.wikipedia.org/wiki/Read-only_memory)内存储的**固件**（firmware）将会运行：它将负责电脑的**上电自检**（[power-on self test](https://en.wikipedia.org/wiki/Power-on_self-test)），**可用内存**（available RAM）的检测，以及CPU和其它硬件的预加载。这之后，它将寻找一个**可引导的存储介质**（bootable disk），并开始引导启动其中的**内核**（kernel）。

x86架构支持两种固件标准：**BIOS**（[Basic Input/Output System](https://en.wikipedia.org/wiki/BIOS)）和**UEFI**（[Unified Extensible Firmware Interface](https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface)）。其中，BIOS标准显得陈旧而过时，但实现简单，并为1980年代后的所有x86设备所支持；相反地，UEFI更现代化，功能也更全面，但开发和构建更复杂（至少从我的角度看是如此）。

在这篇文章中，我们暂时只提供BIOS固件的引导启动方式。

### BIOS启动

几乎所有的x86硬件系统都支持BIOS启动，这也包含新式的、基于UEFI、用**模拟BIOS**（emulated BIOS）的方式向后兼容的硬件系统。这可以说是一件好事情，因为无论是上世纪还是现在的硬件系统，你都只需编写同样的引导启动逻辑；但这种兼容性有时也是BIOS引导启动最大的缺点，因为这意味着在系统启动前，你的CPU必须先进入一个16位系统兼容的**实模式**（[real mode](https://en.wikipedia.org/wiki/Real_mode)），这样1980年代古老的引导固件才能够继续使用。

让我们从头开始，理解一遍BIOS启动的过程。

当电脑启动时，主板上特殊的闪存中存储的BIOS固件将被加载。BIOS固件将会上电自检、初始化硬件，然后它将寻找一个可引导的存储介质。如果找到了，那电脑的控制权将被转交给**引导程序**（bootloader）：一段存储在存储介质的开头的、512字节长度的程序片段。大多数的引导程序长度都大于512字节——所以通常情况下，引导程序都被切分为一段优先启动、长度不超过512字节、存储在介质开头的**第一阶段引导程序**（first stage bootloader），和一段随后由其加载的、长度可能较长、存储在其它位置的**第二阶段引导程序**（second stage bootloader）。

引导程序必须决定内核的位置，并将内核加载到内存。引导程序还需要将CPU从16位的实模式，先切换到32位的**保护模式**（[protected mode](https://en.wikipedia.org/wiki/Protected_mode)），最终切换到64位的**长模式**（[long mode](https://en.wikipedia.org/wiki/Long_mode)）：此时，所有的64位寄存器和整个**主内存**（main memory）才能被访问。引导程序的第三个作用，是从BIOS查询特定的信息，并将其传递到内核；如查询和传递**内存映射表**（memory map）。

编写一个引导程序并不是一个简单的任务，因为这需要使用汇编语言，而且必须经过许多意图并不明显的步骤——比如，把一些**魔术数字**（magic number）写入某个寄存器。因此，我们不会讲解如何编写自己的引导程序，而是推荐[bootimage工具](https://github.com/rust-osdev/bootimage)——它能够自动而方便地为你的内核准备一个引导程序。

### Multiboot标准

每个操作系统都实现自己的引导程序，而这只对单个操作系统有效。为了避免这样的僵局，1995年，**自由软件基金会**（[Free Software Foundation](https://en.wikipedia.org/wiki/Free_Software_Foundation)）颁布了一个开源的引导程序标准——[Multiboot](https://wiki.osdev.org/Multiboot)。这个标准定义了引导程序和操作系统间的统一接口，所以任何适配Multiboot的引导程序，都能用来加载任何同样适配了Multiboot的操作系统。[GNU GRUB](https://en.wikipedia.org/wiki/GNU_GRUB)是一个可供参考的Multiboot实现，它也是最热门的Linux系统引导程序之一。

要编写一款适配Multiboot的内核，我们只需要在内核文件开头，插入被称作**Multiboot头**（[Multiboot header](https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format)）的数据片段。这让GRUB很容易引导任何操作系统，但是，GRUB和Multiboot标准也有一些可预知的问题：

1. 它们只支持32位的保护模式。这意味着，在引导之后，你依然需要配置你的CPU，让它切换到64位的长模式；
2. 它们被设计为精简引导程序，而不是精简内核。举个栗子，内核需要以调整过的**默认页长度**（[default page size](https://wiki.osdev.org/Multiboot#Multiboot_2)）被链接，否则GRUB将无法找到内核的Multiboot头。另一个例子是**引导信息**（[boot information](https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format)），这个包含着大量与架构有关的数据，会在引导启动时，被直接传到操作系统，而不会经过一层清晰的抽象；
3. GRUB和Multiboot标准并没有被详细地注释，阅读相关文档需要一定经验；
4. 为了创建一个能够被引导的磁盘映像，我们在开发时必须安装GRUB：这加大了基于Windows或macOS开发内核的难度。

出于这些考虑，我们决定不使用GRUB或者Multiboot标准。然而，Multiboot支持功能也在bootimage工具的开发计划之中，所以从原理上讲，如果选用bootimage工具，在未来使用GRUB引导你的系统内核是可能的。

## 最小化内核

现在我们已经明白电脑是如何启动的，那也是时候编写我们自己的内核了。我们的小目标是，创建一个内核的磁盘映像，它能够在启动时，向屏幕输出一行“Hello World!”；我们的工作将基于上一章构建的独立式可执行程序。

如果读者还有印象的话，在上一章，我们使用`cargo`构建了一个独立的二进制程序；但这个程序依然基于特定的操作系统平台：因平台而异，我们需要定义不同名称的函数，且使用不同的编译指令。这是因为在默认情况下，`cargo`会为特定的**宿主系统**（host system）构建源码，比如为你正在运行的系统构建源码。这并不是我们想要的，因为我们的内核不应该基于另一个操作系统——我们想要编写的，就是这个操作系统。确切地说，我们想要的是，编译为一个特定的**目标系统**（target system）。

## 安装 Nightly Rust

Rust语言有三个**发行频道**（release channel），分别是stable、beta和nightly。《Rust程序设计语言》中对这三个频道的区别解释得很详细，可以前往[这里](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html)看一看。为了搭建一个操作系统，我们需要一些只有nightly会提供的实验性功能，所以我们需要安装一个nightly版本的Rust。

要管理安装好的Rust，我强烈建议使用[rustup](https://www.rustup.rs/)：它允许你同时安装nightly、beta和stable版本的编译器，而且让更新Rust变得容易。你可以输入`rustup override add nightly`来选择在当前目录使用nightly版本的Rust。或者，你也可以在项目根目录添加一个名称为`rust-toolchain`、内容为`nightly`的文件。要检查你是否已经安装了一个nightly，你可以运行`rustc --version`：返回的版本号末尾应该包含`-nightly`。

Nightly版本的编译器允许我们在源码的开头插入**特性标签**（feature flag），来自由选择并使用大量实验性的功能。举个栗子，要使用实验性的[内联汇编（asm!宏）](https://doc.rust-lang.org/nightly/unstable-book/language-features/asm.html)，我们可以在`main.rs`的顶部添加`#![feature(asm)]`。要注意的是，这样的实验性功能**不稳定**（unstable），意味着未来的Rust版本可能会修改或移除这些功能，而不会有预先的警告过渡。因此我们只有在绝对必要的时候，才应该使用这些特性。

### 目标配置清单

通过`--target`参数，`cargo`支持不同的目标系统。这个目标系统可以使用一个**目标三元组**（[target triple](https://clang.llvm.org/docs/CrossCompilation.html#target-triple)）来描述，它描述了CPU架构、平台供应者、操作系统和**应用程序二进制接口**（[Application Binary Interface, ABI](https://stackoverflow.com/a/2456882)）。比方说，目标三元组`x86_64-unknown-linux-gnu`描述一个基于`x86_64`架构CPU的、没有明确的平台供应者的linux系统，它遵循GNU风格的ABI。Rust支持[许多不同的目标三元组](https://forge.rust-lang.org/platform-support.html)，包括安卓系统对应的`arm-linux-androideabi`和[WebAssembly使用的wasm32-unknown-unknown](https://www.hellorust.com/setup/wasm-target/)。

为了编写我们的目标系统，鉴于我们需要做一些特殊的配置（比如没有依赖的底层操作系统），[已经支持的目标三元组](https://forge.rust-lang.org/platform-support.html)都不能满足我们的要求。幸运的是，只需使用一个JSON文件，Rust便允许我们定义自己的目标系统；这个文件常被称作**目标配置清单**（target specification）。比如，一个描述`x86_64-unknown-linux-gnu`目标系统的配置清单大概长这样：

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

一个配置清单中包含多个**配置项**（field）。大多数的配置项都是LLVM需求的，它们将配置为特定平台生成的代码。打个比方，`data-layout`配置项定义了不同的整数、浮点数、指针类型的长度；另外，还有一些Rust是用作条件变编译的配置项，如`target-pointer-width`。还有一些类型的配置项，定义了这个包该如何被编译，例如，`pre-link-args`配置项指定了该向**链接器**（[linker](https://en.wikipedia.org/wiki/Linker_(computing))）传入的参数。

我们将把我们的内核编译到`x86_64`架构，所以我们的配置清单将和上面的例子相似。现在，我们来创建一个名为`x86_64-blog_os.json`的文件——当然也可以选用自己喜欢的文件名——里面包含这样的内容：

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

需要注意的是，因为我们要在**裸机**（bare metal）上运行内核，我们已经修改了`llvm-target`的内容，并将`os`配置项的值改为`none`。

我们还需要添加下面与编译相关的配置项：

```json
"linker-flavor": "ld.lld",
"linker": "rust-lld",
```

在这里，我们不使用平台默认提供的链接器，因为它可能不支持Linux目标系统。为了链接我们的内核，我们使用跨平台的**LLD链接器**（[LLD linker](https://lld.llvm.org/)），它是和Rust打包发布的。

```json
"panic-strategy": "abort",
```

这个配置项的意思是，我们的编译目标不支持panic时的**栈展开**（[stack unwinding](http://www.bogotobogo.com/cplusplus/stackunwinding.php)），所以我们选择直接**在panic时中止**（abort on panic）。这和在`Cargo.toml`文件中添加`panic = "abort"`选项的作用是相同的，所以我们可以不在这里的配置清单中填写这一项。

```json
"disable-redzone": true,
```

我们正在编写一个内核，所以我们应该同时处理中断。要安全地实现这一点，我们必须禁用一个与**红区**（redzone）有关的栈指针优化：因为此时，这个优化可能会导致栈被破坏。我们撰写了一篇专门的短文，来更详细地解释红区及与其相关的优化。

```json
"features": "-mmx,-sse,+soft-float",
```

`features`配置项被用来启用或禁用某个目标**CPU特征**（CPU feature）。通过在它们前面添加`-`号，我们将`mmx`和`sse`特征禁用；添加前缀`+`号，我们启用了`soft-float`特征。

`mmx`和`sse`特征决定了是否支持**单指令多数据流**（[Single Instruction Multiple Data，SIMD](https://en.wikipedia.org/wiki/SIMD)）相关指令，这些指令常常能显著地提高程序层面的性能。然而，在内核中使用庞大的SIMD寄存器，可能会造成较大的性能影响：因为每次程序中断时，内核不得不储存整个庞大的SIMD寄存器以备恢复——这意味着，对每个硬件中断或系统调用，完整的SIMD状态必须存到主存中。由于SIMD状态可能相当大（512~1600个字节），而中断可能时常发生，这些额外的存储与恢复操作可能显著地影响效率。为解决这个问题，我们对内核禁用SIMD（但这不意味着禁用内核之上的应用程序的SIMD支持）。

禁用SIMD产生的一个问题是，`x86_64`架构的浮点数指针运算默认依赖于SIMD寄存器。我们的解决方法是，启用`soft-float`特征，它将使用基于整数的软件功能，模拟浮点数指针运算。

为了让读者的印象更清晰，我们撰写了一篇关于禁用SIMD的短文。

现在，我们将各个配置项整合在一起。我们的目标配置清单应该长这样：

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

### 编译内核

要编译我们的内核，我们将使用Linux系统的编写风格（这可能是LLVM的默认风格）。这意味着，我们需要把前一篇文章中编写的入口点重命名为`_start`：

```rust
// src/main.rs

#![no_std] // 不链接Rust标准库
#![no_main] // 禁用所有Rust层级的入口点

use core::panic::PanicInfo;

/// 这个函数将在panic时被调用
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle] // 不重整函数名
pub extern "C" fn _start() -> ! {
    // 因为编译器会寻找一个名为`_start`的函数，所以这个函数就是入口点
    // 默认命名为`_start`
    loop {}
}
```

注意的是，无论你开发使用的是哪类操作系统，你都需要将入口点命名为`_start`。前一篇文章中编写的Windows系统和macOS对应的入口点不应该被保留。

通过把JSON文件名传入`--target`选项，我们现在可以开始编译我们的内核。让我们试试看：

```
> cargo build --target x86_64-blog_os.json

error[E0463]: can't find crate for `core` 
（或者是下面的错误）
error[E0463]: can't find crate for `compiler_builtins`
```

哇哦，编译失败了！输出的错误告诉我们，Rust编译器找不到`core`或者`compiler_builtins`包；而所有`no_std`上下文都隐式地链接到这两个包。[`core`包](https://doc.rust-lang.org/nightly/core/index.html)包含基础的Rust类型，如`Result`、`Option`和迭代器等；[`compiler_builtins`包](https://github.com/rust-lang-nursery/compiler-builtins)提供LLVM需要的许多底层操作，比如`memcpy`。

通常状况下，`core`库以**预编译库**（precompiled library）的形式与Rust编译器一同发布——这时，`core`库只对支持的宿主系统有效，而我们自定义的目标系统无效。如果我们想为其它系统编译代码，我们需要为这些系统重新编译整个`core`库。

### Cargo xbuild

这就是为什么我们需要[cargo xbuild工具](https://github.com/rust-osdev/cargo-xbuild)。这个工具封装了`cargo build`；但不同的是，它将自动交叉编译`core`库和一些**编译器内建库**（compiler built-in libraries）。我们可以用下面的命令安装它：

```bash
cargo install cargo-xbuild
```

这个工具依赖于Rust的源代码；我们可以使用`rustup component add rust-src`来安装源代码。

现在我们可以使用`xbuild`代替`build`重新编译：

```bash
> cargo xbuild --target x86_64-blog_os.json
   Compiling core v0.0.0 (/…/rust/src/libcore)
   Compiling compiler_builtins v0.1.5
   Compiling rustc-std-workspace-core v1.0.0 (/…/rust/src/tools/rustc-std-workspace-core)
   Compiling alloc v0.0.0 (/tmp/xargo.PB7fj9KZJhAI)
    Finished release [optimized + debuginfo] target(s) in 45.18s
   Compiling blog_os v0.1.0 (file:///…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

我们能看到，`cargo xbuild`为我们自定义的目标交叉编译了`core`、`compiler_builtin`和`alloc`三个部件。这些部件使用了大量的**不稳定特性**（unstable features），所以只能在[nightly版本的Rust编译器](https://os.phil-opp.com/freestanding-rust-binary/#installing-rust-nightly)中工作。这之后，`cargo xbuild`成功地编译了我们的`blog_os`包。

现在我们可以为裸机编译内核了；但是，我们提供给引导程序的入口点`_start`函数还是空的。我们可以添加一些东西进去，不过我们可以先做一些优化工作。

### 设置默认目标

为了避免每次使用`cargo xbuild`时传递`--target`参数，我们可以覆写默认的编译目标。我们创建一个名为`.cargo/config`的[cargo配置文件](https://doc.rust-lang.org/cargo/reference/config.html)，添加下面的内容：

```toml
# in .cargo/config

[build]
target = "x86_64-blog_os.json"
```

这里的配置告诉`cargo`在没有显式声明目标的情况下，使用我们提供的`x86_64-blog_os.json`作为目标配置。这意味着保存后，我们可以直接使用：

```
cargo build
```

来编译我们的内核。[官方提供的一份文档](https://doc.rust-lang.org/cargo/reference/config.html)中有对cargo配置文件更详细的说明。

### 向屏幕打印字符

要做到这一步，最简单的方式是写入**VGA字符缓冲区**（[VGA text buffer](https://en.wikipedia.org/wiki/VGA-compatible_text_mode)）：这是一段映射到VGA硬件的特殊内存片段，包含着显示在屏幕上的内容。通常情况下，它能够存储25行、80列共2000个**字符单元**（character cell）；每个字符单元能够显示一个ASCII字符，也能设置这个字符的**前景色**（foreground color）和**背景色**（background color）。输出到屏幕的字符大概长这样：

![](https://upload.wikimedia.org/wikipedia/commons/6/6d/Codepage-737.png)

我们将在下篇文章中详细讨论VGA字符缓冲区的内存布局；目前我们只需要知道，这段缓冲区的地址是`0xb8000`，且每个字符单元包含一个ASCII码字节和一个颜色字节。

我们的实现就像这样：

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

在这段代码中，我们预先定义了一个**字节字符串**（byte string）类型的**静态变量**（static variable），名为`HELLO`。我们首先将整数`0xb8000`**转换**（cast）为一个**裸指针**（[raw pointer](https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer)）。这之后，我们迭代`HELLO`的每个字节，使用[enumerate](https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate)获得一个额外的序号变量`i`。在`for`语句的循环体中，我们使用[offset](https://doc.rust-lang.org/std/primitive.pointer.html#method.offset)偏移裸指针，解引用它，来将字符串的每个字节和对应的颜色字节——`0xb`代表淡青色——写入内存位置。

要注意的是，所有的裸指针内存操作都被一个**unsafe语句块**（[unsafe block](https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html)）包围。这是因为，此时编译器不能确保我们创建的裸指针是有效的；一个裸指针可能指向任何一个你内存位置；直接解引用并写入它，也许会损坏正常的数据。使用`unsafe`语句块时，程序员其实在告诉编译器，自己保证语句块内的操作是有效的。事实上，`unsafe`语句块并不会关闭Rust的安全检查机制；它允许你多做的事情[只有四件](https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html#unsafe-superpowers)。

使用`unsafe`语句块要求程序员有足够的自信，所以必须强调的一点是，**肆意使用unsafe语句块并不是Rust编程的一贯方式**。在缺乏足够经验的前提下，直接在`unsafe`语句块内操作裸指针，非常容易把事情弄得很糟糕；比如，在不注意的情况下，我们很可能会意外地操作缓冲区以外的内存。

在这样的前提下，我们希望最小化`unsafe `语句块的使用。使用Rust语言，我们能够将不安全操作将包装为一个安全的抽象模块。举个栗子，我们可以创建一个VGA缓冲区类型，把所有的不安全语句封装起来，来确保从类型外部操作时，无法写出不安全的代码：通过这种方式，我们只需要最少的`unsafe`语句块来确保我们不破坏**内存安全**（[memory safety](https://en.wikipedia.org/wiki/Memory_safety)）。在下一篇文章中，我们将会创建这样的VGA缓冲区封装。

## 启动内核

既然我们已经有了一个能够打印字符的可执行程序，是时候把它运行起来试试看了。首先，我们将编译完毕的内核与引导程序链接，来创建一个引导映像；这之后，我们可以在QEMU虚拟机中运行它，或者通过U盘在真机上运行。

### 创建引导映像

要将可执行程序转换为**可引导的映像**（bootable disk image），我们需要把它和引导程序链接。这里，引导程序将负责初始化CPU并加载我们的内核。

编写引导程序并不容易，所以我们不编写自己的引导程序，而是使用已有的[bootloader](https://crates.io/crates/bootloader)包；无需依赖于C语言，这个包基于Rust代码和内联汇编，实现了一个五脏俱全的BIOS引导程序。为了用它启动我们的内核，我们需要将它添加为一个依赖项，在`Cargo.toml`中添加下面的代码：

```toml
# in Cargo.toml

[dependencies]
bootloader = "0.6.0"
```

只添加引导程序为依赖项，并不足以创建一个可引导的磁盘映像；我们还需要内核编译完成之后，将内核和引导程序组合在一起。然而，截至目前，原生的cargo并不支持在编译完成后添加其它步骤（详见[这个issue](https://github.com/rust-lang/cargo/issues/545)）。

为了解决这个问题，我们建议使用`bootimage`工具——它将会在内核编译完毕后，将它和引导程序组合在一起，最终创建一个能够引导的磁盘映像。我们可以使用下面的命令来安装这款工具：

```bash
cargo install bootimage --version "^0.7.3"
```

参数`^0.7.3`是一个**脱字号条件**（[caret requirement](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#caret-requirements)），它的意义是“0.7.3版本或一个兼容0.7.3的新版本”。这意味着，如果这款工具发布了修复bug的版本`0.7.4`或`0.7.5`，cargo将会自动选择最新的版本，因为它依然兼容`0.7.x`；但cargo不会选择`0.8.0`，因为这个版本被认为并不和`0.7.x`系列版本兼容。需要注意的是，`Cargo.toml`中定义的依赖包版本都默认是脱字号条件：刚才我们指定`bootloader`包的版本时，遵循的就是这个原则。

为了运行`bootimage`以及编译引导程序，我们需要安装rustup模块`llvm-tools-preview`——我们可以使用`rustup component add llvm-tools-preview`来安装这个工具。

成功安装`bootimage`后，创建一个可引导的磁盘映像就变得相当容易。我们来输入下面的命令：

```bash
> cargo bootimage
```

可以看到的是，`bootimage`工具开始使用`cargo xbuild`编译你的内核，所以它将增量编译我们修改后的源码。在这之后，它会编译内核的引导程序，这可能将花费一定的时间；但和所有其它依赖包相似的是，在首次编译后，产生的二进制文件将被缓存下来——这将显著地加速后续的编译过程。最终，`bootimage`将把内核和引导程序组合为一个可引导的磁盘映像。

运行这行命令之后，我们应该能在`target/x86_64-blog_os/debug`目录内找到我们的映像文件`bootimage-blog_os.bin`。我们可以在虚拟机内启动它，也可以刻录到U盘上以便在真机上启动。（需要注意的是，因为文件格式不同，这里的bin文件并不是一个光驱映像，所以将它刻录到光盘不会起作用。）

事实上，在这行命令背后，`bootimage`工具执行了三个步骤：

1. 编译我们的内核为一个**ELF**（[Executable and Linkable Format](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format)）文件；
2. 编译引导程序为独立的可执行文件；
3. 将内核ELF文件**按字节拼接**（append by bytes）到引导程序的末端。

当机器启动时，引导程序将会读取并解析拼接在其后的ELF文件。这之后，它将把程序片段映射到**分页表**（page table）中的**虚拟地址**（virtual address），清零**BSS段**（BSS segment），还将创建一个栈。最终它将读取**入口点地址**（entry point address）——我们程序中`_start`函数的位置——并跳转到这个位置。

### 在QEMU中启动内核

现在我们可以在虚拟机中启动内核了。为了在[QEMU](https://www.qemu.org/)中启动内核，我们使用下面的命令：

```bash
> qemu-system-x86_64 -drive format=raw,file=bootimage-blog_os.bin
```

![](https://os.phil-opp.com/minimal-rust-kernel/qemu.png)

我们可以看到，屏幕窗口已经显示出“Hello World!”字符串。祝贺你！

### 在真机上运行内核

我们也可以使用dd工具把内核写入U盘，以便在真机上启动。可以输入下面的命令：

```bash
> dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

在这里，`sdX`是U盘的**设备名**（[device name](https://en.wikipedia.org/wiki/Device_file)）。请注意，**在选择设备名的时候一定要极其小心，因为目标设备上已有的数据将全部被擦除**。

写入到U盘之后，你可以在真机上通过引导启动你的系统。视情况而定，你可能需要在BIOS中打开特殊的启动菜单，或者调整启动顺序。需要注意的是，`bootloader`包暂时不支持UEFI，所以我们并不能在UEFI机器上启动。

### 使用`cargo run`

要让在QEMU中运行内核更轻松，我们可以设置在cargo配置文件中设置`runner`配置项：

```toml
# in .cargo/config

[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```

在这里，`target.'cfg(target_os = "none")'`筛选了三元组中宿主系统设置为`"none"`的所有编译目标——这将包含我们的`x86_64-blog_os.json`目标。另外，`runner`的值规定了运行`cargo run`使用的命令；这个命令将在成功编译后执行，而且会传递可执行文件的路径为第一个参数。[官方提供的cargo文档](https://doc.rust-lang.org/cargo/reference/config.html)讲述了更多的细节。

命令`bootimage runner`由`bootimage`包提供，参数格式经过特殊设计，可以用于`runner`命令。它将给定的可执行文件与项目的引导程序依赖项链接，然后在QEMU中启动它。`bootimage`包的[README文档](https://github.com/rust-osdev/bootimage)提供了更多细节和可以传入的配置参数。

现在我们可以使用`cargo xrun`来编译内核并在QEMU中启动了。和`xbuild`类似，`xrun`子命令将在调用cargo命令前编译内核所需的包。这个子命令也由`cargo-xbuild`工具提供，所以你不需要安装额外的工具。

## 下篇预告

在下篇文章中，我们将细致地探索VGA字符缓冲区，并包装它为一个安全的接口。我们还将基于它实现`println!`宏。
