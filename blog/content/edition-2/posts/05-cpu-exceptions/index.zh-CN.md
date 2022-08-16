+++
title = "CPU Exceptions"
weight = 5
path = "zh-CN/cpu-exceptions"
date  = 2018-06-17

[extra]
# Please update this when updating the translation
translation_based_on_commit = "096c044b4f3697e91d8e30a2e817e567d0ef21a2"
# GitHub usernames of the people that translated this post
translators = ["liuyuran"]
+++

CPU异常在很多情况下都有可能发生，比如访问无效的内存地址，或者在除法运算里除以0。为了处理这些错误，我们需要设置一个 _中断描述符表_ 来提供异常处理函数。在文章的最后，我们的内核将能够捕获 [断点异常][breakpoint exceptions] 并在处理后恢复正常执行。

[breakpoint exceptions]: https://wiki.osdev.org/Exceptions#Breakpoint

<!-- more -->

这个系列的blog在[GitHub]上开放开发，如果你有任何问题，请在这里开一个issue来讨论。当然你也可以在[底部][at the bottom]留言。你可以在[`post-05`][post branch]找到这篇文章的完整源码。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-05

<!-- toc -->

## 简述
异常信号会在当前指令触发错误时被触发，例如执行了除数为0的除法。当异常发生后，CPU会中断当前的工作，并立即根据异常类型调用错误处理函数。

在x86架构中，存在20种不同的CPU异常类型，以下为最重要的几种：

- **Page Fault**: 页错误是被非法内存访问出发的，例如当前指令试图访问未被映射过的页，或者试图写入只读页。
- **Invalid Opcode**: 该错误是说当前指令操作符无效，比如在不支持SSE的旧式CPU上执行了 [SSE 指令][SSE instructions]。
- **General Protection Fault**: 该错误的原因有很多，主要原因就是权限异常，即试图使用用户态代码执行核心指令，或者将保留字段写入特定寄存器中。
- **Double Fault**: 当错误发生时，CPU会尝试调用错误处理函数，但如果 _在调用错误处理函数过程中_ 再次发生错误，CPU就会触发该错误。另外，如果没有注册错误处理函数也会触发该错误。
- **Triple Fault**: 如果CPU调用了两次错误处理函数都没有成功，该错误会被抛出。这是一个致命级别的 _三重异常_，这意味着我们已经无法捕捉它，对于大多数操作系统而言，此时就应该重置数据并重启操作系统。

[SSE instructions]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions

在 [OSDev wiki][exceptions] 可以看到完整的异常类型列表。

[exceptions]: https://wiki.osdev.org/Exceptions

### 中断描述符表
要捕捉CPU异常，我们需要设置一个 _中断描述符表_ (IDT)，用来捕获每一个异常。由于硬件层面会不加验证的直接使用，所以我们需要根据预定义格式直接写入数据。符表的每一行都遵循如下的16位结构。

Type| Name                     | Description
----|--------------------------|-----------------------------------
u16 | Function Pointer [0:15]  | 处理函数地址的低位（最后16位）
u16 | GDT selector             | [全局描述符表][global descriptor table]中的代码段标记。
u16 | Options                  | （如下所述）
u16 | Function Pointer [16:31] | 处理函数地址的中位（中间16位）
u32 | Function Pointer [32:63] | 处理函数地址的高位（剩下的所有位）
u32 | Reserved                 |

[global descriptor table]: https://en.wikipedia.org/wiki/Global_Descriptor_Table

Options字段的格式如下：

Bits  | Name                              | Description
------|-----------------------------------|-----------------------------------
0-2   | Interrupt Stack Table Index       | 0: 不要切换栈数据, 1-7: 当处理函数被调用时，切换到中断栈表的第n层栈。
3-7   | Reserved              |
8     | 0: Interrupt Gate, 1: Trap Gate   | 如果该比特被置为0，当处理函数被调用时，中断会被禁用。
9-11  | must be one                       |
12    | must be zero                      |
13‑14 | Descriptor Privilege Level (DPL)  | 执行处理函数所需的最小特权等级。
15    | Present                           |

每个异常都具有一个预定义的IDT序号，比如 invalid opcode 异常对应6号，而 page fault 异常对应14号，因此硬件可以直接寻找到对应的IDT条目。 OSDev wiki中的 [异常对照表][exceptions] 可以查到所有异常的IDT序号（在Vector nr.列）。

通常而言，当异常发生时，CPU会执行如下步骤：

1. 将一些寄存器数据推入栈中，包括指令指针以及 [RFLAGS] 寄存器。（我们会在文章稍后些的地方用到这些数据。）
2. 读取中断描述符表（IDT）的对应条目，比如当发生 page fault 异常时，调用14号条目。
3. 判断该条目确实存在，如果不存在，则触发 double fault 异常。
4. 如果该条目属于中断门（interrupt gate，bit 40 被设置为0），则禁用硬件中断。
5. 将 [GDT] 选择器载入代码段寄存器（CS segment）。
6. 跳转执行处理函数。

[RFLAGS]: https://en.wikipedia.org/wiki/FLAGS_register
[GDT]: https://en.wikipedia.org/wiki/Global_Descriptor_Table

不过现在我们不必为4和5多加纠结，未来我们会单独讲解全局描述符表和硬件中断的。

## IDT类型
与其创建我们自己的IDT类型映射，不如直接使用 `x86_64` crate 内置的 [`InterruptDescriptorTable` struct]，其实现是这样的：

[`InterruptDescriptorTable` struct]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html

``` rust
#[repr(C)]
pub struct InterruptDescriptorTable {
    pub divide_by_zero: Entry<HandlerFunc>,
    pub debug: Entry<HandlerFunc>,
    pub non_maskable_interrupt: Entry<HandlerFunc>,
    pub breakpoint: Entry<HandlerFunc>,
    pub overflow: Entry<HandlerFunc>,
    pub bound_range_exceeded: Entry<HandlerFunc>,
    pub invalid_opcode: Entry<HandlerFunc>,
    pub device_not_available: Entry<HandlerFunc>,
    pub double_fault: Entry<HandlerFuncWithErrCode>,
    pub invalid_tss: Entry<HandlerFuncWithErrCode>,
    pub segment_not_present: Entry<HandlerFuncWithErrCode>,
    pub stack_segment_fault: Entry<HandlerFuncWithErrCode>,
    pub general_protection_fault: Entry<HandlerFuncWithErrCode>,
    pub page_fault: Entry<PageFaultHandlerFunc>,
    pub x87_floating_point: Entry<HandlerFunc>,
    pub alignment_check: Entry<HandlerFuncWithErrCode>,
    pub machine_check: Entry<HandlerFunc>,
    pub simd_floating_point: Entry<HandlerFunc>,
    pub virtualization: Entry<HandlerFunc>,
    pub security_exception: Entry<HandlerFuncWithErrCode>,
    // some fields omitted
}
```

每一个字段都是 [`idt::Entry<F>`] 类型，这个类型包含了一条完整的IDT条目（定义参见上文）。 其泛型参数 `F` 定义了错误处理函数的类型，在有些字段中该参数为 [`HandlerFunc`]，而有些则是 [`HandlerFuncWithErrCode`]，而对于 page fault 这种特殊异常，则为 [`PageFaultHandlerFunc`]。

[`idt::Entry<F>`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.Entry.html
[`HandlerFunc`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.HandlerFunc.html
[`HandlerFuncWithErrCode`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.HandlerFuncWithErrCode.html
[`PageFaultHandlerFunc`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.PageFaultHandlerFunc.html

首先让我们看一看 `HandlerFunc` 类型的定义：

```rust
type HandlerFunc = extern "x86-interrupt" fn(_: InterruptStackFrame);
```

这是一个针对 `extern "x86-interrupt" fn` 类型的 [类型别名][type alias]。`extern` 关键字使用 [外部调用约定][foreign calling convention] 定义了一个函数，这种定义方式多用于和C语言代码通信（`extern "C" fn`），那么这里的 `x86-interrupt` 又是在调用什么地方？

[type alias]: https://doc.rust-lang.org/book/ch19-04-advanced-types.html#creating-type-synonyms-with-type-aliases
[foreign calling convention]: https://doc.rust-lang.org/nomicon/ffi.html#foreign-calling-conventions

## 中断调用约定
异常触发十分类似于函数调用：CPU会直接跳转到处理函数的第一个指令处开始执行，执行结束后，CPU会跳转到返回地址，并继续执行之前的函数调用。

尽管如此，两者还是有一些不同点的：函数调用是由编译器通过 `call` 指令主动发起的，而错误处理函数则可能会由 _任何_ 指令触发。要了解这两者所造成影响的不同，我们需要更深入的追踪函数调用。

[调用约定][Calling conventions] 指定了函数调用的详细信息，比如可以指定函数的参数存放在哪里（寄存器，或者栈，或者别的什么地方）以及返回值是什么样子的。在 x86_64 Linux 中，以下规则适用于C语言函数（参见 [System V ABI] 标准）：

[Calling conventions]: https://en.wikipedia.org/wiki/Calling_convention
[System V ABI]: https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf

- 前六个整型参数从寄存器传入 `rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9`
- 其他参数从栈传入
- 函数返回值存放在 `rax` 和 `rdx`

注意，Rust并不遵循C ABI，而是遵循自己的一套规则，即 [非正式的 Rust ABI 草案][rust abi]，所以这些规则仅在使用 `extern "C" fn` 对函数进行定义时才会使用。

[rust abi]: https://github.com/rust-lang/rfcs/issues/600

### 保留寄存器和临时寄存器
调用约定将寄存器分为两部分：_保留寄存器_ 和 _临时寄存器_ 。

_保留寄存器_ 的值应当在函数调用时保持不变，所以已执行的函数（_“callee”_）仅被允许在返回之前将这些寄存器的值恢复到初始值，因而这些寄存器又被称为 _“callee-saved”_。 在函数开始时将这类寄存器的值存入栈中，并在返回之前将之恢复到寄存器中是一种十分常见的做法。

而 _临时寄存器_ 则相反，被调用函数可以无限制的反复写入寄存器，若调用者希望此类寄存器在函数调用后保持数值不变，则需要自己来处理备份和恢复过程（例如将其数值保存在栈中），因而这类寄存器又被称为 _caller-saved_。

在 x86_64 架构下，C调用约定指定了这些寄存器分类：

保留寄存器 | 临时寄存器
---|---
`rbp`, `rbx`, `rsp`, `r12`, `r13`, `r14`, `r15` | `rax`, `rcx`, `rdx`, `rsi`, `rdi`, `r8`, `r9`, `r10`, `r11`
_callee-saved_ | _caller-saved_

编译器已经内置了这些规则，因而可以自动生成保证程序正常执行的指令。例如绝大多数函数的汇编指令都以 `push rbp` 开头，也就是将 `rbp` 的值备份到栈中（因为它是 `callee-saved` 型寄存器）。

### 保存所有寄存器数据
区别于函数调用，异常在执行 _任何_ 指令时都有可能发生。在大多数情况下，我们在编译期不可能知道程序跑起来会发生什么异常。比如编译器无法预知某条指令是否会触发 page fault 或者 stack overflow。

正因为我们无法预知异常发生的时刻，所以提前保存寄存器数据是不可能实现的。这也就意味着我们不能使用调用约定来自动化的处理错误处理函数 caller-saved 类型的寄存器上下文。所以此时我们需要一个调用约定来处理 _所有寄存器_ 的数据，巧了，`x86-interrupt` 就是这样的一个调用约定，它可以确保所有寄存器在函数返回之后恢复函数被调用之前的状态。

但是请注意，这并不意味着所有寄存器信息都会被存储到栈里，编译器仅仅会存储那些在函数中被写入的寄存器的信息。因此，仅使用极少量寄存器的简短函数在编译后会十分高效。

### 中断栈帧
当一个常规函数调用发生时（使用 `call` 指令），CPU会在跳转目标函数之前，将返回地址推入栈中。当函数返回时（使用 `ret` 指令），CPU会在跳回目标函数之前弹出返回地址。所以常规函数调用的栈帧看起来是这样的：

![function stack frame](function-stack-frame.svg)

对于错误处理函数和中断处理函数，仅仅推入一个返回地址并不足够，因为中断处理函数通常会运行在一个不那么一样的上下文中（栈指针、CPU flags等等）。所以CPU在遇到中断发生时是这么处理的：

1. **对其栈指针**: An interrupt can occur at any instructions, so the stack pointer can have any value, too. However, some CPU instructions (e.g. some SSE instructions) require that the stack pointer is aligned on a 16 byte boundary, therefore the CPU performs such an alignment right after the interrupt.
2. **切换堆栈** (in some cases): A stack switch occurs when the CPU privilege level changes, for example when a CPU exception occurs in a user mode program. It is also possible to configure stack switches for specific interrupts using the so-called _Interrupt Stack Table_ (described in the next post).
3. **推入旧的栈指针**: The CPU pushes the values of the stack pointer (`rsp`) and the stack segment (`ss`) registers at the time when the interrupt occurred (before the alignment). This makes it possible to restore the original stack pointer when returning from an interrupt handler.
4. **推入并更新 `RFLAGS` 寄存器**: The [`RFLAGS`] register contains various control and status bits. On interrupt entry, the CPU changes some bits and pushes the old value.
5. **推入指令指针**: Before jumping to the interrupt handler function, the CPU pushes the instruction pointer (`rip`) and the code segment (`cs`). This is comparable to the return address push of a normal function call.
6. **推入错误码** (for some exceptions): For some specific exceptions such as page faults, the CPU pushes an error code, which describes the cause of the exception.
7. **执行中断处理函数**: The CPU reads the address and the segment descriptor of the interrupt handler function from the corresponding field in the IDT. It then invokes this handler by loading the values into the `rip` and `cs` registers.

[`RFLAGS`]: https://en.wikipedia.org/wiki/FLAGS_register

所以 _中断栈帧_ 看起来是这样的：

![interrupt stack frame](exception-stack-frame.svg)

In the `x86_64` crate, the interrupt stack frame is represented by the [`InterruptStackFrame`] struct. It is passed to interrupt handlers as `&mut` and can be used to retrieve additional information about the exception's cause. The struct contains no error code field, since only some few exceptions push an error code. These exceptions use the separate [`HandlerFuncWithErrCode`] function type, which has an additional `error_code` argument.

[`InterruptStackFrame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptStackFrame.html

### Behind the Scenes
The `x86-interrupt` calling convention is a powerful abstraction that hides almost all of the messy details of the exception handling process. However, sometimes it's useful to know what's happening behind the curtain. Here is a short overview of the things that the `x86-interrupt` calling convention takes care of:

- **Retrieving the arguments**: Most calling conventions expect that the arguments are passed in registers. This is not possible for exception handlers, since we must not overwrite any register values before backing them up on the stack. Instead, the `x86-interrupt` calling convention is aware that the arguments already lie on the stack at a specific offset.
- **Returning using `iretq`**: Since the interrupt stack frame completely differs from stack frames of normal function calls, we can't return from handlers functions through the normal `ret` instruction. Instead, the `iretq` instruction must be used.
- **Handling the error code**: The error code, which is pushed for some exceptions, makes things much more complex. It changes the stack alignment (see the next point) and needs to be popped off the stack before returning. The `x86-interrupt` calling convention handles all that complexity. However, it doesn't know which handler function is used for which exception, so it needs to deduce that information from the number of function arguments. That means that the programmer is still responsible to use the correct function type for each exception. Luckily, the `InterruptDescriptorTable` type defined by the `x86_64` crate ensures that the correct function types are used.
- **Aligning the stack**: There are some instructions (especially SSE instructions) that require a 16-byte stack alignment. The CPU ensures this alignment whenever an exception occurs, but for some exceptions it destroys it again later when it pushes an error code. The `x86-interrupt` calling convention takes care of this by realigning the stack in this case.

If you are interested in more details: We also have a series of posts that explains exception handling using [naked functions] linked [at the end of this post][too-much-magic].

[naked functions]: https://github.com/rust-lang/rfcs/blob/master/text/1201-naked-fns.md
[too-much-magic]: #too-much-magic

## Implementation
Now that we've understood the theory, it's time to handle CPU exceptions in our kernel. We'll start by creating a new interrupts module in `src/interrupts.rs`, that first creates an `init_idt` function that creates a new `InterruptDescriptorTable`:

``` rust
// in src/lib.rs

pub mod interrupts;

// in src/interrupts.rs

use x86_64::structures::idt::InterruptDescriptorTable;

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
}
```

Now we can add handler functions. We start by adding a handler for the [breakpoint exception]. The breakpoint exception is the perfect exception to test exception handling. Its only purpose is to temporarily pause a program when the breakpoint instruction `int3` is executed.

[breakpoint exception]: https://wiki.osdev.org/Exceptions#Breakpoint

The breakpoint exception is commonly used in debuggers: When the user sets a breakpoint, the debugger overwrites the corresponding instruction with the `int3` instruction so that the CPU throws the breakpoint exception when it reaches that line. When the user wants to continue the program, the debugger replaces the `int3` instruction with the original instruction again and continues the program. For more details, see the ["_How debuggers work_"] series.

["_How debuggers work_"]: https://eli.thegreenplace.net/2011/01/27/how-debuggers-work-part-2-breakpoints

For our use case, we don't need to overwrite any instructions. Instead, we just want to print a message when the breakpoint instruction is executed and then continue the program. So let's create a simple `breakpoint_handler` function and add it to our IDT:

```rust
// in src/interrupts.rs

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::println;

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}
```

Our handler just outputs a message and pretty-prints the interrupt stack frame.

When we try to compile it, the following error occurs:

```
error[E0658]: x86-interrupt ABI is experimental and subject to change (see issue #40180)
  --> src/main.rs:53:1
   |
53 | / extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
54 | |     println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
55 | | }
   | |_^
   |
   = help: add #![feature(abi_x86_interrupt)] to the crate attributes to enable
```

This error occurs because the `x86-interrupt` calling convention is still unstable. To use it anyway, we have to explicitly enable it by adding `#![feature(abi_x86_interrupt)]` on the top of our `lib.rs`.

### Loading the IDT
In order that the CPU uses our new interrupt descriptor table, we need to load it using the [`lidt`] instruction. The `InterruptDescriptorTable` struct of the `x86_64` provides a [`load`][InterruptDescriptorTable::load] method function for that. Let's try to use it:

[`lidt`]: https://www.felixcloutier.com/x86/lgdt:lidt
[InterruptDescriptorTable::load]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html#method.load

```rust
// in src/interrupts.rs

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.load();
}
```

When we try to compile it now, the following error occurs:

```
error: `idt` does not live long enough
  --> src/interrupts/mod.rs:43:5
   |
43 |     idt.load();
   |     ^^^ does not live long enough
44 | }
   | - borrowed value only lives until here
   |
   = note: borrowed value must be valid for the static lifetime...
```

So the `load` methods expects a `&'static self`, that is a reference that is valid for the complete runtime of the program. The reason is that the CPU will access this table on every interrupt until we load a different IDT. So using a shorter lifetime than `'static` could lead to use-after-free bugs.

In fact, this is exactly what happens here. Our `idt` is created on the stack, so it is only valid inside the `init` function. Afterwards the stack memory is reused for other functions, so the CPU would interpret random stack memory as IDT. Luckily, the `InterruptDescriptorTable::load` method encodes this lifetime requirement in its function definition, so that the Rust compiler is able to prevent this possible bug at compile time.

In order to fix this problem, we need to store our `idt` at a place where it has a `'static` lifetime. To achieve this we could allocate our IDT on the heap using [`Box`] and then convert it to a `'static` reference, but we are writing an OS kernel and thus don't have a heap (yet).

[`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html


As an alternative we could try to store the IDT as a `static`:

```rust
static IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
    IDT.breakpoint.set_handler_fn(breakpoint_handler);
    IDT.load();
}
```

However, there is a problem: Statics are immutable, so we can't modify the breakpoint entry from our `init` function. We could solve this problem by using a [`static mut`]:

[`static mut`]: https://doc.rust-lang.org/1.30.0/book/second-edition/ch19-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable

```rust
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
    unsafe {
        IDT.breakpoint.set_handler_fn(breakpoint_handler);
        IDT.load();
    }
}
```

This variant compiles without errors but it's far from idiomatic. `static mut`s are very prone to data races, so we need an [`unsafe` block] on each access.

[`unsafe` block]: https://doc.rust-lang.org/1.30.0/book/second-edition/ch19-01-unsafe-rust.html#unsafe-superpowers

#### Lazy Statics to the Rescue
Fortunately the `lazy_static` macro exists. Instead of evaluating a `static` at compile time, the macro performs the initialization when the `static` is referenced the first time. Thus, we can do almost everything in the initialization block and are even able to read runtime values.

We already imported the `lazy_static` crate when we [created an abstraction for the VGA text buffer][vga text buffer lazy static]. So we can directly use the `lazy_static!` macro to create our static IDT:

[vga text buffer lazy static]: @/edition-2/posts/03-vga-text-buffer/index.md#lazy-statics

```rust
// in src/interrupts.rs

use lazy_static::lazy_static;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}
```

Note how this solution requires no `unsafe` blocks. The `lazy_static!` macro does use `unsafe` behind the scenes, but it is abstracted away in a safe interface.

### Running it

The last step for making exceptions work in our kernel is to call the `init_idt` function from our `main.rs`. Instead of calling it directly, we introduce a general `init` function in our `lib.rs`:

```rust
// in src/lib.rs

pub fn init() {
    interrupts::init_idt();
}
```

With this function we now have a central place for initialization routines that can be shared between the different `_start` functions in our `main.rs`, `lib.rs`, and integration tests.

Now we can update the `_start` function of our `main.rs` to call `init` and then trigger a breakpoint exception:

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init(); // new

    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3(); // new

    // as before
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    loop {}
}
```

When we run it in QEMU now (using `cargo run`), we see the following:

![QEMU printing `EXCEPTION: BREAKPOINT` and the interrupt stack frame](qemu-breakpoint-exception.png)

It works! The CPU successfully invokes our breakpoint handler, which prints the message, and then returns back to the `_start` function, where the `It did not crash!` message is printed.

We see that the interrupt stack frame tells us the instruction and stack pointers at the time when the exception occurred. This information is very useful when debugging unexpected exceptions.

### Adding a Test

Let's create a test that ensures that the above continues to work. First, we update the `_start` function to also call `init`:

```rust
// in src/lib.rs

/// Entry point for `cargo test`
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init();      // new
    test_main();
    loop {}
}
```

Remember, this `_start` function is used when running `cargo test --lib`, since Rust tests the `lib.rs` completely independently of the `main.rs`. We need to call `init` here to set up an IDT before running the tests.

Now we can create a `test_breakpoint_exception` test:

```rust
// in src/interrupts.rs

#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}
```

The test invokes the `int3` function to trigger a breakpoint exception. By checking that the execution continues afterwards, we verify that our breakpoint handler is working correctly.

You can try this new test by running `cargo test` (all tests) or `cargo test --lib` (only tests of `lib.rs` and its modules). You should see the following in the output:

```
blog_os::interrupts::test_breakpoint_exception...	[ok]
```

## Too much Magic?
The `x86-interrupt` calling convention and the [`InterruptDescriptorTable`] type made the exception handling process relatively straightforward and painless. If this was too much magic for you and you like to learn all the gory details of exception handling, we got you covered: Our [“Handling Exceptions with Naked Functions”] series shows how to handle exceptions without the `x86-interrupt` calling convention and also creates its own IDT type. Historically, these posts were the main exception handling posts before the `x86-interrupt` calling convention and the `x86_64` crate existed. Note that these posts are based on the [first edition] of this blog and might be out of date.

[“Handling Exceptions with Naked Functions”]: @/edition-1/extra/naked-exceptions/_index.md
[`InterruptDescriptorTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html
[first edition]: @/edition-1/_index.md

## What's next?
We've successfully caught our first exception and returned from it! The next step is to ensure that we catch all exceptions, because an uncaught exception causes a fatal [triple fault], which leads to a system reset. The next post explains how we can avoid this by correctly catching [double faults].

[triple fault]: https://wiki.osdev.org/Triple_Fault
[double faults]: https://wiki.osdev.org/Double_Fault#Double_Fault
