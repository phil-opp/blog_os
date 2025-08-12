+++
title = "Async/Await"
weight = 12
path = "zh-CN/async-await"
date = 2020-03-27

[extra]
chapter = "Multitasking"

# Please update this when updating the translation

translation_based_on_commit = "67b3ac65dc735e0e109c2fb23ca18a536b84dc0d"

# GitHub usernames of the people that translated this post

translators = ["ic3w1ne"]

# GitHub usernames of the people that contributed to this translation

translation_contributors = []

+++

在这篇文章中，我们将探索 Rust 的 _协作式多任务处理_ 及 _async/await_ 特性。我们将深入探讨 Rust 中 async/await 的工作原理，包括 `Future` trait 的设计、状态机转换与 _pinning_ 。随后，我们通过创建一个异步键盘任务和基础执行器，为我们的内核添加对 async/await 的基本支持。

<!-- more -->

这个系列的 blog 在 [GitHub] 上公开开发。如果您遇到任何问题或有疑问，请在这里开一个 issue 来讨论。你也可以在[底部][at the bottom]留下评论。你可以在 [`post-12`][post branch] 找到这篇文章的完整源码。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-12

<!-- toc -->

## 多任务

大多数操作系统的基本功能之一就是[多任务处理][_multitasking_]，即同时执行多个任务的能力。例如，当你在查看这篇文章时，可能还打开了其他程序，比如文本编辑器或终端窗口。即便你只打开了一个浏览器窗口，也可能有各种后台任务在管理你的桌面窗口、检查更新或者索引文件。

[_multitasking_]: https://en.wikipedia.org/wiki/Computer_multitasking

虽然看起来所有任务都在同时运行，但实际上单个 CPU 核心一次只能执行单个任务。为了制造任务同时运行的假象，操作系统会在活动任务之间快速切换，使每个任务都能取得一点进展。由于计算机运行速度极快，我们大多数时候都不会注意到这些切换。

单核 CPU 一次只能执行一个任务，而多核 CPU 能够以真正并行的方式运行多个任务。例如，一个 8 核 CPU 可以同时运行 8 个任务。我们将在后续文章中介绍如何设置多核 CPU。本文中，为了简单起见，我们将重点讨论单核。（值得注意的是，所有的多核 CPU 都是从只有一个激活的核心开始的，所以我们现在可以将它们视为单核 CPU 来处理）。

多任务处理有两种形式：_协作式多任务处理_ 要求任务定期主动让出对 CPU 的控制权，以便其他任务能够运行。_抢占式多任务处理_ 利用操作系统在任意时间点强制暂停线程的能力实现切换线程的功能。在下文中，我们将更详细地探讨这两种多任务处理形式，并讨论它们各自的优势和缺点。

### 抢占式多任务处理

抢占式多任务处理的核心理念在于由操作系统决定何时进行任务切换。 为此，系统利用了每次中断时可重新获得 CPU 控制权这一机制。这使得系统能在有新输入时立即切换任务，例如当鼠标移动或网络数据包到达时。操作系统还可以通过配置硬件定时器，令其在指定时间后发送中断，从而精确控制每个任务允许运行的时间。

下图展示了硬件中断时的任务切换过程:

![](regain-control-on-interrupt.svg)

第一行中，CPU 正在执行程序 `A` 的任务 `A1` ，所有其他任务均处于暂停状态。在第二行，一个硬件中断到达 CPU 。如[硬件中断][_Hardware Interrupts_]文章所述，CPU 立即停止执行任务 `A1` 并跳转到中断描述符表(IDT)中定义的中断处理程序。通过这个中断处理程序，操作系统重新获得了 CPU 的控制权，这使得它能够切换到任务 `B1` 而非继续原任务  `A1` 。

[_Hardware Interrupts_]: @/edition-2/posts/07-hardware-interrupts/index.md

#### 保存状态

任务可能在任意时间点被中断，即使它们可能正处于某些计算过程中。为了稍后能够恢复他们，操作系统必须备份任务的完整状态，包括其[调用栈][call stack]和所有 CPU 寄存器的值。这一过程被称为[上下文切换][_context switch_]。

[call stack]: https://en.wikipedia.org/wiki/Call_stack
[_context switch_]: https://en.wikipedia.org/wiki/Context_switch

由于调用栈可能非常大，操作系统通常会为每个线程设置独立的调用栈，而非在每次任务切换时备份调用栈内容。这样一个拥有自己的栈的任务被称为一个 执行线程 [_thread of execution_] 或简称 线程_thread_。为每个任务使用独立的栈，在上下文切换时就只需保存寄存器内容（包括程序计数器和栈指针）。这种方法最大限度地减少了上下文切换的性能开销，这一点非常重要，因为上下文切换每秒可能发生多达100次。

[_thread of execution_]: https://en.wikipedia.org/wiki/Thread_(computing)

#### 讨论

抢占式多任务处理的主要优势在于操作系统能够完全控制任务允许执行的时间。这种方式可以确保每个任务公平地获得 CPU 时间份额，而无需依赖任务间的协作。这一特性在运行第三方任务或多个用户共享系统时尤为重要。

抢占式多任务处理的缺点在于每个任务都需要独立的栈空间。相较于共享栈，使用独立栈会导致每个任务占用更多内存，并且通常会限制任务的数量。另一个缺点是操作系统总是需要在每次任务切换时保存完整的 CPU 寄存器状态，即使任务只使用了寄存器的一小部分。

抢占式多任务处理和线程是操作系统的基本组成部分，因为它们使得运行不受信任的用户空间程序成为可能。我们将在后续文章中详细讨论这些概念。不过本文的重点将放在协作式多任务处理上，因为它对于我们的内核来说也足够实用。

### 协作式多任务处理

与在任意时间点强制暂停运行任务不同，协作式多任务处理让每个任务持续运行，直到它自愿放弃对 CPU 的控制权。这使得任务能够在合适的时机自行暂停，例如当它们需要等待 I/O 操作时。

协作式多任务处理常用于语言层面，比如以 [协程coroutines][coroutines] 或 [async/await] 的形式实现。其核心思想是由程序员或编译器在程序中插入 [yield][_yield_] 操作，这些操作会放弃 CPU 控制权并允许其他任务运行。例如，可以在一个复杂循环的每次迭代后插入yield。

[coroutines]: https://en.wikipedia.org/wiki/Coroutine
[async/await]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html
[_yield_]: https://en.wikipedia.org/wiki/Yield_(multithreading)

通常我们会将协作式多任务与 [异步操作asynchronous operations][asynchronous operations] 结合使用。不同于等待操作完成并在此期间阻止其他任务运行，异步操作会在操作未完成时返回 "未就绪"（"not ready"）状态。在这种情况下，等待中的任务可以执行 yield 操作，让其他任务运行。

[asynchronous operations]: https://en.wikipedia.org/wiki/Asynchronous_I/O

#### 保存状态

由于任务自行定义暂停点，它们不需要操作系统来保存其状态。相反，它们可以在暂停自己前精确保存恢复所需的状态，这通常会带来更好的性能表现。例如，一个刚刚完成复杂计算的任务可能只需要保存最终结果，而不再需要中间过程。

协作式多任务处理的编程语言级实现甚至能够在暂停前保存调用栈的必要部分。例如，Rust 的 async/await 实现会将所有仍被需要的局部变量存储在一个自动生成的结构体中（如后文所示）通过在暂停前保存调用栈的相关部分，所有任务可以共享单个调用栈，这使得每个任务的内存消耗大幅降低。从而实现创建任意数量的协作式任务并且不会耗尽内存。

#### 讨论

协作式多任务处理的缺点在于，一个不配合的任务可能会长时间占用处理器资源。因此，恶意或有缺陷的任务可能会阻止其他任务运行，并且会拖慢甚至阻塞整个系统。因此，协作式多任务处理应仅在所有任务都会协作的情况下使用。让操作系统依赖于任意用户级程序的协作并不是一个好主意。

然而，协作式多任务处理在性能和内存方面的显著优势，使其成为适合在程序内部使用的好方法，特别是与异步操作结合使用。操作系统内核作为与异步硬件交互的性能关键程序，采用协作式多任务处理似乎是一种实现并发的理想方式。

## Rust 中的 Async/Await

Rust 语言为协作式多任务处理提供了一流的支持，其实现形式是 async/await。在我们探讨 async/await 的概念及其工作原理之前，需要先理解 Rust 中 _futures_ 和异步编程的运作机制。

### Futures

一个 _future_ 代表一个可能尚未就绪的值。例如，这个值可以是一个由其他任务计算得出的整数，或从网络下载的文件。futures 使得程序可以继续执行，直到需要该值时再处理，而非在原地等待直到它可用。

#### 示例

futures 的概念可以通过一个小例子说明： 

![Sequence diagram: main calls `read_file` and is blocked until it returns; then it calls `foo()` and is also blocked until it returns. The same process is repeated, but this time `async_read_file` is called, which directly returns a future; then `foo()` is called again, which now runs concurrently with the file load. The file is available before `foo()` returns.](async-example.svg)

该序列图展示了一个 `main` 函数，它从文件系统中读取文件，然后调用 `foo` 函数。这个过程会重复两次：一次使用同步的 `read_file` 调用，另一次使用异步的 `async_read_file` 调用。

使用同步调用时， `main` 函数需要等待文件从文件系统中加载完成后才能调用 `foo` 函数。

通过异步的 `async_read_file` 调用，文件系统会直接返回一个 future 并在后台异步加载文件。这使得 `main` 函数能够更早地调用 `foo` ，然后 `foo` 会与文件加载并行运行。在这个例子中，文件加载甚至在 `foo` 返回前就完成了，因此 `main` 在 `foo` 返回后无需等待就能直接处理文件。

#### Rust 中的 Future

在 Rust 中，future 由 [`Future`] trait 表示，其定义如下：

[`Future`]: https://doc.rust-lang.org/nightly/core/future/trait.Future.html

```rust
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
}
```

[关联类型][associated type] `Output` 用于指定异步值的类型。例如, 上图中的 `async_read_file` 函数将返回一个 `Future` 实例，其 `Output` 被设置为 `File` 。

[associated type]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#specifying-placeholder-types-in-trait-definitions-with-associated-types

[`poll`] 方法可用于检查值是否已就绪。它返回一个 [`Poll`] 枚举，其定义如下：

[`poll`]: https://doc.rust-lang.org/nightly/core/future/trait.Future.html#tymethod.poll
[`Poll`]: https://doc.rust-lang.org/nightly/core/task/enum.Poll.html

```rust
pub enum Poll<T> {
    Ready(T),
    Pending,
}
```

当值已可用时（例如文件已从磁盘完全读取），它会被包装后返回 `Ready` 变体。否则返回 `Pending` 变体，向调用者表明该值尚不可用。

`poll` 方法接收两个参数： `self: Pin<&mut Self>` and `cx: &mut Context` 。其中，前者的行为类似于普通的 `&mut self` 引用，不同之处在于 `Self` 值是被 [_pinned_] 在其内存位置。如果不了解 async/await 的工作原理，那么理解 `Pin` 的原理和必要性会变得很困难。因此我们会在后文中详细解释。

[_pinned_]: https://doc.rust-lang.org/nightly/core/pin/index.html

参数 `cx: &mut Context` 的作用是传递一个[`Waker`] 实例给异步任务，例如文件系统加载。这个 `Waker` 允许异步任务发出信号来表明它已全部或者部分完成，例如文件已从磁盘加载完成。由于主任务知道在 `Future` 就绪时它会收到通知，因此它不需要反复调用 `poll` 方法。我们将在本文后面实现自己的 waker 类型时更详细地解释这个过程。

[`Waker`]: https://doc.rust-lang.org/nightly/core/task/struct.Waker.html

### 使用 Future 进行开发

我们现在已经了解了 Future 的定义，并理解了 `poll` 方法背后的基本理念。然而，我们仍然不知道如何高效地使用 Future。问题在于 Future 表示异步任务的结果，而这些结果可能还不可用。但在实际应用中，我们经常需要直接使用这些值进行后续计算。那么问题来了，当需要时，应该如何高效地获取 Future 的值？

#### 等待 Future 就绪

一种可能的解决方案是等待 Future 变为就绪状态。具体实现可能如下所示：

```rust
let future = async_read_file("foo.txt");
let file_content = loop {
    match future.poll(…) {
        Poll::Ready(value) => break value,
        Poll::Pending => {}, // 什么都不做
    }
}
```

在这里，我们通过在循环中反复调用 `poll` 来 _主动_ 等待 future 。`poll` 的参数在此处并不重要，因此我们将其省略。虽然这种解决方案可行，但效率非常低下，因为它会让 CPU 持续忙碌直到值变得可用。

更高效的方法可能是 _阻塞_ 当前线程，直到 future 值变得可用。当然，这只有在拥有线程的情况下才可能实现，因此该解决方案不适用于我们的内核，至少目前还不适用。即使在支持阻塞的系统上，这种方式通常也不被推荐，因为它会将异步任务再次转变为同步任务，从而抑制了并行任务潜在的性能优势。

#### Future 组合器

等待的替代方案是使用 future 组合器（future combinators）。Future 组合器是像 `map` 这样的方法，它允许将多个 future 链式组合在一起，类似于 [`Iterator`] trait的方法。 这些组合器不会等待 future 完成，而是返回一个新的 future，该 future 会在 `poll` 时应用映射操作。

[`Iterator`]: https://doc.rust-lang.org/stable/core/iter/trait.Iterator.html

例如，一个简单的 `string_len` 组合器，用于将 `Future<Output = String>` 转换为 `Future<Output = usize>` 可以这样实现：

```rust
struct StringLen<F> {
    inner_future: F,
}

impl<F> Future for StringLen<F> where F: Future<Output = String> {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        match self.inner_future.poll(cx) {
            Poll::Ready(s) => Poll::Ready(s.len()),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn string_len(string: impl Future<Output = String>)
    -> impl Future<Output = usize>
{
    StringLen {
        inner_future: string,
    }
}

// Usage
fn file_len() -> impl Future<Output = usize> {
    let file_content_future = async_read_file("foo.txt");
    string_len(file_content_future)
}
```

这段代码不完全能工作，因为它没有处理 [_pinning_] 问题，但作为示例已经足够。其基本思路是，`string_len` 函数将给定的 `Future` 实例包装到一个新的 `StringLen` 结构体，这个结构体同样也实现了 `Future` 。当被包装的 future 被轮询时，它会轮询内部 future。如果值还不可用，包装后的 future 也会返回 `Poll::Pending`。如果值可用，就从 `Poll::Ready` 变体中把字符串提取出来并计算其长度。 随后，它会被重新包装进 `Poll::Ready` 并返回。

[_pinning_]: https://doc.rust-lang.org/stable/core/pin/index.html

通过这个 `string_len` 函数，我们无需等待就能计算异步字符串的长度。由于该函数再次返回一个 `Future` ，调用者无法直接使用返回值，而是需要再次使用组合器函数。这样一来，整个调用链就变成了异步的，我们可以在某些节点（例如 main 函数中）高效地同时等待多个 future。

由于手动编写组合器函数较为困难，它们通常由库提供。虽然 Rust 标准库本身尚未提供组合器函数，但半官方的（且兼容 no_std 的）[`futures`] crate 提供了这些功能。其 [`FutureExt`] trait 提供了诸如 [`map`] 或 [`then`] 等高级组合器方法，可用于通过任意闭包来操作结果。

[`futures`]: https://docs.rs/futures/0.3.4/futures/
[`FutureExt`]: https://docs.rs/futures/0.3.4/futures/future/trait.FutureExt.html
[`map`]: https://docs.rs/futures/0.3.4/futures/future/trait.FutureExt.html#method.map
[`then`]: https://docs.rs/futures/0.3.4/futures/future/trait.FutureExt.html#method.then

##### 优势

future 组合器的最大优势在于它们能保持操作的异步性。在与异步 I/O 接口结合使用时，这种方法可以实现极高的性能。 future 组合器以普通结构体配合 trait 的方式实现，使得编译器能够对其进行深度优化。更多详情请参阅 [_Rust中的零成本 futures_][_Zero-cost futures in Rust_] 文章，它宣布了 futures 被加入 Rust 生态系统的消息。

[_Zero-cost futures in Rust_]: https://aturon.github.io/blog/2016/08/11/futures/

##### 缺点 {#drawbacks}

虽然 future 组合器能够编写出非常高效的代码，但在某些情况下，由于类型系统和基于闭包的接口，它们可能变得难以使用。例如，考虑如下代码：

```rust
fn example(min_len: usize) -> impl Future<Output = String> {
    async_read_file("foo.txt").then(move |content| {
        if content.len() < min_len {
            Either::Left(async_read_file("bar.txt").map(|s| content + &s))
        } else {
            Either::Right(future::ready(content))
        }
    })
}
```

([Try it on the playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=91fc09024eecb2448a85a7ef6a97b8d8))

这里我们读取 `foo.txt` 文件，然后使用 `then` 组合器根据文件内容链接第二个future。如果内容长度小于给定的 `min_len`，我们会读取另一个文件 `bar.txt` 并将其追加到 `content` ，否则仅返回 `foo.txt` 的内容。

我们需要对传递给 `then` 的闭包使用 [move 关键字][`move` keyword]，否则 `min_len` 中会出现生命周期错误。使用 [`Either`] 包装器的原因是 `if` 和 `else` 代码块必须始终保持相同的类型。由于我们在代码块中返回了不同的 future 类型，必须使用包装器类型将它们统一为单一类型。[`ready`] 函数将一个值包装成立刻可用的 future。这里需要该函数是因为 `Either` 包装器要求被包装的值必须实现 Future。

[`move` keyword]: https://doc.rust-lang.org/std/keyword.move.html
[`Either`]: https://docs.rs/futures/0.3.4/futures/future/enum.Either.html
[`ready`]: https://docs.rs/futures/0.3.4/futures/future/fn.ready.html

可以想象，对于大型项目来说，这很快就会导致代码变得复杂。特别是涉及借用和不同的生命周期时，情况会变得更加复杂。正因如此，大量工作被投入到为 Rust 添加 async/await 支持中，来让异步代码编写起来更简单。

### Async/Await 异步/等待模式

async/await 的设计理念是让程序员编写 _看似_ 普通的同步代码，但由编译器转换为异步代码。它基于 `async` 和 `await` 两个关键字运作。`async` 关键字可用于函数签名中来将一个同步函数转换为返回 future 的异步函数：

```rust
async fn foo() -> u32 {
    0
}

// 上述代码大致被编译器转换成
fn foo() -> impl Future<Output = u32> {
    future::ready(0)
}
```

只有这个关键字本身看起来不太有用。然而，在 `async` 函数内部，`await` 关键字可用于获取一个 future 的异步值：

```rust
async fn example(min_len: usize) -> String {
    let content = async_read_file("foo.txt").await;
    if content.len() < min_len {
        content + &async_read_file("bar.txt").await
    } else {
        content
    }
}
```

([Try it on the playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=d93c28509a1c67661f31ff820281d434))

此函数直接转换自[上文](#drawbacks)中使用组合函数的 `example` 函数。通过使用 `.await` 运算符，我们无需任何闭包或者 `Either` 类型就可以直接获取 future 的值。于是我们就可以像写普通的同步代码一样编写代码，只不过 _这实际上是异步代码_。

#### 状态机转换

在底层，编译器将 `async` 函数体转换为一个 [状态机][_state machine_] ，每次调用 `.await` 都代表一个不同的状态。对于上述 `example` 函数，编译器会创建一个包含以下四种状态的状态机：

[_state machine_]: https://en.wikipedia.org/wiki/Finite-state_machine

![Four states: start, waiting on foo.txt, waiting on bar.txt, end](async-state-machine-states.svg)

每个状态代表函数执行过程中的不同暂停点。_"Start"_ 和 _"End"_ 状态分别表示函数执行的开端和终止。_"Waiting on foo.txt"_ 状态表示该函数当前正在等待第一个 `async_read_file` 的结果。类似的，_"Waiting on bar.txt"_ 状态表示该函数正在等待第二个 `async_read_file` 的结果。

状态机通过将每次 `poll` 调用作为可能的状态转换来实现 `Future` trait：

![Four states and their transitions: start, waiting on foo.txt, waiting on bar.txt, end](async-state-machine-basic.svg)

该图表使用箭头表示状态转换，菱形表示条件分支。例如，如果 `foo.txt` 文件尚未就绪，则会选择标记为 _"no"_  的分支，到达 _"Waiting on foo.txt"_ 状态。否则，将执行 _"yes"_ 分支。那个小的无标注的红色菱形代表 `example` 函数中 `if content.len() < 100` 分支。

我们看到第一次 `poll` 调用启动了该函数并让其运行，直到遇到一个尚未可用的 future。如果路径上所有 future 都已就绪，函数可以一直运行到 _"End"_ 状态，此时它会返回包裹在 `Poll::Ready` 中的结果。否则，状态机将进入等待状态并且返回 `Poll::Pending`。在下一次 `poll` 调用时，状态机将从上次的等待状态中恢复并尝试之前的操作。

#### 保存状态

为了能够从上一个等待状态继续执行，状态机必须在内部跟踪当前状态。此外，它还必须保存所有在下一次 `poll` 调用时继续执行所需的变量。这正是编译器可以大显身手的地方：因为它知道哪些变量在何时被使用，当需要时，它能自动生成包含这些确切变量的结构体。

例如，编译器会为上述 `example` 函数生成类似以下的结构体：

```rust
// 这里再写一遍 `example` 函数，方便阅读
async fn example(min_len: usize) -> String {
    let content = async_read_file("foo.txt").await;
    if content.len() < min_len {
        content + &async_read_file("bar.txt").await
    } else {
        content
    }
}

// 由编译器生成的状态结构体：

struct StartState {
    min_len: usize,
}

struct WaitingOnFooTxtState {
    min_len: usize,
    foo_txt_future: impl Future<Output = String>,
}

struct WaitingOnBarTxtState {
    content: String,
    bar_txt_future: impl Future<Output = String>,
}

struct EndState {}
```

在 "start" 和 _"Waiting on foo.txt"_ 状态下，需要存储 `min_len` 参数以供稍后与 `content.len()` 进行比较。 _"Waiting on foo.txt"_ 状态额外存储了一个 `foo_txt_future` ，它代表 `async_read_file` 调用返回的 future。这个 future 在状态机继续运行时需要再次被轮询，因此需要保存它。

_"Waiting on bar.txt"_ 状态包含用于后续 `bar.txt` 准备就绪时字符串拼接的 `content` 变量。它还存储了一个 `bar_txt_future` ，用于表示正在加载中的 `bar.txt` 。

该结构体不再包含 `min_len` 变量，因为在 `content.len()` 比较之后就不再需要它。在 _"end"_ 状态，不会存储任何变量，因为函数已经运行完成。

请注意，这只是编译器可能生成的代码示例。结构体名称和字段布局的实现细节可能会有所不同。

#### 完整状态机类型

虽然编译器生成的具体代码属于实现细节，但想象一下为 `example` 函数生成的状态机 _可能_ 是什么样子有助于理解。我们已经定义了表示不同状态并包含所需变量的结构体。为了基于它们创建一个状态机，我们可以将它们组合成一个 [`enum`]：

[`enum`]: https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html

```rust
enum ExampleStateMachine {
    Start(StartState),
    WaitingOnFooTxt(WaitingOnFooTxtState),
    WaitingOnBarTxt(WaitingOnBarTxtState),
    End(EndState),
}
```

我们为每个状态定义独立的枚举变体，并为每个变体添加对应的状态结构体作为字段。为实现状态转换，编译器会生成一个基于 `example` 函数的 `Future` trait：

```rust
impl Future for ExampleStateMachine {
    type Output = String; // `example` 的返回类型

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        loop {
            match self { // TODO: 处理 pinning
                ExampleStateMachine::Start(state) => {…}
                ExampleStateMachine::WaitingOnFooTxt(state) => {…}
                ExampleStateMachine::WaitingOnBarTxt(state) => {…}
                ExampleStateMachine::End(state) => {…}
            }
        }
    }
}
```

该 future 的 `Output` 类型是 `String` ，因为它是 `example` 函数的返回类型。为了实现 `poll` 函数，我们在 `loop` 内对当前状态使用 `match` 语句。其核心思想是只要可能就切换到下一个状态，并在无法继续时使用显式的 `return Poll::Pending` 。

为简单起见，我们仅展示简化代码，不处理 [_pinning_]、所有权、生命周期等问题。因此，当前及后续代码应视为伪代码，不可直接使用。当然，编译器实际生成的代码会正确处理所有情况，尽管实现方式可能有所不同。

为保持代码片段简洁，我们将分别展示每个 `match` 分支的代码。让我们从 `Start` 状态开始：

```rust
ExampleStateMachine::Start(state) => {
    // 来自 `example` 函数体
    let foo_txt_future = async_read_file("foo.txt");
    // `.await` 运算符
    let state = WaitingOnFooTxtState {
        min_len: state.min_len,
        foo_txt_future,
    };
    *self = ExampleStateMachine::WaitingOnFooTxt(state);
}
```

状态机在函数刚开始时为 `Start` 状态。在这种情况下，我们会执行 `example` 函数体内的所有代码，直到遇到第一个 `.await` 为止。为了处理 `.await` 运算符，我们会将 `self` 状态机的状态设置为 `WaitingOnFooTxt`，其中包括了 `WaitingOnFooTxtState` 结构体的构建。

由于 `match self {…}` 语句是在循环中执行的，执行流程会跳转到之后的 `WaitingOnFooTxt` 分支：

```rust
ExampleStateMachine::WaitingOnFooTxt(state) => {
    match state.foo_txt_future.poll(cx) {
        Poll::Pending => return Poll::Pending,
        Poll::Ready(content) => {
            // 来自 `example` 函数体
            if content.len() < state.min_len {
                let bar_txt_future = async_read_file("bar.txt");
                // `.await` 运算符
                let state = WaitingOnBarTxtState {
                    content,
                    bar_txt_future,
                };
                *self = ExampleStateMachine::WaitingOnBarTxt(state);
            } else {
                *self = ExampleStateMachine::End(EndState);
                return Poll::Ready(content);
            }
        }
    }
}
```

在这个 `match` 分支中，我们首先调用 `foo_txt_future` 的 `poll` 函数。如果它尚未就绪，我们直接退出循环并返回 `Poll::Pending` 。由于此时 `self` 仍处于 `WaitingOnFooTxt` 状态，状态机的下一次 `poll` 调用将进入相同的 `match` 分支，并重新尝试轮询 `foo_txt_future`。

当 `foo_txt_future` 就绪时，我们将结果赋值给 `content` 变量并继续执行 `example` 函数的代码：如果 `content.len()` 小于状态结构体中保存的 `min_len` 则异步读取 `bar.txt` 文件。我们再次将 `.await` 操作转换为状态变更，这次变更为 `WaitingOnBarTxt` 状态。由于我们是在循环中执行 `match` 操作，执行流程会直接跳转到新状态对应的 `match` 分支继续处理。其中会对 `bar_txt_future` 进行轮询。

若进入 `else` 分支，则不会发生进一步的 `.await` 操作。我们到达函数末尾并返回包裹在 `Poll::Ready` 中的 `content` 。同时将当前状态更改为 `End` 状态。

 `WaitingOnBarTxt` 状态的代码如下所示：

```rust
ExampleStateMachine::WaitingOnBarTxt(state) => {
    match state.bar_txt_future.poll(cx) {
        Poll::Pending => return Poll::Pending,
        Poll::Ready(bar_txt) => {
            *self = ExampleStateMachine::End(EndState);
            // 来自 `example` 函数体
            return Poll::Ready(state.content + &bar_txt);
        }
    }
}
```

与 `WaitingOnFooTxt` 状态类似，我们首先轮询 `bar_txt_future` 。如果它仍然如果处于 pending 状态，我们退出循环并返回 `Poll::Pending` 。否则，我们可以执行 `example` 函数最后的操作：拼接 `content` 以及 future 的返回值。我们将状态机更新为 `End` 状态，然后返回包装在 `Poll::Ready` 中的结果。

最终，`End` 状态的代码如下所示：

```rust
ExampleStateMachine::End(_) => {
    panic!("poll called after Poll::Ready was returned");
}
```

Futures 在返回 `Poll::Ready` 后不应再次轮询，所以在处于 `End` 状态时发生 `poll` 调用，则直接 panic。

我们现在已经了解了编译器生成的状态机及其对 Future 的实现 _可能_ 的样子。实际上，编译器是以另一种方式生成代码的。（如果你感兴趣的话：这个实现当前基于 [协程][_coroutines_]，但这仅仅是一种实现细节。）

[_coroutines_]: https://doc.rust-lang.org/stable/unstable-book/language-features/coroutines.html

拼图的最后一块是为 `example` 函数本身生成的代码。记住，函数签名是这样定义的：

```rust
async fn example(min_len: usize) -> String
```

由于完整函数体现在已由状态机实现，唯一需要该函数完成的是初始化状态机并返回它。生成的代码可能如下所示：

```rust
fn example(min_len: usize) -> ExampleStateMachine {
    ExampleStateMachine::Start(StartState {
        min_len,
    })
}
```

该函数不再具有 `async` 修饰符，因为它现在显式返回一个 实现了 `Future` trait 的 `ExampleStateMachine` 类型。正如预期的那样，这个状态机构建出来处于 `Start` 状态，并使用 `min_len` 参数初始化对应的状态结构体。

请注意，此函数不会启动状态机的执行。这是 Rust 中 future 的一个基本设计决策：在首次被轮询之前，它们不会执行任何操作。

### Pinning

在本文中我们已经多次提到了 _固定_ (pinning)，现在终于可以深入探讨什么是固定以及为什么需要它。

#### 自引用结构体

如上所述，状态机转换会把每个暂停点的局部变量存储在结构体中。对于 `example` 函数这样的小例子，这很直接且不会导致任何问题。但当变量相互引用时，情况就变得复杂了。例如，考虑以下函数：

```rust
async fn pin_example() -> i32 {
    let array = [1, 2, 3];
    let element = &array[2];
    async_write_file("foo.txt", element.to_string()).await;
    *element
}
```

该函数创建了一个包含元素 `1`, `2`, 和 `3` 的小型 `array`。然后它创建对最后一个数组元素的引用，并将其存储在 `element` 变量中。接着，它异步地将数字转换为字符串并写入 `foo.txt` 文件。最后，它返回由 `element` 引用的数字。

由于该函数使用了单个 `await` 操作，生成的状态机包含三个状态：开始、结束和"等待写入"。该函数不接受参数，因此开始状态的结构体为空。如前所述，结束状态的结构体为空，因为函数在此处已执行完毕。"等待写入"状态的结构体则更为有趣：

```rust
struct WaitingOnWriteState {
    array: [1, 2, 3],
    element: 0x1001c, // 最后一个数组元素的地址
}
```

我们需要同时 `array` 数组和 `element` 变量，因为 `element` 对于返回值是必需的，而 `array` 被 `element` 引用。由于 `element` 是一个引用，它存储了一个 _指针_ （即内存地址）指向被引用的元素。这里我们以 `0x1001c` 为例。实际上，它就是 `array` 字段最后一个元素的地址，因此这取决于结构体在内存中的位置。具有这种内部指针的结构体被称为 _自引用结构体_ （_self-referential_ ），因为它们通过其中某个字段引用了自身。

#### 自引用结构体的问题

我们自引用结构体的内部指针引出了一个根本性问题，当我们查看其内存布局时，这一点变得显而易见：

![array at 0x10014 with fields 1, 2, and 3; element at address 0x10020, pointing to the last array element at 0x1001c](self-referential-struct.svg)

`array` 字段起始于地址 0x10014，`element` 元素字段位于地址 0x10020。它指向地址 0x1001c，因为最后一个数组元素位于此地址。此时一切正常。然而，当我们把这个结构体移动到不同的内存地址时就会出现问题：

![array at 0x10024 with fields 1, 2, and 3; element at address 0x10030, still pointing to 0x1001c, even though the last array element now lives at 0x1002c](self-referential-struct-moved.svg)

我们将结构体稍微移动了一下，现在它从地址 `0x10024` 开始。这种情况可能发生在，例如当我们把结构体作为函数参数传递或将其赋值给不同的栈变量时。问题在于，即使最后一个 `array` 元素已经移动，`element` 字段仍然指向地址 `0x1001c` ，然而实际上该元素现在位于地址 `0x1002c`。因此这个指针变成悬垂指针，导致在下一次 `poll` 调用时出现未定义行为。

#### 可能的解决方案

解决悬垂指针问题有三种基本方法：

* **移动时更新指针：**其理念是每次结构体在内存中移动时都更新内部指针，从而保持有效。遗憾的是，这种方法需要对 Rust 进行大量修改，这可能导致巨大的性能损失。原因是需要某种运行时机制来跟踪所有结构体的字段类型并在每次移动操作时检查是否需要更新指针。

* **存储偏移量而非自引用：**为避免更新指针，编译器可以尝试将自引用存储为相对于结构体起始位置的偏移量。例如，上述 `WaitingOnWriteState` 结构体中的 `element` 字段可以存储为一个值为 8 的 `element_offset` 字段，因为引用点指向的数组元素在结构体起始位置之后后 8 字节处。由于偏移量结构体被移动时保持不变，没有字段需要更新。这种方法的问题在于需要编译器检测所有自引用。这在编译时无法实现，因为引用的值可能取决于用户输入，因此就又需要一个运行时系统来分析引用并正确创建状态结构体。这不仅会导致运行时开销，还会影响某些编译器优化，从而再次造成较大的性能损失。

* **禁止移动结构体：**如上所述，只有在内存中移动结构体时才会出现悬垂指针。通过完全禁止对自引用结构体的移动操作就可以避免这个问题。这种方法的最大优势在于它能够在类型系统层面实现，无需额外的运行时开销。缺点是它将处理可能移动的自引用结构体的责任交给了程序员。

Rust 选择了第三种解决方案，这源于其提供 _零成本抽象_ 的原则，即抽象不应带来额外的运行时开销。_pinning_ API 正是为此目的而在 [RFC 2349](https://github.com/rust-lang/rfcs/blob/master/text/2349-pin.md) 中提出的。接下来，我们将简要概述这个API，并解释它如何与 async/await 和 futures 协同工作。

#### 堆上的值

第一个观察结果是，[堆分配的][heap-allocated] 值在大多数情况下已经拥有固定的内存地址。它们通过调用 `allocate` 来创建，由一个指针类型比如 `Box<T>` 来引用。虽然可以移动指针类型，但指针所指向的堆值在内存中的地址保持不变，除非调用 `deallocate` 将其释放。

[heap-allocated]: @/edition-2/posts/10-heap-allocation/index.md

使用堆分配，我们可以尝试创建一个自引用结构体：

```rust
fn main() {
    let mut heap_value = Box::new(SelfReferential {
        self_ptr: 0 as *const _,
    });
    let ptr = &*heap_value as *const SelfReferential;
    heap_value.self_ptr = ptr;
    println!("heap value at: {:p}", heap_value);
    println!("internal reference: {:p}", heap_value.self_ptr);
}

struct SelfReferential {
    self_ptr: *const Self,
}
```

([Try it on the playground][playground-self-ref])

[playground-self-ref]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=ce1aff3a37fcc1c8188eeaf0f39c97e8

我们创建了一个名为 `SelfReferential` 的简单结构体，它包含一个单独的指针字段。首先，我们使用空指针初始化此结构体，然后通过 `Box::new` 在堆上分配内存存储它。接下来尝试确定堆分配结构体的内存地址并将其存储在 `ptr` 变量中。最后，通过将 `ptr` 变量赋值给 `self_ptr` 字段使结构体形成自引用。

当我们在 playground 上执行这段代码时，可以看到堆值的地址与其内部指针是相等的，这意味着 `self_ptr` 字段是一个有效的自引用。由于 `heap_value` 变量仅是一个指针，移动它（例如传递给函数）并不会改变结构体自身的地址，因此即使指针被移动，`self_ptr` 仍保持有效。

然而，仍有一种方式可以破坏这个示例：我们可以从 `Box<T>` 移出或替换其内容:

```rust
let stack_value = mem::replace(&mut *heap_value, SelfReferential {
    self_ptr: 0 as *const _,
});
println!("value at: {:p}", &stack_value);
println!("internal reference: {:p}", stack_value.self_ptr);
```

([Try it on the playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=e160ee8a64cba4cebc1c0473dcecb7c8))

这里我们使用 [`mem::replace`] 函数将堆分配的值替换为一个新的结构体实例。 这样我们就可以将原始的 `heap_value` 移动到栈上，而结构体的 `self_ptr` 字段此时变成了一个悬垂指针，仍然指向旧的堆地址。当您尝试在 playground 上运行示例时，会看到打印的 _"value at:"_ and _"internal reference:"_  行确实显示了不同的指针。因此仅对值进行堆分配并不足以确保自引用安全。

[`mem::replace`]: https://doc.rust-lang.org/nightly/core/mem/fn.replace.html

导致上述破坏的根本问题是 `Box<T>` 允许我们获取堆分配值的 `&mut T` 引用。这个 `&mut T` 引用导致可以使用诸如 [`mem::replace`] 或者 [`mem::swap`] 这样的方法使堆分配的值失效。为解决此问题，我们必须防止创建指向自引用结构体的 `&mut` 引用。

[`mem::swap`]: https://doc.rust-lang.org/nightly/core/mem/fn.swap.html

#### `Pin<Box<T>>` 与 `Unpin`

固定 pinning API 通过 [`Pin`] 包装类型以及 [`Unpin`] trait 提供了解决 `&mut T` 问题的方案。这些类型背后的理念是，将所有 `Pin` 中能获取包装值的 `&mut` 引用的方法（例如 [`get_mut`][pin-get-mut] 或 [`deref_mut`][pin-deref-mut]) 都限制在 `Unpin` trait 上使用。`Unpin` trait 是一个 [_auto trait_] ，会自动为所有类型实现，除了那些明确选择不实现的类型。通过让自引用结构体不实现 `Unpin`，使得无法（安全地）从 `Pin<Box<T>>` 类型中获取它们的 `&mut T` ，从而保证它们内部的自引用保持有效。

[`Pin`]: https://doc.rust-lang.org/stable/core/pin/struct.Pin.html
[`Unpin`]: https://doc.rust-lang.org/nightly/std/marker/trait.Unpin.html
[pin-get-mut]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.get_mut
[pin-deref-mut]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.deref_mut
[_auto trait_]: https://doc.rust-lang.org/reference/special-types-and-traits.html#auto-traits

举个例子，让我们更新上面的 `SelfReferential` 类型来让其不实现 `Unpin`：

```rust
use core::marker::PhantomPinned;

struct SelfReferential {
    self_ptr: *const Self,
    _pin: PhantomPinned,
}
```

我们通过添加第二个类型为 [`PhantomPinned`] 的 `_pin` 字段来选择退出。该类型是零大小的标记类型，仅用于不实现 `Unpin` trait。根据 [_auto trait_] 的工作原理，当某个字段不是 `Unpin` 时，就足以使整个结构体不实现 `Unpin` trait。

[`PhantomPinned`]: https://doc.rust-lang.org/nightly/core/marker/struct.PhantomPinned.html

第二步是将示例中的 `Box<SelfReferential>` 类型更改为 `Pin<Box<SelfReferential>>` 类型。最简单的方法是使用 [`Box::pin`] 函数而非 [`Box::new`] 来创建堆分配的值：

[`Box::pin`]: https://doc.rust-lang.org/nightly/alloc/boxed/struct.Box.html#method.pin
[`Box::new`]: https://doc.rust-lang.org/nightly/alloc/boxed/struct.Box.html#method.new

```rust
let mut heap_value = Box::pin(SelfReferential {
    self_ptr: 0 as *const _,
    _pin: PhantomPinned,
});
```

除了将 `Box::new` 改为 `Box::pin` 外，我们还需要在结构体初始化器中添加新的 `_pin` 字段。由于 `PhantomPinned` 是零大小类型，我们只要有其类型名称即可完成初始化。

当我们现在[尝试运行调整后的示例](https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=961b0db194bbe851ff4d0ed08d3bd98a)时，会发现它不再有效：

```
error[E0594]: cannot assign to data in a dereference of `std::pin::Pin<std::boxed::Box<SelfReferential>>`
  --> src/main.rs:10:5
   |
10 |     heap_value.self_ptr = ptr;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^ cannot assign
   |
   = help: trait `DerefMut` is required to modify through a dereference, but it is not implemented for `std::pin::Pin<std::boxed::Box<SelfReferential>>`

error[E0596]: cannot borrow data in a dereference of `std::pin::Pin<std::boxed::Box<SelfReferential>>` as mutable
  --> src/main.rs:16:36
   |
16 |     let stack_value = mem::replace(&mut *heap_value, SelfReferential {
   |                                    ^^^^^^^^^^^^^^^^ cannot borrow as mutable
   |
   = help: trait `DerefMut` is required to modify through a dereference, but it is not implemented for `std::pin::Pin<std::boxed::Box<SelfReferential>>`
```

这两个错误的发生是因为 `Pin<Box<SelfReferential>>` 类型不再实现 `DerefMut` trait。这正是我们想要的，因为 `DerefMut` trait 会返回一个 `&mut` 引用，而这正是我们想要避免的。这种情况之所以发生，仅仅是因为我们同时选择了不实现 `Unpin` 并将 `Box::new` 改为 `Box::pin`。

现在的问题是，编译器不仅阻止了第16行中的类型移动，还禁止在第10行初始化 `self_ptr` 字段。这是因为编译器无法区分 `&mut` 引用的有效和无效使用。要使初始化正常工作，我们必须使用不安全的 [`get_unchecked_mut`] 方法：

[`get_unchecked_mut`]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.get_unchecked_mut

```rust
// 安全，因为修改一个字段不会移动整个结构体
unsafe {
    let mut_ref = Pin::as_mut(&mut heap_value);
    Pin::get_unchecked_mut(mut_ref).self_ptr = ptr;
}
```

([Try it on the playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=b9ebbb11429d9d79b3f9fffe819e2018))

`get_unchecked_mut` 函数工作于 `Pin<&mut T>` 之上，而非 `Pin<Box<T>>` ，因此我们必须使用 [`Pin::as_mut`] 转换值。然后我们可以通过 `get_unchecked_mut` 返回的 `&mut` 引用来设置 `self_ptr` 字段。

[`Pin::as_mut`]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.as_mut

现在剩下的唯一错误就是 `mem::replace` 上的预期错误了。记住，这个操作试图将堆分配的值移动到栈上，这会破坏存储在 `self_ptr` 字段的自引用。通过选择不实现 `Unpin` 并采用 `Pin<Box<T>>` ，我们可以在编译器阻止此类操作并安全地处理自引用结构体。正如我们所看到的，编译器（目前）还无法证明创建自引用是安全的，因此我们需要使用 unsafe 代码块自行验证其正确性。

#### 栈上的Pinning与 `Pin<&mut T>`

在上一节中，我们学习了如何使用 `Pin<Box<T>>` 安全地创建堆分配的自引用值。虽然这种方法效果良好且相对安全（除了不安全的构造过程外），但所需的堆分配会带来性能开销。由于 Rust 致力于尽可能实现零成本抽象，pinning API 也允许创建指向栈上值的 `Pin<&mut T>` 实例。

与拥有被包装值的所有权的 `Pin<Box<T>>` 实例不同， `Pin<&mut T>` 实例仅临时借用所包装的值。这使得情况更加复杂，因为它要求程序员自行提供额外的保证。最重要的是，一个 `Pin<&mut T>` 必须在被引用的 `T` 的整个生命周期内保持固定，这一点对于基于栈的变量来说难以验证。为此，存在像 [`pin-utils`] 这样的 crate，但我仍然不建议固定到栈上，除非你非常清楚自己在做什么。

[`pin-utils`]: https://docs.rs/pin-utils/0.1.0-alpha.4/pin_utils/

如需进一步阅读，请查阅 [`pin` 模块][`pin` module] 的文档以及 [`Pin::new_unchecked`] 方法。

[`pin` module]: https://doc.rust-lang.org/nightly/core/pin/index.html
[`Pin::new_unchecked`]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.new_unchecked

#### Pinning 与 Futures

正如我们在这篇文章中已经看到的，[`Future::poll`] 方法通过 `Pin<&mut Self>` 参数的形式使用固定：

[`Future::poll`]: https://doc.rust-lang.org/nightly/core/future/trait.Future.html#tymethod.poll

```rust
fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>
```

该方法采用 `self: Pin<&mut Self>` 而非普通的 `&mut self` 的原因是，通过 async/await 创建的 future 实例通常是自引用的，如我们[之前][self-ref-async-await]所见的那样。将 `Self` 包装进 `Pin` 并让编译器为 async/await 生成的自引用 future 不实现 `Unpin` ，可以确保在 `poll` 调用之间这些 future 在内存中不会被移动。这确保了所有内部引用仍然有效。

[self-ref-async-await]: @/edition-2/posts/12-async-await/index.md#self-referential-structs

值得注意的是，在首次调用 `poll` 前移动 future 是安全的。这是由于 future 有惰性，在首次被轮询前不会执行任何操作。刚生成的状态机处于 `start` 状态，因此仅包含函数参数而不包含内部引用。为了调用 `poll` ，调用者必须先将 future 包装到 `Pin` 中，这确保了 future 在内存中不再能被移动。由于栈固定（stack pinning）更难实现，我建议在这种情况下始终结合使用 [`Box::pin`] 和 [`Pin::as_mut`]。

[`futures`]: https://docs.rs/futures/0.3.4/futures/

如果你有兴趣了解如何安全地使用栈固定（pinning）技术自行实现一个 future 组合器函数，可以参考 `futures` crate 中相对简短的 [map 组合器 方法的源代码][map-src] 以及 pin 文档中关于 [projections and structural pinning] 的章节。

[map-src]: https://docs.rs/futures-util/0.3.4/src/futures_util/future/future/map.rs.html
[projections and structural pinning]: https://doc.rust-lang.org/stable/std/pin/index.html#projections-and-structural-pinning

### Executors 与 Wakers

使用 async/await 可以以完全异步的方式更符合人体工程学地处理 futures。 然而，正如我们之前所学，futures 在被轮询前不会执行任何操作。这意味着我们必须在某时刻调用 `poll` ，否则异步代码永远不会执行。

对于单个 future，我们总是可以像[上面描述](#等待 Future 就绪)的那样使用循环手动等待每个future。然而，这种方法效率非常低下，对于创建大量 futures 的程序来说不太实用。解决这个问题最常见的方法是定义一个全局的执行器  _executor_ ，它负责轮询系统中所有的 future 直到它们全部完成。

#### Executors 执行器

执行器的目的是允许将 future 作为独立任务生成，通常通过某种 `spawn` 方法。然后，执行器负责轮询所有 future 直到它们完成。集中管理所有 future 的最大优势在于，当某个 future 返回 `Poll::Pending` 时，执行器可以立即切换到另一个 future。这样，异步操作就能并行运行，使得 CPU 保持忙碌状态。

许多执行器实现还能充分利用多核 CPU 系统的优势。它们会创建一个 [线程池][thread pool] ，在有足够多任务时能够利用所有核心资源，并采用诸如 [工作窃取][work stealing] 等技术来平衡各核心之间的负载。还有一些特殊的、适用于嵌入式系统的执行器实现，针对低延迟和内存占用进行优化。

[thread pool]: https://en.wikipedia.org/wiki/Thread_pool
[work stealing]: https://en.wikipedia.org/wiki/Work_stealing

为了避免重复轮询 future 带来的开销，执行器通常会利用 Rust 的 future 所支持的 _waker_ API。

#### Wakers 唤醒器

waker API 的核心思想是：每次调用 `poll` 时都会传入一个特殊的 `Waker` 类型, 封装在 [`Context`] 类型中。这个  `Waker`  类型由执行器创建，可被异步任务用来通知其已完成或者部分完成的状态。因此，执行器无需对之前返回 `Poll::Pending` 的 future 重复调用 `poll` ，直到收到对应 waker 的通知。

[`Context`]: https://doc.rust-lang.org/nightly/core/task/struct.Context.html

通过一个小例子可以很好地说明这一点：

```rust
async fn write_file() {
    async_write_file("foo.txt", "Hello").await;
}
```

此函数会异步地将字符串 "Hello" 写入 `foo.txt` 文件。由于硬盘写入需要一定时间，首次轮询这个 future 时很可能会返回 `Poll::Pending` 。硬盘驱动器会在内部存储传递给 `poll` 调用的 `Waker` ，并在文件写入磁盘时使用它来通知执行器。这样，执行器在收到唤醒通知之前就无需浪费任何时间来尝试轮询该 future。

在这篇文章的实现部分，我们将通过创建一个支持 waker 的自定义执行器来了解 `Waker` 类型的具体工作原理。

### 协作式多任务处理?

在这篇文章的开头，我们讨论了抢占式和协作式多任务处理。抢占式多任务依赖操作系统强制切换运行中的任务，而协作式多任务则要求任务通过定期执行 _yield_  操作主动放弃 CPU 控制权。协作式方法的最大优势在于任务能够自行保存状态，从而实现更高效的上下文切换，并允许任务间共享同一个调用栈。

虽然可能不太明显，但 futures 和 async/await 实际上是一种协作式多任务模式的实现：

* 每个添加到执行器的 future 本质上都是协作式任务。
* 相对于使用显式的 yield 操作符，future 通过 `Poll::Pending`（或在最后  `Poll::Ready`）放弃 CPU 核心的控制权。
  * 并没有谁要强制 future 放弃 CPU。如果它们想，它们可以永不从 `poll` 中返回。例如通过无限循环。
  * 由于每个 future 都有能力阻断执行器中其他 future 的执行，我们得首先相信它们是无恶意的。
* Future 内部存储了所有在下一次 `poll` 调用时继续执行所需的状态。使用 async/await 时，编译器会自动检测所有需要的变量并将它们存储在生成的状态机内部。
  * 仅保存继续执行所需的最小状态。
  * 由于 `poll` 方法在返回时会释放调用栈，这同一个栈可以用于轮询其他 future。

我们看到 future 和 async/await 完美契合协作式多任务模式；它们只不过使用了一些不同的术语。因此在下文中，术语 "任务 task" 和 "future" 可以互换使用。

## 实现

既然我们已经理解了基于 future 和 async/await 的协作式多任务在 Rust 中是如何工作的，现在就该为我们的内核添加对它们的支持了。由于 `Future` trait 是 `core` 库的一部分，而 async/await 是语言本身的特性，我们无需特别处理就能在我们的 `#![no_std]` 内核中使用它。唯一的要求是我们至少需要使用 `2020-03-25` 之后的 Rust nightly 版本，因为在此之前async/await 还不适用于 `no_std` 。

只要使用足够新的 nightly 版本，我们就可以在 `main.rs` 中开始使用 async/await：

```rust
// in src/main.rs

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}
```

`async_number` 函数是一个异步函数 `async fn`，因此编译器会将其转换为一个实现了 `Future` 的状态机。由于该函数仅返回 `42`，最终生成的 future 将直接在第一次 `poll` 调用时返回 `Poll::Ready(42)` 。与 `async_number` 类似，`example_task` 函数也是一个 `async fn`。它会等待（awaits）`async_number` 返回的数字，然后使用 `println` 宏打印该数字。

要运行 `example_task` 返回的 future，我们需要对其调用 `poll` 直到它通过返回 `Poll::Ready` 来告知它已经完成。为此，我们需要创建一个简单的执行器类型。

### Task 模块

在开始实现执行器之前，我们先创建一个包含 `Task` 类型的新 `task` 模块：

```rust
// in src/lib.rs

pub mod task;
```

```rust
// in src/task/mod.rs

use core::{future::Future, pin::Pin};
use alloc::boxed::Box;

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}
```

`Task` 结构体是一个针对已固定、堆分配且动态分发并以空类型 `()` 作为输出的 future 而设计的新包装器。让我们详细了解一下：

* 我们要求与任务关联的 future 返回 `()`。这意味着任务不会返回任何结果，它们的运行会产生一些效果，例如，我们上面定义的 `example_task` 函数没有返回值，但它会向屏幕打印一些东西。
* `dyn` 关键字表示我们在 `Box` 中存储了一个 [_trait object_] 。这意味着 future 上的方法是 [动态分发_dynamically dispatched_][_dynamically dispatched_] 的，这使得不同类型的 future 能够存储在 `Task` 类型中。这一占很重要，因为每个 `async fn` 都有自己的类型，而我们希望能够创建多种不同的任务。
* 正如我们在 [固定 相关章节][section about pinning] 中学到的， `Pin<Box>` 类型通过将值放在堆上并组织创建  `&mut`  引用来确保它不会在内存中被移动。这一点很重要，因为由 async/await 生成的 future 可能是自引用的。 也就是说会包含指向自己的指针，这些指针会在 future 移动过程中失效。

[_trait object_]: https://doc.rust-lang.org/book/ch17-02-trait-objects.html
[_dynamically dispatched_]: https://doc.rust-lang.org/book/ch17-02-trait-objects.html#trait-objects-perform-dynamic-dispatch
[section about pinning]: #pinning

为了从 future 创建新的 `Task` 结构体，我们创建了一个 `new` 函数：

```rust
// in src/task/mod.rs

impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            future: Box::pin(future),
        }
    }
}
```

该函数接收任意输出类型为 `()` 的 future，并通过 `Box::pin` 函数将其固定在内存中。然后将 box 后的 future 包装在 `Task` 结构体中并返回。此处需要 `'static` 生命周期，因为返回的 `Task` 可以存活任意时长，所以 future 在这个时间内也必须保持有效。

我们还添加了一个 `poll` 方法，允许执行器轮询存储的 future：

```rust
// in src/task/mod.rs

use core::task::{Context, Poll};

impl Task {
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}
```

由于 `Future` trait 的 `poll` 方法期望被 `Pin<&mut T>` 类型调用，我们使用 `Pin::as_mut` 方法先转换 `Pin<Box<T>>` 类型的 `self.future` 字段。然后我们在转换后的 `self.future` 字段上调用 `poll` 并返回结果。由于 `Task::poll` 方法应仅由我们稍后将创建的执行器调用，因此我们将该函数保留为 `task` 模块的私有方法。

### 简单的执行器

考虑到执行器可能变得相当复杂，在后续实现功能更全面的执行器之前，我们先从创建一个非常基础的执行器开始。为此，我们先创建一个新的 `task::simple_executor` 子模块：

```rust
// in src/task/mod.rs

pub mod simple_executor;
```

```rust
// in src/task/simple_executor.rs

use super::Task;
use alloc::collections::VecDeque;

pub struct SimpleExecutor {
    task_queue: VecDeque<Task>,
}

impl SimpleExecutor {
    pub fn new() -> SimpleExecutor {
        SimpleExecutor {
            task_queue: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }
}
```

该结构体包含一个类型为 [`VecDeque`] 的 `task_queue` 字段，其本质上是一个向量，允许在两端进行推入和弹出操作。采用这种类型的初衷是我们可以使用 `spawn` 方法在结尾插入新的任务，并从开头弹出下一个任务用于执行。这样子，我们就得到了一个简单的 [FIFO 队列][FIFO queue] （_"first in, first out"_）。

[`VecDeque`]: https://doc.rust-lang.org/stable/alloc/collections/vec_deque/struct.VecDeque.html
[FIFO queue]: https://en.wikipedia.org/wiki/FIFO_(computing_and_electronics)

#### 假的唤醒器（Dummy Waker）

为了调用 `poll` 方法，我们需要创建一个 `Context` 类型，它封装了一个 `Waker` 类型。为了简单起见，我们将首先创建一个什么都不做的假 waker。为此，我们需要创建一个 [`RawWaker`] 实例，它定义了不同 `Waker` 方法的实现，然后使用 [`Waker::from_raw`] 函数将其转换为 `Waker`：

[`RawWaker`]: https://doc.rust-lang.org/stable/core/task/struct.RawWaker.html
[`Waker::from_raw`]: https://doc.rust-lang.org/stable/core/task/struct.Waker.html#method.from_raw

```rust
// in src/task/simple_executor.rs

use core::task::{Waker, RawWaker};

fn dummy_raw_waker() -> RawWaker {
    todo!();
}

fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}
```

`from_raw` 函数是不安全的，因为如果程序员未能遵守 `RawWaker` 文档的要求，就可能出现未定义行为。在我们关注 `dummy_raw_waker` 函数的实现之前，先来理解 `RawWaker` 类型的工作原理。

##### `RawWaker`

`RawWaker` 类型要求程序员显式定义一个 [虚方法表 virtual method table][_virtual method table_] (vtable)。 该表指定了当 `RawWaker` 被克隆、唤醒或被释放时应当调用的函数。该 vtable 的布局由 [`RawWakerVTable`] 类型定义。每个函数接收一个 `*const ()` 参数，这是一个指向某个值的 _类型擦除type-erased_ 的指针。

使用 `*const ()` 指针而非一个合适的引用的原因是 `RawWaker` 类型应当不是泛型（non-generic）但是仍支持任意类型。为了提供该指针，我们将指针放入 [`RawWaker::new`] （这个函数用于初始化 `RawWaker`）的 `data` 参数中。随后 `Waker` 会使用这个 `RawWaker` 的 `data` 调用 vtable 函数。

[_virtual method table_]: https://en.wikipedia.org/wiki/Virtual_method_table
[`RawWakerVTable`]: https://doc.rust-lang.org/stable/core/task/struct.RawWakerVTable.html
[`RawWaker::new`]: https://doc.rust-lang.org/stable/core/task/struct.RawWaker.html#method.new

通常， `RawWaker` 是为某个堆分配的、被包装在 [`Box`] 或者 [`Arc`] 类型中的结构体创建的。对于这类类型，可以使用诸如 [`Box::into_raw`] 这样的方法来将  `Box<T>` 转换为 `*const T` 指针。随后该指针可被转换为匿名的  `*const ()` 指针并传递给 `RawWaker::new`。由于每个虚表函数都接收相同的 `*const ()` 作为参数，这些函数可以安全地将指针转换回 `Box<T>` 或者 `&T` 来操作。可以想象，这个过程极其危险，很容易导致未定义行为。因此，除非必要，不建议手动创建 `RawWaker` 。

[`Box`]: https://doc.rust-lang.org/stable/alloc/boxed/struct.Box.html
[`Arc`]: https://doc.rust-lang.org/stable/alloc/sync/struct.Arc.html
[`Box::into_raw`]: https://doc.rust-lang.org/stable/alloc/boxed/struct.Box.html#method.into_raw

##### 一个假的 `RawWaker`（A Dummy `RawWaker`）

虽然不建议手动创建 `RawWaker` ，但目前尚无其他方式来创建一个什么都不做的假 `Waker` 。幸运的是，正因为什么都不做，实现 `dummy_raw_waker` 函数显得相对安全一点：

```rust
// in src/task/simple_executor.rs

use core::task::RawWakerVTable;

fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(0 as *const (), vtable)
}
```

首先，我们定义两个名为 `no_op` 和 `clone` 的内部函数。`no_op` 函数接收一个 `*const ()` 指针且不执行任何操作。 `clone` 函数同样接收一个 `*const ()` 指针并通过再次调用 `dummy_raw_waker` 返回一个新的 `RawWaker`。我们使用这两个函数来创建一个最简的 `RawWakerVTable`：`clone` 函数用于克隆操作，而 `no_op` 函数则用于所有其他操作。由于这个 `RawWaker` 不做任何实际工作，因此从 `clone` 返回一个新的 `RawWaker` 而非克隆它本身也没关系。

创建完 `vtable` 后，我们使用 `RawWaker::new` 函数来创建 `RawWaker`。被传递的 `*const ()` 无关紧要，因为 vtable 中没有任何一个函数使用它。因此，我们只需传递一个空指针。

#### 一个 `run` 方法

既然我们已经掌握了创建 `Waker` 实例的方法，就可以用它为我们的执行器实现一个 `run` 方法。 最简单的 `run` 方法就是在循环中不断轮询所有排队中的任务，直到它们全部完成。这种方式效率不高，因为它没有利用 `Waker` 类型的通知机制，但这是一个快速上手的简单方法：

```rust
// in src/task/simple_executor.rs

use core::task::{Context, Poll};

impl SimpleExecutor {
    pub fn run(&mut self) {
        while let Some(mut task) = self.task_queue.pop_front() {
            let waker = dummy_waker();
            let mut context = Context::from_waker(&waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {} // 任务完成
                Poll::Pending => self.task_queue.push_back(task),
            }
        }
    }
}
```

该函数使用 `while let` 循环来处理 `task_queue` 中的所有任务。对于每个任务，它首先通过包装由我们的 `dummy_waker` 函数返回的 `Waker` 实例来创建一个 `Context` 类型。然后它使用这个 `context` 调用 `Task::poll` 方法。如果 `poll` 方法返回  `Poll::Ready`，就表示任务已完成，我们可以接着处理下一个任务。如果任务仍处于 `Poll::Pending` 状态，我们会再次将其添加到队列末尾，以便后续的循环迭代再次轮询它。

#### 尝试

现在有了 `SimpleExecutor` 类型，我们可以在 `main.rs` 中尝试运行 `example_task` 函数返回的任务：

```rust
// in src/main.rs

use blog_os::task::{Task, simple_executor::SimpleExecutor};

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] 初始化过程，包括 `init_heap`

    let mut executor = SimpleExecutor::new();
    executor.spawn(Task::new(example_task()));
    executor.run();

    // […] test_main, "it did not crash" 信息, hlt_loop
}


// 下面再次展示 example_task 函数，方便阅读

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}
```

当我们运行它时，会看到预期的 _"async number: 42"_ 消息打印在屏幕上：

![QEMU printing "Hello World", "async number: 42", and "It did not crash!"](qemu-simple-executor.png)

让我们总结一下这个示例中发生的各个步骤：

* 首先，创建一个新的 `SimpleExecutor` 类型实例，其 `task_queue` 为空。
* 接着，我们调用异步函数 `example_task`，该函数返回一个 future。我们将这个 future 包装在 `Task` 类型中，这会将其移动到堆上并固定，然后通过 `spawn` 方法将任务添加到执行器的 `task_queue` 中。
* 接着我们调用 `run` 方法来启动队列中单个任务的执行。这包括：
  * 从 `task_queue` 前端弹出任务。
  * 为任务创建一个 `RawWaker` ，将其转换为`Waker` 实例，之后从它创建一个 `Context` 实例。
  * 使用 `Context` ，在任务的 future 上调用 `poll` 方法。
  * 由于 `example_task` 并不需要等待什么，它可以在第一次轮询就直接跑完。于是就会打印出 _"async number: 42"_ 消息。
  * 由于 `example_task` 直接返回 `Poll::Ready` ，它不会被重新添加到 `task_queue` 尾部。
* `run` 方法会在 `task_queue` 变空之后返回。`kernel_main` 函数会继续执行，并打印 _"It did not crash!"_ 。

### 异步键盘输入

我们的简单执行器并未利用 `Waker` 通知机制，而只是循环遍历所有任务直到它们完成。这对我们的示例来说不是问题，因为我们的 `example_task` 可以在首次轮询时直接运行至结束。要了解正确使用 `Waker` 带来的性能优势，我们首先需要创建一个真正异步的任务，即一个能够在第一次轮询调用时返回 `Poll::Pending` 的任务。

我们的系统中已经具备某种异步机制，可以为此所用：硬件中断。正如我们在 [中断][_Interrupts_] 一文中了解到的，硬件中断可能在任何时间点发生，这由外部设备决定。例如，硬件定时器会在预定义的时间间隔过后发送一个中断信号给 CPU。当 CPU 接收到中断时，立即将控制权转移至中断描述符表（IDT）中定义的相应处理函数。 

[_Interrupts_]: @/edition-2/posts/07-hardware-interrupts/index.md

接下来我们将基于键盘中断创建一个异步任务。键盘中断是一个很好的选择，因为它既具有非确定性又对延迟敏感。非确定性意味着无法预测下一次按键何时发生，因为这完全取决于用户。延迟敏感意味着我们需要及时处理键盘输入，否则用户会感受到延迟。为了高效支持此类任务，执行器必须对 `Waker` 通知提供适当支持。

#### 扫描码队列（Scancode Queue）

目前，我们直接在中断处理程序中处理键盘输入。这种做法从长远来看并不理想，中断处理程序应尽可能保持简短，因为它们可能会阻碍重要工作。事实上，中断处理程序只应执行最必要的少量工作（例如读取键盘扫描码），而将其余工作（例如解释扫描码）留给后台任务处理。

将工作委托给后台任务的常见模式是创建某种队列。中断处理程序将工作单元推入队列，后台任务则处理队列中的工作。对于我们的键盘中断来说，这意味着中断处理程序仅从键盘读取扫描码，将其推入队列后直接返回。键盘任务位于队列的另一端，负责解释并处理每个被推送过来的扫描码：

![Scancode queue with 8 slots on the top. Keyboard interrupt handler on the bottom left with a "push scancode" arrow to the left of the queue. Keyboard task on the bottom right with a "pop scancode" arrow coming from the right side of the queue.](scancode-queue.svg)

该队列的一个简单实现可以是受互斥锁保护的 `VecDeque`。然而，在中断处理程序中使用互斥锁并不是个好主意，因为这很容易导致死锁。例如，在键盘任务将队列锁定时用户按下按键，中断处理程序会尝试再次获取锁并无限期挂起。此方法的另一个问题是 `VecDeque` 在快满时会通过执行新的堆分配来自动增加其容量。这可能导致再次出现死锁，因为我们的分配器内部也使用了互斥锁。进一步的问题在于，由于堆内存已碎片化，堆内存分配可能失败或耗费相当长的时间。

为了避免这些问题，我们需要一种在 `push` 时无需互斥锁或内存分配的队列实现。这类队列可通过使用无锁（lock-free）[原子操作][atomic operations] 压入或者弹出元素来实现。这样，就可以创建只需要 `&self` 引用，无需互斥锁就可以使用的 `push` 和 `pop` 操作。为避免在 `push` 时分配内存，我们可以使用一个预分配的固定大小的缓冲区。虽然这会导致队列变得有界（即有最大长度），但是实践中通常可以定义出一个合理的上界，所以这不是啥大问题。

[atomic operations]: https://doc.rust-lang.org/core/sync/atomic/index.html

##### `crossbeam` Crate

以正确且高效的方式实现这样的队列非常困难，因此我建议使用现有的、经过充分测试的实现方案。有一个名为 [`crossbeam`] 的流行的 Rust 项目实现了多种用于并发编程的无互斥锁类型。它提供了一种名为 [`ArrayQueue`] 的类型，这正是我们当前场景所需要的。而且很幸运的是，该类型完全兼容支持内存分配的 `no_std` crate。

[`crossbeam`]: https://github.com/crossbeam-rs/crossbeam
[`ArrayQueue`]: https://docs.rs/crossbeam/0.7.3/crossbeam/queue/struct.ArrayQueue.html

要使用该类型，我们需要添加对 `crossbeam-queue` 的依赖：

```toml
# in Cargo.toml

[dependencies.crossbeam-queue]
version = "0.3.11"
default-features = false
features = ["alloc"]
```

默认情况下，该 crate 依赖于标准库。要使其兼容 `no_std` ，需要禁用其默认特性并启用 `alloc` 特性。（注意，我们也可以添加对主 `crossbeam` crate 的依赖，它会重新导出 `crossbeam-queue` crate，但这样会导致依赖项增多，延长编译时间。)

##### 队列实现

使用 `ArrayQueue` 类型，我们现在可以在新的 `task::keyboard` 模块中创建一个全局扫描码队列：

```rust
// in src/task/mod.rs

pub mod keyboard;
```

```rust
// in src/task/keyboard.rs

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
```

由于 [`ArrayQueue::new`] 会执行堆分配操作，而这在编译时[还][const-heap-alloc]无法实现，我们无法直接初始化静态变量。为此，我们使用了 [`conquer_once`] crate 的 [`OnceCell`] 类型，它能安全地实现静态值的一次性初始化。要引入该 crate，我们需要在 `Cargo.toml` 中添加它作为依赖项：

[`ArrayQueue::new`]: https://docs.rs/crossbeam/0.7.3/crossbeam/queue/struct.ArrayQueue.html#method.new
[const-heap-alloc]: https://github.com/rust-lang/const-eval/issues/20
[`OnceCell`]: https://docs.rs/conquer-once/0.2.0/conquer_once/raw/struct.OnceCell.html
[`conquer_once`]: https://docs.rs/conquer-once/0.2.0/conquer_once/index.html

```toml
# in Cargo.toml

[dependencies.conquer-once]
version = "0.2.0"
default-features = false
```

除了 [`OnceCell`] 原语，我们也可以在此处使用 [`lazy_static`] 宏。不过，`OnceCell` 类型的优势在于，我们可以确保初始化操作不会发生在中断处理程序中，从而防止中断处理程序执行堆分配操作。

[`lazy_static`]: https://docs.rs/lazy_static/1.4.0/lazy_static/index.html

#### 填充队列

为了填充扫描码队列，我们创建了一个新的 `add_scancode` 函数，它将被中断处理程序调用。

```rust
// in src/task/keyboard.rs

use crate::println;

/// 被中断处理程序调用
///
/// 不能阻塞或者分配
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}
```

我们使用 [`OnceCell::try_get`] 获取已初始化的队列的引用。如果队列尚未初始化，则忽略键盘扫描码并打印警告信息。关键点在于我们不应在此函数中尝试初始化队列，因为它会被中断处理程序调用，而中断处理程序不应执行堆分配。由于此函数不应能从我们的 `main.rs` 中调用，我们使用 `pub(crate)` 可见性使其仅对我们的 `lib.rs` 可用。

[`OnceCell::try_get`]: https://docs.rs/conquer-once/0.2.0/conquer_once/raw/struct.OnceCell.html#method.try_get

[`ArrayQueue::push`] 方法仅需要 `&self` 引用，这使得在静态队列上调用该方法非常简单。 `ArrayQueue` 类型会自行处理所有必要的同步，所以此处不需要互斥锁包装。当队列满时，我们也会打印一个警告信息。

[`ArrayQueue::push`]: https://docs.rs/crossbeam/0.7.3/crossbeam/queue/struct.ArrayQueue.html#method.push

为了在键盘中断时调用 `add_scancode` 函数，我们更新了 `interrupts` 模块中的 `keyboard_interrupt_handler` 函数：

```rust
// in src/interrupts.rs

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame
) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::task::keyboard::add_scancode(scancode); // new

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

我们移除了该函数中的所有键盘处理代码，转而添加了对 `add_scancode` 函数的调用。函数的其余部分保持与之前相同。

正如预期的那样，当我们使用 `cargo run` 运行项目时，按键不再被打印到屏幕上。相反，我们看到每次按键都会出现扫描码队列未初始化的警告。

#### 扫描码流

为了初始化 `SCANCODE_QUEUE` 并以异步方式从队列中读取扫描码，我们创建了一个 `ScancodeStream` 类型：

```rust
// in src/task/keyboard.rs

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}
```

`_private` 字段的目的是防止从模块外部构造该结构体。这使得 `new` 函数成为构造该类型的唯一方式。在函数中，我们首先尝试初始化 `SCANCODE_QUEUE` 静态变量。如果它已被初始化，我们会触发 panic 以确保只能创建 一个 `ScancodeStream` 实例。

为了使扫描码可用于异步任务，下一步是实现一个类似 `poll` （`poll`-like）的方法。该方法尝试从队列中弹出下一个扫描码。虽然这听起来像是我们应该为我们的类型实现 `Future` trait，但实际上并非如此。问题在于  `Future` trait 仅抽象单个异步值，并期望在返回 `Poll::Ready` 后不再被调用。然而，我们的扫描码队列包含多个异步值，因此可以持续轮询它。

##### `Stream` Trait

由于能产生多个异步值的类型很常见，`futures` crate 为此类类型提供了一个实用的抽象：[`Stream`] trait。该 trait 定义如下：

[`Stream`]: https://rust-lang.github.io/async-book/05_streams/01_chapter.html

```rust
pub trait Stream {
    type Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context)
        -> Poll<Option<Self::Item>>;
}
```

这个定义与 `Future` trait 非常相似，主要区别如下；

* 相关类型命名为 `Item` 而非 `Output`。
* `Stream` trait 没有定义返回 `Poll<Self::Item>` 的 `poll` 方法，而是定义了返回 `Poll<Option<Self::Item>>` 的 `poll_next` 方法（注意多出的 `Option` 包装）。

还存在语义上的差异：可以重复调用 `poll_next` ，直到它返回 `Poll::Ready(None)` 表示 stream 已结束。在这方面，该方法类似于 [`Iterator::next`] 方法，后者同样在最后一个值之后返回 `None` 。

[`Iterator::next`]: https://doc.rust-lang.org/stable/core/iter/trait.Iterator.html#tymethod.next

##### 实现 `Stream`

让我们为 `ScancodeStream` 实现 `Stream` trait，来以异步方式提供 `SCANCODE_QUEUE` 的值。为此，我们首先需要添加对 `futures-util` crate 的依赖，它包含 `Stream` 类型：

```toml
# in Cargo.toml

[dependencies.futures-util]
version = "0.3.4"
default-features = false
features = ["alloc"]
```

我们禁用默认特性以使该 crate 兼容 `no_std` ，并启用 `alloc` 使其基于分配的类型可用（我们稍后会需要这个功能）。（注意，我们也可以添加对主 `futures` crate 的依赖，它会重新导出 `futures-util` crate，但这将导致更多的依赖项和更长的编译时间。)

现在我们可以导入并实现 `Stream` trait:

```rust
// in src/task/keyboard.rs

use core::{pin::Pin, task::{Poll, Context}};
use futures_util::stream::Stream;

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE.try_get().expect("not initialized");
        match queue.pop() {
            Some(scancode) => Poll::Ready(Some(scancode)),
            None => Poll::Pending,
        }
    }
}
```

我们首先使用 [`OnceCell::try_get`] 方法来获取已初始化的扫描码队列的引用。 由于我们在 `new` 函数中已经初始化了队列，这不应当会失败，因此可以安全地使用 `expect` 方法在未初始化时触发 panic。接着，我们使用[`ArrayQueue::pop`] 方法尝试从队列中获取下一个元素。如果成功，我们返回封装在 `Poll::Ready(Some(…))` 的扫描码。若失败则表明队列为空，此时我们返回 `Poll::Pending`。

[`ArrayQueue::pop`]: https://docs.rs/crossbeam/0.7.3/crossbeam/queue/struct.ArrayQueue.html#method.pop

#### Waker 支持

与 `Futures::poll` 方法类似，`Stream::poll_next` 方法要求异步任务在返回 `Poll::Pending` 之后变为就绪状态时通知执行器。这样，执行器无需重复轮询同一任务，直到收到通知为止，这显著降低了等待任务的性能开销。

要发送此通知，任务应从传入的 `Context` 引用中提取 `Waker` 并将其存储在某处。当任务准备就绪时，应调用存储的 `Waker` 上的 `wake` 方法来通知执行器应当再次轮询该任务。

##### AtomicWaker

要为我们的 `ScancodeStream` 实现 `Waker` 通知，我们需要一个可以在两次轮询调用之间存储 `Waker` 的位置。我们不能将其作为字段存储在 `ScancodeStream` 中，因为它需要能从 `add_scancode` 函数访问。解决方案是使用 `futures-util` crate 提供的 [`AtomicWaker`] 类型的静态变量。就像 `ArrayQueue` 类型，该类型基于原子指令，可以安全地存储在 `static`  中并支持并发修改。

[`AtomicWaker`]: https://docs.rs/futures-util/0.3.4/futures_util/task/struct.AtomicWaker.html

让我们使用 `AtomicWaker` 类型来定义一个静态的 `WAKER`：

```rust
// in src/task/keyboard.rs

use futures_util::task::AtomicWaker;

static WAKER: AtomicWaker = AtomicWaker::new();
```

这个想法是让 `poll_next` 实现将当前的 waker 存储在这个静态变量中，而 `add_scancode` 函数在有新扫描码加入队列时对其执行 `wake` 函数。

##### 存储 Waker

由  `poll`/`poll_next` 定义的规则要求当任务返回 `Poll::Pending` 时，为传过来的 `Waker` 注册一个唤醒动作（wakeup）。让我们修改 `poll_next` 实现以满足这一要求：

```rust
// in src/task/keyboard.rs

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");

        // fast path
        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}
```

和之前一样，我们首先使用 `OnceCell::try_get` 函数获取已初始化的扫描码队列的引用。随后我们乐观地尝试从队列中 `pop` 扫描码，当成功时返回 `Poll::Ready` 。这样，我们就可以避免队列不为空时注册唤醒器产生的性能开销。

如果首次调用 `queue.pop()` 未成功，意味着队列可能为空。只是“可能”，是因为中断处理程序可能在检查之后立即异步地填充了队列。由于这种竞态条件可能在下次检查时再次发生，我们需要在第二次检查之前将 `Waker` 注册到 `WAKER` 静态变量中。这样，虽然在返回 `Poll::Pending` 之前有可能会收到唤醒动作，但可以确保在检查之后每一个压入的扫描码都能收到唤醒动作。

在通过 [`AtomicWaker::register`] 函数注册传入的 `Context` 中包含的 `Waker` 后，我们第二次尝试从队列中弹出。如果现在成功，我们返回 `Poll::Ready`。同时我们使用 [`AtomicWaker::take`] 再次移除已注册的 waker，因为不再需要唤醒通知。当 `queue.pop()` 第二次失败时，我们会像之前一样返回 `Poll::Pending` ，但这次会附带一个已注册的唤醒动作。

[`AtomicWaker::register`]: https://docs.rs/futures-util/0.3.4/futures_util/task/struct.AtomicWaker.html#method.register
[`AtomicWaker::take`]: https://docs.rs/futures/0.3.4/futures/task/struct.AtomicWaker.html#method.take

需要注意的是，对于尚未返回 `Poll::Pending` 的任务，有两种方式可能触发唤醒。一种方式是前面提到的竞态 条件，当唤醒在返回 `Poll::Pending` 之前时立即发生。另一种情况是当注册唤醒器后队列不再为空，此时会返回 `Poll::Ready` 。由于这些虚假的唤醒无法避免，执行器需要能够正确处理它们。

##### 唤醒存储的唤醒器

为了唤醒存储的 `Waker`，我们在 `add_scancode` 函数中添加了对 `WAKER.wake()` 的调用：

```rust
// in src/task/keyboard.rs

pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            WAKER.wake(); // new
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}
```

我们唯一所做的更改是在将数据成功推送到扫描码队列时添加了对 `WAKER.wake()` 的调用。如果在 `WAKER` 静态变量中注册了唤醒器，此方法将调用其同名的 [`wake`] 方法，从而通知执行器。否则该操作将无实际效果，什么都不会发生。

[`wake`]: https://doc.rust-lang.org/stable/core/task/struct.Waker.html#method.wake

关键点在于我们必须只在推入队列之后再调用 `wake` ，否则任务可能会在队列仍为空时过早被唤醒。这种情况可能发生在例如一个多线程执行器在不同的 CPU 核心上并发启动被唤醒的任务时。虽然我们现在还没添加线程支持，但之后我们会实现，并且不希望出问题。

#### 键盘任务

既然我们已经为 `ScancodeStream` 实现了 `Stream` trait，现在我们可以用它来创建一个异步键盘任务：

```rust
// in src/task/keyboard.rs

use futures_util::stream::StreamExt;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use crate::print;

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(ScancodeSet1::new(),
        layouts::Us104Key, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}
```

这段代码与我们之前在 [键盘中断处理程序][keyboard interrupt handler] 中的代码非常相似，只是在本篇文章中进行了修改。唯一的区别在于，我们不再从 I/O 端口读取扫描码，而是从 `ScancodeStream` 获取。为此，我们首先创建一个新的 `Scancode` 流，然后重复使用 [`StreamExt`] trait 中提供的 [`next`] 方法来获取一个 `Future` ，这个 `Future` 可以解析为流中下一个元素。通过在其上使用 await 来异步地等待其结果。

[keyboard interrupt handler]: @/edition-2/posts/07-hardware-interrupts/index.md#interpreting-the-scancodes
[`next`]: https://docs.rs/futures-util/0.3.4/futures_util/stream/trait.StreamExt.html#method.next
[`StreamExt`]: https://docs.rs/futures-util/0.3.4/futures_util/stream/trait.StreamExt.html

我们使用 `while let` 循环直到流返回 `None` 表示结束。由于我们的 `poll_next` 方法永远不会返回 `None` ，因此这实际上是一个无限循环，所以 `print_keypresses` 任务永远不会结束。

让我们将 `print_keypresses` 任务添加到 `main.rs` 的执行器中，来让键盘输入再次正常工作：

```rust
// in src/main.rs

use blog_os::task::keyboard; // new

fn kernel_main(boot_info: &'static BootInfo) -> ! {

    // […] 初始化过程，包括 init_heap, test_main

    let mut executor = SimpleExecutor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::print_keypresses())); // new
    executor.run();

    // […] "it did not crash" 信息, hlt_loop
}
```

现在执行 `cargo run` 时，我们可以看到键盘输入功能已恢复：

![QEMU printing ".....H...e...l...l..o..... ...W..o..r....l...d...!"](qemu-keyboard-output.gif)

如果你留意观察电脑的 CPU 使用率，就会发现 `QEMU` 进程现在持续占用 CPU 资源。这是因为我们的 `SimpleExecutor` 在一个循环中反复轮询任务。因此即使我们没有在键盘上按下任何键，执行器也会持续调用 `print_keypresses` 任务的 `poll` 方法，即使该任务此时无法取得任何进展并且会返回 `Poll::Pending` 。

### 支持 Waker 的 Executor

为解决性能问题，我们需要创建一个能正确利用 `Waker` 通知的执行器。这样，当下一个键盘中断发生时，执行器会收到通知，从而不需要反复轮询 `print_keypresses` 任务。

#### 任务 ID

创建支持唤醒通知的执行器的第一步是为每个任务分配唯一 ID。这是必需的，因为我们需要一种方式来指定应该唤醒哪个任务。我们首先创建一个新的 `TaskId` 包装类型：

```rust
// in src/task/mod.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);
```

`TaskId` 结构体是 `u64` 的简单包装类型。我们为其派生多个 trait 以使其可打印、可复制、可比较并且可排序。后者很重要，因为我们希望使用 `TaskId` 作为 [`BTreeMap`] 的键类型。

[`BTreeMap`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html

为了创建新的唯一 ID，我们创建了一个 `TaskId::new` 函数：

```rust
use core::sync::atomic::{AtomicU64, Ordering};

impl TaskId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}
```

该函数使用了一个静态的 `NEXT_ID` 变量，其类型为 [`AtomicU64`] 以确保每个 ID 都仅被赋值一次。[`fetch_add`] 方法会原子性地增加该值并在单个原子操作中返回先前值。这意味着即使当 `TaskId::new` 方法被并行调用，每个 ID 都只被返回一次。[`Ordering`] 参数决定是否允许编译器在指令流中重新排列 `fetch_add` 操作。由于我们仅要求 ID 唯一，因此在此情况下，具有最弱要求的 `Relaxed` 排序就足够了。

[`AtomicU64`]: https://doc.rust-lang.org/core/sync/atomic/struct.AtomicU64.html
[`fetch_add`]: https://doc.rust-lang.org/core/sync/atomic/struct.AtomicU64.html#method.fetch_add
[`Ordering`]: https://doc.rust-lang.org/core/sync/atomic/enum.Ordering.html

我们现在可以为 `Task` 类型扩展一个额外的 `id` 字段：

```rust
// in src/task/mod.rs

pub struct Task {
    id: TaskId, // new
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            id: TaskId::new(), // new
            future: Box::pin(future),
        }
    }
}
```

新的 `id` 字段能够为任务赋予唯一名称，这是唤醒特定任务所必需的。

#### `Executor` 类型

我们在 `task::executor` 模块中创建新的 `Executor` 类型：

```rust
// in src/task/mod.rs

pub mod executor;
```

```rust
// in src/task/executor.rs

use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc};
use core::task::Waker;
use crossbeam_queue::ArrayQueue;

pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(100)),
            waker_cache: BTreeMap::new(),
        }
    }
}
```

与在 `SimpleExecutor` 中使用 `VecDeque` 存储任务不同，我们使用一个存储任务 ID 的 `task_queue` 和一个名为 `tasks` 、包含实际 `Task` 实例的 `BTreeMap`。该 map 通过 `TaskId` 高效地索引特定任务。

`task_queue` 字段是一个存储任务 ID 的 `ArrayQueue` 类型，被封装在 [`Arc`] 类型中以实现引用计数（_reference counting_）。引用计数可以实现在多个所有者之间共享所有权。它通过在堆上分配值并统计其活跃的引用来实现。当活跃引用数量降至零时，该值将被不再需要，可以释放。

我们给 `task_queue` 使用 `Arc<ArrayQueue>` 类型，因为它将在执行器和唤醒器之间共享。其设计思路是唤醒器将被唤醒任务的 ID 推送到队列中。执行器位于队列的接收端，通过 ID 从 `tasks` map 中检索被唤醒的任务，然后运行它们。选择固定大小队列而非无界队列（例如 [`SegQueue`]）的原因是在往其中推入数据时，中断处理程序不应该进行内存分配。

除了 `task_queue` 和 `tasks` map 外，`Executor` 类型还有一个 `waker_cache` 字段，同样为 map。该 map 会在任务创建后缓存其 `Waker`，原因有二：首先，它通过为同一任务的多次唤醒复用同一个唤醒器来提高性能，而不是每次都创建新的唤醒器。其次，它确保引用计数的唤醒器不会在中断处理程序中被释放，因为这可能导致死锁（下文将对此进行更详细的说明）。

[`Arc`]: https://doc.rust-lang.org/stable/alloc/sync/struct.Arc.html
[`SegQueue`]: https://docs.rs/crossbeam-queue/0.2.1/crossbeam_queue/struct.SegQueue.html

为了创建执行器，我们提供了一个简单的 `new` 函数。我们设置 `task_queue` 的容量为100，这在可预见的未来应该绰绰有余。万一未来我们的系统并发任务数超过100，我们也可以轻松增加这个容量。

#### 生成任务

对于 `SimpleExecutor`，我们针对 `Executor` 类型提供了 `spawn` 方法，用于添加给定的任务添加到 `tasks` map 中，并通过将其 ID 推送到 `task_queue` 来立即唤醒它：

```rust
// in src/task/executor.rs

impl Executor {
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        self.task_queue.push(task_id).expect("queue full");
    }
}
```

如果 map 中已存在相同 ID 的任务，`BTreeMap::insert` 方法会将其返回。这种情况不应发生，因为每个任务都有唯一 ID，所以此时我们触发 panic，这表明我们的代码中存在错误。同样地，当 `task_queue` 已满时我们也会触发 panic，因为如果我们选择一个足够大的队列大小，这种情况也不应该发生。

#### 运行任务

要执行 `task_queue` 中的所有任务，我们创建私有的 `run_ready_tasks` 方法：

```rust
// in src/task/executor.rs

use core::task::{Context, Poll};

impl Executor {
    fn run_ready_tasks(&mut self) {
        // 解构 `self` 来避免借用检查器报错
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // 任务不存在
            };
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // 任务完成 -> 移除它和它缓存的唤醒器
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }
}
```

该函数的基本思路与我们的 `SimpleExecutor` 类似：循环遍历 `task_queue` 中的所有任务，为每个任务创建一个唤醒器，然后轮询它们。不过与将待处理任务重新放回 `task_queue` 的末尾不同，我们让 `TaskWaker` 负责将唤醒的任务重新加入队列。该唤醒器类型的实现将在稍后展示。

让我们深入看看这个 `run_ready_tasks` 方法的一些实现细节：

* 我们使用 [解构][_destructuring_] 将 self 拆分为三个字段，以避免一些借用检查器报错。具体来说，我们的实现需要从一个闭包内访问 `self.task_queue`，这会导致尝试借用自身。这是一个基本的借用检查器问题，该问题将在 [RFC 2229] 被 [实现][RFC 2229 impl] 后得到解决。
* 对于每个弹出的任务 ID，我们从 `tasks` map 中获取对应任务的可变引用。由于我们的 `ScancodeStream` 实现在检查任务是否需要进入休眠状态前会先注册唤醒器，可能会出现一个已不存在的任务被唤醒的情况。这种情况下，我们只需忽略这次唤醒并继续处理队列里的下一个 ID。
* 为了避免每次轮询时创建唤醒器带来的性能开销，我们使用了 `waker_cache` map 用于存储每个任务创建后对应的唤醒器。为此，我们使用 [`BTreeMap::entry`] 方法结合 [`Entry::or_insert_with`] ，来在唤醒器不存在时创建新实例，然后获取其可变引用。为了创建新的唤醒器，我们克隆 `task_queue` 并将其与任务 ID 一同传递给 `TaskWaker::new` 函数（具体实现如下所示）。由于 `task_queue` 被封装在 `Arc` 中，克隆操作仅会增加该值的引用计数，但仍指向同一个堆分配的队列。请注意，并非所有唤醒器的实现都能像这样重复使用，不过我们的 `TaskWaker` 类型可以做到。

[_destructuring_]: https://doc.rust-lang.org/book/ch18-03-pattern-syntax.html#destructuring-to-break-apart-values
[RFC 2229]: https://github.com/rust-lang/rfcs/pull/2229
[RFC 2229 impl]: https://github.com/rust-lang/rust/issues/53488

[`BTreeMap::entry`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html#method.entry
[`Entry::or_insert_with`]: https://doc.rust-lang.org/alloc/collections/btree_map/enum.Entry.html#method.or_insert_with

当任务返回 `Poll::Ready` 时即视为完成。此时我们会使用 [`BTreeMap::remove`] 方法将其从 `tasks` map 中移除。我们还会移除其缓存的唤醒器，如果存在的话。

[`BTreeMap::remove`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html#method.remove

#### 唤醒器的设计

唤醒器的作用是将被唤醒任务的 ID 推送到执行器的 `task_queue` 中。我们通过创建一个新的 `TaskWaker` 结构体来实现这一点。该结构体存储任务 ID 和对 `task_queue` 的引用：

```rust
// in src/task/executor.rs

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}
```

由于 `task_queue` 的所有权在执行器和唤醒器之间共享，我们使用 [`Arc`] 包装类型来实现共享的引用计数所有权。

[`Arc`]: https://doc.rust-lang.org/stable/alloc/sync/struct.Arc.html

唤醒操作的实现相当简单：

```rust
// in src/task/executor.rs

impl TaskWaker {
    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task_queue full");
    }
}
```

我们将 `task_id` 推送到引用的 `task_queue`。由于对 [`ArrayQueue`] 类型的修改仅需要一个共享引用，我们可以在 `&self` 上实现此方法，而非 `&mut self` 。

##### `Wake` Trait

为了使用我们的 `TaskWaker` 类型轮询 future，首先需要将其转换为 [`Waker`] 实例。这是必需的，因为[`Future::poll`] 函数使用一个 [`Context`] 实例作为参数，而该实例只能从 `Waker` 类型构造。虽然我们可以通过提供对 [`RawWaker`] 类型的实现来做到这一点，但还是这么做更简单且安全：实现基于  `Arc` 的 [`Wake`][wake-trait] trait 并使用标准库提供的 [`From`] 实现来构造 `Waker`。

该 trait 的实现如下所示：

[wake-trait]: https://doc.rust-lang.org/nightly/alloc/task/trait.Wake.html

```rust
// in src/task/executor.rs

use alloc::task::Wake;

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}
```

由于唤醒器通常在执行器与异步任务之间共享，该 trait 方法要求将 `Self` 实例包装在实现了引用计数所有权的 [`Arc`] 类型中。这意味着为了调用它们，我们需要移动 `TaskWaker` 到 `Arc` 。

 `wake` 和 `wake_by_ref` 方法之间的区别在于，后者只需要一个对 `Arc` 的引用，而前者则获取 `Arc` 的所有权，因此通常需要增加引用计数。并非所有类型都支持通过引用唤醒，因此对 `wake_by_ref` 方法的实现是可选的。不过，它能带来更好的性能，因为它避免了不必要的引用计数修改。在我们的案例中，可以简单地将这两个 trait 方法导向（forward）我们的 `wake_task` 函数，该函数只需要一个共享的 `&self` 引用。

##### 创建唤醒器

由于 `Waker` 类型对所有实现了 `Wake` trait 且用 `Arc` 包装的值都支持 [`From`] 转换，我们现在可以实现 `Executor::run_ready_tasks` 方法所需的 `TaskWaker::new` 函数了：

[`From`]: https://doc.rust-lang.org/nightly/core/convert/trait.From.html

```rust
// in src/task/executor.rs

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }
}
```

我们使用传入的 `task_id` 和 `task_queue` 创建 `TaskWaker` 。然后，将其包装在 `Arc` 中，并通过 `Waker::from` 实现将其转换为 [`Waker`]。这个 `from` 方法负责为我们的 `TaskWaker` 类型构建 [`RawWakerVTable`] 和 [`RawWaker`] 的实例。如果您对其工作原理细节感兴趣，请查看 [implementation in the `alloc` crate][waker-from-impl]。

[waker-from-impl]: https://github.com/rust-lang/rust/blob/cdb50c6f2507319f29104a25765bfb79ad53395c/src/liballoc/task.rs#L58-L87

#### `run` 方法

有了我们的唤醒器实现，现在终于可以为执行器构建一个 `run` 方法：

```rust
// in src/task/executor.rs

impl Executor {
    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
        }
    }
}
```

该方法只需循环调用 `run_ready_tasks` 函数。虽然理论上我们可以在 `tasks` map 为空时从函数返回，但由于我们的 `keyboard_task` 永远不会完成，这种情况永远不会发生，因此一个简单的 `loop` 循环就足够了。由于该函数永远不会返回，我们使用 `!` 返回类型将函数标记为发散。

[diverging]: https://doc.rust-lang.org/stable/rust-by-example/fn/diverging.html

现在我们可以修改 `kernel_main` 来使用新的 `Executor` 替代 `SimpleExecutor`：

```rust
// in src/main.rs

use blog_os::task::executor::Executor; // new

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] initialization routines, including init_heap, test_main

    let mut executor = Executor::new(); // new
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.run();
}
```

我们只需更改导入和类型名称。由于我们的 `run` 函数被标记为发散，编译器知道它永远不会返回，因此我们不再需要在 `kernel_main` 函数末尾调用 `hlt_loop`。

现在我们使用 `cargo run` 运行内核时，可以看到键盘输入仍然有效：

![QEMU printing ".....H...e...l...l..o..... ...a..g..a....i...n...!"](qemu-keyboard-output-again.gif)

然而，QEMU 的 CPU 利用率并未得到改善。原因在于我们仍然让 CPU 持续处于忙碌状态。我们不再一直轮询任务到它们被再次唤醒，但仍在循环中频繁地检查 `task_queue` 。为了解决这个问题，我们需要让 CPU 在没有任务时进入休眠状态。

#### 空闲时休眠

基本思路是在 `task_queue` 为空时执行 [hlt 指令][`hlt` instruction]。该指令会让 CPU 进入休眠状态，直到下一个中断到来。CPU 能在中断发生时立即重新激活，这确保了当中断处理程序向 `task_queue` 推送时，系统仍能立即作出响应。

[`hlt` instruction]: https://en.wikipedia.org/wiki/HLT_(x86_instruction)

为实现此功能，我们在执行器中创建了一个新的 `sleep_if_idle` 方法，并从我们的 `run` 方法中调用它：

```rust
// in src/task/executor.rs

impl Executor {
    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();   // new
        }
    }

    fn sleep_if_idle(&self) {
        if self.task_queue.is_empty() {
            x86_64::instructions::hlt();
        }
    }
}
```

由于我们在 `run_ready_tasks` 之后直接调用了 `sleep_if_idle` ，而该函数会循环执行直到 `task_queue` 为空，再次检查队列可能显得多余。然而，硬件中断可能在 `run_ready_tasks` 返回后立即发生，因此在调用 `sleep_if_idle` 函数时可能会有新任务进入队列。仅当队列仍为空时，我们才会使用 [`x86_64`] crate 提供的 [`instructions::hlt`] 包装函数来执行 `hlt` 指令使 CPU 进入休眠。

[`instructions::hlt`]: https://docs.rs/x86_64/0.14.2/x86_64/instructions/fn.hlt.html
[`x86_64`]: https://docs.rs/x86_64/0.14.2/x86_64/index.html

遗憾的是，这个实现中仍存在一个微妙的竞态条件。由于中断是异步的且可能在任何时刻发生，有可能在 `is_empty` 检查与 `hlt` 调用之间恰好发生中断：

```rust
if self.task_queue.is_empty() {
    /// <--- 中断可能在此处发生
    x86_64::instructions::hlt();
}
```

若此时中断向 `task_queue` 推送了任务，我们就会让 CPU 进入休眠，尽管此时队列中已有任务等待运行。最坏情况下，这可能导致键盘中断的处理被延迟，直至下一次按键或定时器中断。那么我们该如何防止这种情况呢？

答案是在检查前禁用 CPU 中断，并在之后与 `hlt` 指令一起原子性地重新启用中断。这样，中间发生的所有中断都会被延迟到执行 `hlt` 指令后，确保不会错过任何唤醒动作。为实现这一方法，我们可以使用 [`x86_64`] crate 提供的 [`interrupts::enable_and_hlt`][`enable_and_hlt`] 函数。

[`enable_and_hlt`]: https://docs.rs/x86_64/0.14.2/x86_64/instructions/interrupts/fn.enable_and_hlt.html

更新后的 `sleep_if_idle` 函数实现如下：

```rust
// in src/task/executor.rs

impl Executor {
    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts::{self, enable_and_hlt};

        interrupts::disable();
        if self.task_queue.is_empty() {
            enable_and_hlt();
        } else {
            interrupts::enable();
        }
    }
}
```

为避免竞态条件，我们在检查 `task_queue` 是否为空前会先禁用中断。如果为空，则使用 `enable_and_hlt` 函数以单一原子操作的形式来启用中断并使 CPU 进入睡眠。若队列不再为空，则意味着有中断在 `run_ready_tasks` 返回后唤醒了一个任务。在这种情况下，我们再次启用中断，并且直接继续处理任务，而不执行 `hlt` 指令。

现在我们的执行器在没有任务时会正确让 CPU 进入休眠状态。可以看到，当我们再次使用 `cargo run` 运行内核时，QEMU 进程的 CPU 占用率大幅降低。

#### 可能的扩展功能

我们的执行器现在能够高效地运行任务。它利用唤醒通知机制来避免轮询等待中的任务，并在当前无工作可做时让 CPU 进入休眠状态。不过，我们的执行器仍相当基础，还有许多扩展其功能的可能性：

* **调度：**对于我们的 `task_queue`，我们目前使用 `VecDeque` 类型来实现 FIFO 策略，这也经常被称作 Round Robin 调度。该策略可能并非对所有工作负载都最高效。例如，在某些情况下，优先处理对延迟敏感的任务或执行大量 I/O 操作的任务会更高效。详情请参阅 [_Operating Systems: Three Easy Pieces_] 中的 [scheduling chapter] 章节或者 [Wikipedia article on scheduling][scheduling-wiki] 。
* **任务生成：**当前我们的 `Executor::spawn` 方法需要 `&mut self` 引用，因此在调用 `run` 方法后就不再可用。为解决这个问题，我们可以创建一个 `Spawner` 类型，它与执行器共享一些队列，并允许从任务自身创建新的任务。这些队列可以直接用 `task_queue` ，或者用一个单独的队列，让执行器在循环中不断检查。
* **利用线程：**目前我们尚未支持线程功能，但将在下一篇文章中添加该功能。这将允许在不同线程中启动多个执行器实例。这种方法的优势在于，由于其他任务可以并发运行，因此可以减少长时间运行的任务造成的延迟。该方法还能充分利用多核 CPU 的处理能力。
* **负载均衡：**在添加线程支持时，了解如何在多个执行器之间分配任务以确保所有 CPU 核心都得到利用变得至关重要。实现这一点的常用技术是 [工作窃取][_work stealing_]。

[scheduling chapter]: http://pages.cs.wisc.edu/~remzi/OSTEP/cpu-sched.pdf
[_Operating Systems: Three Easy Pieces_]: http://pages.cs.wisc.edu/~remzi/OSTEP/
[scheduling-wiki]: https://en.wikipedia.org/wiki/Scheduling_(computing)
[_work stealing_]: https://en.wikipedia.org/wiki/Work_stealing

## 总结

我们在这篇文章开头介绍了**多任务处理**的概念，并区分了 _抢占式多任务处理_，包括定期强制中断运行任务的抢占式多任务，以及 _协作式多任务_，它让任务持续运行，直到它们主动放弃对 CPU 的控制权。

接着我们探讨了 Rust 对 async/await 的支持如何提供协作式多任务处理的语言层面的实现。Rust 的异步机制建立在基于轮询的 `Future` trait 之上，该 trait 对异步任务进行了抽象。通过 async/await 语法，可以像处理普通同步代码那样操作 futures。不同之处在于异步函数会再次返回一个 `Future` ，需要在某个时刻将其添加到执行器中才能运行。

在幕后，编译器将 async/await 代码转换为 _状态机_ ，其中每个 `.await` 操作对应一个可能的暂停点。利用对程序的了解，编译器能够为每个暂停点保存恢复所需的最小状态，从而使得每个任务的内存消耗非常小。一个挑战在于生成的状态机可能包含 _自引用结构体_，例如当异步函数的局部变量互相引用。为了防止指针失效，Rust 使用 `Pin` 类型来确保 future 在首次被轮询后不再在内存中移动。

在我们的实现中，我们首先创建了一个非常基础的任务执行器，它会在一个繁忙的循环里轮询所有已生成的任务，而不使用 `Waker` 类型。随后我们通过实现异步键盘任务展示了唤醒器通知的优势。该任务使用 `crossbeam` crate 提供的无互斥锁 `ArrayQueue` 类型定义了静态的 `SCANCODE_QUEUE`。键盘中断处理程序不再直接处理按键操作，而是将所有接收到的扫描码放入队列中，随后唤醒已注册的 `Waker` 以通知有新输入可用。在接收端，我们创建了一个 `ScancodeStream` 类型，用于提供 `Future` 解析，来获得队列中的下一个扫描码。这使得创建异步的 `print_keypresses` 任务，使用 async/await 解释并打印队列中的扫描码成为可能。

为了利用键盘任务的唤醒通知，我们创建了一个新的 `Executor` 类型，它使用一个 `Arc` 共享的 `task_queue` 存储就绪任务。我们实现了一个 `TaskWaker` 类型，用于将被唤醒任务的 ID 直接推送到这个 `task_queue` 中，然后由执行器再次轮询。为了在没有可运行任务时节省电量，我们通过 `hlt` 指令让 CPU 进入睡眠。最后，我们讨论了一些执行器的潜在扩展功能，例如提供多核支持。

## 下一步是什么?

通过使用 async/await，我们现在在内核中实现了基本的协作式多任务支持。协作式多任务非常高效，但当单个任务持续占用资源时会导致延迟问题，阻碍其他任务执行。正因如此，为我们的内核添加抢占式多任务处理支持就显得尤为重要。

在下一篇文章中，我们将介绍 _线程_ ——作为抢占式多任务处理最常见的形式。除了可以解决长耗时任务的问题，线程机制还将有助于我们后续使用多 CPU 核心以及未来运行不受信任的用户程序。
