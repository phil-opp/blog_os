+++
title = "Disable SIMD"
weight = 2
path = "zh-CN/disable-simd"
template = "edition-2/extra.html"
+++

[单指令多数据][Single Instruction Multiple Data (SIMD)] 指令允许在一个操作符（比如加法）内传入多组数据，以此加速程序执行速度。`x86_64` 架构支持多种SIMD标准：

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

<!-- more -->

- [MMX]: _多媒体扩展_ 指令集于1997年发布，定义了8个64位寄存器，分别被称为 `mm0` 到 `mm7`，不过，这些寄存器只是 [x87浮点执行单元][x87 floating point unit] 中寄存器的映射而已。
- [SSE]: _流处理SIMD扩展_ 指令集于1999年发布，不同于MMX的复用浮点执行单元，该指令集加入了一个完整的新寄存器组，即被称为 `xmm0` 到 `xmm15` 的16个128位寄存器。
- [AVX]: _先进矢量扩展_ 用于进一步扩展多媒体寄存器的数量，它定义了 `ymm0` 到 `ymm15` 共16个256位寄存器，但是这些寄存器继承于 `xmm`，例如 `xmm0` 寄存器是 `ymm0` 的低128位。

[MMX]: https://en.wikipedia.org/wiki/MMX_(instruction_set)
[x87 floating point unit]: https://en.wikipedia.org/wiki/X87
[SSE]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions
[AVX]: https://en.wikipedia.org/wiki/Advanced_Vector_Extensions

通过应用这些SIMD标准，计算机程序可以显著提高执行速度。优秀的编译器可以将常规循环自动优化为适用SIMD的代码，这种优化技术被称为 [自动矢量化][auto-vectorization]。

[auto-vectorization]: https://en.wikipedia.org/wiki/Automatic_vectorization

尽管如此，SIMD会让操作系统内核出现一些问题。具体来说，就是操作系统在处理硬件中断时，需要保存所有寄存器信息到内存中，在中断结束后再将其恢复以供使用。所以说，如果内核需要使用SIMD寄存器，那么每次处理中断需要备份非常多的数据（512-1600字节），这会显著地降低性能。要避免这部分性能损失，我们需要禁用 `sse` 和 `mmx` 这两个特性（`avx` 默认已禁用）。

我们可以在编译配置文件中的 `features` 配置项做出如下修改，加入以减号为前缀的 `mmx` 和 `sse` 即可：

```json
"features": "-mmx,-sse"
```

## 浮点数
还有一件不幸的事，`x86_64` 架构在处理浮点数计算时，会用到 `sse` 寄存器，因此，禁用SSE的前提下使用浮点数计算LLVM都一定会报错。 更大的问题在于Rust核心库里就存在着为数不少的浮点数运算（如 `f32` 和 `f64` 的数个trait），所以试图避免使用浮点数是不可能的。

幸运的是，LLVM支持 `soft-float` 特性，这个特性可以使用整型运算在软件层面模拟浮点数运算，使得我们为内核关闭SSE成为了可能，只需要牺牲一点点性能。

要为内核打开 `soft-float` 特性，我们只需要在编译配置文件中的 `features` 配置项做出如下修改即可：

```json
"features": "-mmx,-sse,+soft-float"
```
