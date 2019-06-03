+++
title = "Disable SIMD"
weight = 2
path = "disable-simd"

+++

[Single Instruction Multiple Data (SIMD)] instructions are able to perform an operation (e.g. addition) simultaneously on multiple data words, which can speed up programs significantly. The `x86_64` architecture supports various SIMD standards:

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

- [MMX]: The _Multi Media Extension_ instruction set was introduced in 1997 and defines eight 64 bit registers called `mm0` through `mm7`. These registers are just aliases for the registers of the [x87 floating point unit].
- [SSE]: The _Streaming SIMD Extensions_ instruction set was introduced in 1999. Instead of re-using the floating point registers, it adds a completely new register set. The sixteen new registers are called `xmm0` through `xmm15` and are 128 bits each.
- [AVX]: The _Advanced Vector Extensions_ are extensions that further increase the size of the multimedia registers. The new registers are called `ymm0` through `ymm15` and are 256 bits each. They extend the `xmm` registers, so e.g. `xmm0` is the lower half of `ymm0`.

[MMX]: https://en.wikipedia.org/wiki/MMX_(instruction_set)
[x87 floating point unit]: https://en.wikipedia.org/wiki/X87
[SSE]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions
[AVX]: https://en.wikipedia.org/wiki/Advanced_Vector_Extensions

By using such SIMD standards, programs can often speed up significantly. Good compilers are able to transform normal loops into such SIMD code automatically through a process called [auto-vectorization].

[auto-vectorization]: https://en.wikipedia.org/wiki/Automatic_vectorization

However, the large SIMD registers lead to problems in OS kernels. The reason is that the kernel has to backup all registers that it uses to memory on each hardware interrupt, because they need to have their original values when the interrupted program continues. So if the kernel uses SIMD registers, it has to backup a lot more data (512–1600 bytes), which noticeably decreases performance. To avoid this performance loss, we want to disable the `sse` and `mmx` features (the `avx` feature is disabled by default).

We can do that through the the `features` field in our target specification. To disable the `mmx` and `sse` features we add them prefixed with a minus:

```json
"features": "-mmx,-sse"
```

## Floating Point
Unfortunately for us, the `x86_64` architecture uses SSE registers for floating point operations. Thus, every use of floating point with disabled SSE causes an error in LLVM. The problem is that Rust's core library already uses floats (e.g., it implements traits for `f32` and `f64`), so avoiding floats in our kernel does not suffice.

Fortunately, LLVM has support for a `soft-float` feature, emulates all floating point operations through software functions based on normal integers. This makes it possible to use floats in our kernel without SSE, it will just be a bit slower.

To turn on the `soft-float` feature for our kernel, we add it to the `features` line in our target specification, prefixed with a plus:

```json
"features": "-mmx,-sse,+soft-float"
```
