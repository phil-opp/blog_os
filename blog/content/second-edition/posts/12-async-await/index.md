+++
title = "Async/Await"
weight = 12
path = "async-await"
date = 0000-01-01

[extra]
chapter = "Interrupts"
+++

In this post we explore _cooperative multitasking_ and the _async/await_ feature of Rust. This will make it possible to run multiple concurrent tasks in our kernel. TODO

<!-- more -->

This blog is openly developed on [GitHub]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom]. The complete source code for this post can be found in the [`post-12`][post branch] branch.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-12

<!-- toc -->

## Multitasking

One of the fundamental features of most operating systems is [_multitasking_], which is the ability to execute multiple tasks concurrently. For example, you probably have other programs open while looking at this post, such as a text editor or a terminal window. Even if you have only a single browser window open, there are probably various background tasks for managing your desktop windows, checking for updates, or indexing files.

[_multitasking_]: https://en.wikipedia.org/wiki/Computer_multitasking

While it seems like all tasks run in parallel, only a single task can be executed on a CPU core at a time. To create the illusion that the tasks run in parallel, the operating system rapidly switches between active tasks so that each one can make a bit of progress. Since computers are fast, we don't notice these switches most of the time.

While single-core CPUs can only execute a single task at a time, multi-core CPUs can run multiple tasks in a truly parallel way. For example, a CPU with 8 cores can run 8 tasks at the same time. We will explain how to setup multi-core CPUs in a future post. For this post, we will focus on single-core CPUs for simplicity. (It's worth noting that all multi-core CPUs start with only a single active core, so we can treat them as single-core CPUs for now.)

There are two forms of multitasking: _Cooperative_ multitasking requires tasks to regularly give up control of the CPU so that other tasks can make progress. _Preemptive_ multitasking uses operating system capabilities to switch threads at arbitrary points in time by forcibly pausing them. In the following we will explore the two forms of multitasking in more detail and discuss their respective advantages and drawbacks.

### Preemptive Multitasking

The idea behind preemptive multitasking is that the operating system controls when to switch tasks. For that, it utilizes the fact that it regains control of the CPU on each interrupt. This makes it possible to switch tasks whenever new input is available to the system. For example, it would be possible to switch tasks when the mouse is moved or a network packet arrives. The operating system can also determine the exact time that a task is allowed to run by configuring a hardware timer to send an interrupt after that time.

The following graphic illustrates the task switching process on a hardware interrupt:

![](regain-control-on-interrupt.svg)

In the first row, the CPU is executing task `A1` of program `A`. All other tasks are paused. In the second row, a hardware interrupt arrives at the CPU. As described in the [_Hardware Interrupts_] post, the CPU immediately stops the execution of task `A1` and jumps to the interrupt handler defined in the interrupt descriptor table (IDT). Through this interrupt handler, the operating system now has control of the CPU again, which allows it to switch to task `B1` instead of continuing task `A1`.

[_Hardware Interrupts_]: @/second-edition/posts/07-hardware-interrupts/index.md

#### Saving State

Since tasks are interrupted at arbitrary points in time, they might be in the middle of some calculation. In order to be able to resume them later, the operating system must backup the whole state of the task, including its [call stack] and the values of all CPU registers. This process is called a [_context switch_].

[call stack]: https://en.wikipedia.org/wiki/Call_stack
[_context switch_]: https://en.wikipedia.org/wiki/Context_switch

As the call stack can be very large, the operating system typically sets up a separate call stack for each task instead of backing up the call stack content on each task switch. Such a task with a separate stack is called a [_thread of execution_] or _thread_ for short. By using a separate stack for each task, only the register contents need to be saved on a context switch (including the program counter and stack pointer). This approach minimizes the performance overhead of a context switch, which is very important since context switches often occur up to 100 times per second.

[_thread of execution_]: https://en.wikipedia.org/wiki/Thread_(computing)

#### Discussion

The main advantage of preemptive multitasking is that the operating system can fully control the allowed execution time of a task. This way, it can guarantee that each task gets a fair share of the CPU time, without the need to trust the tasks to cooperate. This is especially important when running third-party tasks or when multiple users share a system.

The disadvantage of preemption is that each task requires its own stack. Compared to a shared stack, this results in a higher memory usage per task and often limits the number of tasks in the system. Another disadvantage is that the operating system always has to save the complete CPU register state on each task switch, even if the task only used a small subset of the registers.

Preemptive multitasking and threads are fundamental components of an operating system because they make it possible to run untrusted userspace programs. We will discuss these concepts in full detail in future posts. For this post, however, we will focus on cooperative multitasking, which also provides useful capabilities for our kernel.

### Cooperative Multitasking

Instead of forcibly pausing running tasks at arbitrary points in time, cooperative multitasking lets each task run until it voluntarily gives up control of the CPU. This allows tasks to pause themselves at convenient points in time, for example when it needs to wait for an I/O operation anyway.

Cooperative multitasking is often used at the language level, for example in form of [coroutines] or [async/await]. The idea is that either the programmer or the compiler inserts [_yield_] operations into the program, which give up control of the CPU and allow other tasks to run. For example, a yield could be inserted after each iteration of a complex loop.

[coroutines]: https://en.wikipedia.org/wiki/Coroutine
[async/await]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html
[_yield_]: https://en.wikipedia.org/wiki/Yield_(multithreading)

It is common to combine cooperative multitasking with [asynchronous operations]. Instead of [blocking] until an operation is finished and preventing other tasks to run in this time, asynchronous operations return a "not ready" status if the operation is not finished yet. In this case, the waiting task can execute a yield operation to let other tasks run.

[asynchronous operations]: https://en.wikipedia.org/wiki/Asynchronous_I/O
[blocking]: http://faculty.salina.k-state.edu/tim/ossg/Device/blocking.html

#### Saving State

Since tasks define their pause points themselves, they don't need the operating system to save their state. Instead, they can save exactly the state they need for continuation before they pause themselves, which often results in better performance. For example, a task that just finished a complex computation might only need to backup the final result of the computation since it does not need the intermediate results anymore.

Language-supported implementations of cooperative tasks are often even able to backup up the required parts of the call stack before pausing. As an example, Rust's async/await implementation stores all local variables that are still needed in an automatically generated struct (see below). By backing up the relevant parts of the call stack before pausing, all tasks can share the same call stack, which results in a much smaller memory consumption per task. As a result, it is possible to create an almost arbitrary number of tasks without running out of memory.

#### Discussion

The drawback of cooperative multitasking is that an uncooperative task can potentially run for an unlimited amount of time. Thus, a malicious or buggy task can prevent other tasks from running and slow down or even block the whole system. For this reason, cooperative multitasking should only be used when all tasks are known to cooperate. As a counterexample, it's not a good idea to make the operating system rely on the cooperation of arbitrary userlevel programs.

However, the strong performance and memory benefits of cooperative multitasking make it a good approach for usage _within_ a program, especially in combination with asynchronous operations. Since an operating system kernel is a performance-critical program that interacts with asynchronous hardware, cooperative multitasking seems like a good approach for concurrency in our kernel.

## Async/Await in Rust

The Rust language provides first-class support for cooperative multitasking in form of async/await. Before we can explore what async/await is and how it works, we need to understand how _futures_ and asynchronous programming work in Rust.

### Futures

A _future_ represents a value that might not be available yet. This could be for example an integer that is computed by another task or a file that is downloaded from the network. Instead of waiting until the value is available, futures make it possible to continue execution until the value is needed.

#### Example

The concept of futures is best illustrated with a small example:

![Sequence diagram: main calls `read_file` and is blocked until it returns; then it calls `foo()` and is also blocked until it returns. The same process is repeated, but this time `async_read_file` is called, which directly returns a future; then `foo()` is called again, which now runs concurrently to the file load. The file is available before `foo()` returns.](async-example.svg)

This sequence diagram shows a `main` function that reads a file from the file system and then calls a function `foo`. This process is repeated two times: Once with a synchronous `read_file` call and once with an asynchronous `async_read_file` call.

With the synchronous call, the `main` function needs to wait until the file is loaded from the file system. Only then it can call the `foo` function, which requires it to again wait for the result.

With the asynchronous `async_read_file` call, the file system directly returns a future and loads the file asynchronously in the background. This allows the `main` function to call `foo` much earlier, which then runs in parallel with the file load. In this example, the file load even finishes before `foo` returns, so `main` can directly work with the file without further waiting after `foo` returns.

#### Futures in Rust

In Rust, futures are represented by the [`Future`] trait, which looks like this:

[`Future`]: https://doc.rust-lang.org/nightly/core/future/trait.Future.html

```rust
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
}
```

The [associated type] `Output` specfies the type of the asynchronous value. For example, the `async_read_file` function in the diagram above would return a `Future` instance with `Output` set to `File`.

[associated type]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#specifying-placeholder-types-in-trait-definitions-with-associated-types

The [`poll`] method allows to check if the value is already available. It returns a [`Poll`] enum, which looks like this:

[`poll`]: https://doc.rust-lang.org/nightly/core/future/trait.Future.html#tymethod.poll
[`Poll`]: https://doc.rust-lang.org/nightly/core/task/enum.Poll.html

```rust
pub enum Poll<T> {
    Ready(T),
    Pending,
}
```

When the value is already available (e.g. the file was fully read from disk), it is returned wrapped in the `Ready` variant. Otherwise, the `Pending` variant is returned, which signals the caller that the value is not yet available.

The `poll` method takes two arguments: `self: Pin<&mut Self>` and `cx: &mut Context`. The former behaves like a normal `&mut self` reference, with the difference that the `Self` value is [_pinned_] to its memory location. Understanding `Pin` and why it is needed is difficult without understanding how async/await works first. We will therefore explain it later in this post.

[_pinned_]: https://doc.rust-lang.org/nightly/core/pin/index.html

The purpose of the `cx: &mut Context` parameter is to pass a [`Waker`] instance to the asynchronous task, e.g. the file system load. This `Waker` allows the asynchronous task to signal that it (or a part of it) is finished, e.g. that the file was loaded from disk. Since the main task knows that it will be notified when the `Future` is ready, it does not need to call `poll` over and over again. We will explain this process in more detail later in this post when we implement an own `Waker` type.

[`Waker`]: https://doc.rust-lang.org/nightly/core/task/struct.Waker.html

### Working with Futures

We now know how futures are defined and understand the basic idea behind the `poll` method. However, we still don't know how to effectively work with futures. The problem is that futures represent results of asynchronous tasks, which might be not available yet. In practice, however, we often need these values directly for further calculations. So the question is: How can we efficiently retrieve the value of a future when we need it?

#### Waiting on Futures

One possible answer is to wait until a future becomes ready. This could look something like this:

```rust
let future = async_read_file("foo.txt");
let file_content = loop {
    match future.poll(…) {
        Poll::Ready(value) => break value,
        Poll::Pending => {}, // do nothing
    }
}
```

Here we _actively_ wait for the future by calling `poll` over and over again in a loop. The arguments to `poll` don't matter here, so we omitted them. While this solution works, it is very inefficient because we keep the CPU busy until the value becomes available.

A more efficient approach could be to _block_ the current thread until the future becomes available. This is of course only possible if you have threads, so this solution does not work for kernel, at least not yet. Even on systems where blocking is supported, it is often not desired because it turns an asynchronous task into a synchronous task again, thereby inhibiting the potential performance benefits of parallel tasks.

#### Future Combinators

An alternative to waiting is to use future combinators. Future combinators are functions like `map` that allow chaining and combining futures together, similar to the functions on [`Iterator`]. Instead of waiting on the future, these combinators return a future themselves, which applies the mapping operation on `poll`.

[`Iterator`]: https://doc.rust-lang.org/stable/core/iter/trait.Iterator.html

As an example, a simple `string_len` combinator for converting `Future<Output = String>` to a `Future<Output = usize` could look like this:

```rust
struct StringLen<F> {
    inner_future: F,
}

impl<F> Future for StringLen<F> where Fut: Future<Output = String> {
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

This code does not quite work because it does not handle [_pinning_], but it suffices as an example. The basic idea is that the `string_len` function wraps a given `Future` instance into a new `StringLen` struct, which also implements `Future`. When the wrapped future is polled, it polls the inner future. If the value is not ready yet, `Poll::Pending` is returned from the wrapped future too. If the value is ready, the string is extracted from the `Poll::Ready` variant and its length is calculated. Afterwards, it is wrapped in `Poll::Ready` again and returned.

[_pinning_]: https://doc.rust-lang.org/stable/core/pin/index.html

With this `string_len` function, we can calculate the length of an asynchronous string without waiting for it. Since the function returns a `Future` again, the caller can't work directly on the returned value, but needs to use combinator functions again. This way, the whole call graph becomes asynchronous and we can efficiently wait for multiple futures at once at some point, e.g. in the main function.

Manually writing combinator functions is difficult, therefore they are often provided by libraries. While the Rust standard library itself provides no combinator methods yet, the semi-official (and `no_std` compatible) [`futures`] crate does. Its [`FutureExt`] trait provides high-level combinator methods such as [`map`] or [`then`], which can be used to manipulate the result with arbitrary closures.

[`futures`]: https://docs.rs/futures/0.3.4/futures/
[`FutureExt`]: https://docs.rs/futures/0.3.4/futures/future/trait.FutureExt.html
[`map`]: https://docs.rs/futures/0.3.4/futures/future/trait.FutureExt.html#method.map
[`then`]: https://docs.rs/futures/0.3.4/futures/future/trait.FutureExt.html#method.then

##### Advantages

The big advantage of future combinators is that they keep the operations asynchronous. In combination with asynchronous I/O interfaces, this approach can lead to very high performance. The fact that future combinators are implemented as normal structs with trait implementations allows the compiler to excessively optimizing them. For more details, see the [_Zero-cost futures in Rust_] post, which announced the addition of futures to the Rust ecosystem.

[_Zero-cost futures in Rust_]: https://aturon.github.io/blog/2016/08/11/futures/

##### Drawbacks

While future combinators make it possible to write very efficient code, they can be difficult to use in some situations because of the type system and the closure based interface. For example, consider code like this:

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

Here we read the file `foo.txt` and then use the [`then`] combinator to chain a second future based on the file content. If the content length is smaller than the given `min_len`, we read a different `bar.txt` file and append it to `content` using the [`map`] combinator. Otherwise we return only the content of `foo.txt`.

We need to use the [`move` keyword] for the closure passed to `then` because otherwise there would be a lifetime error for `min_len`. The reason for the [`Either`] wrapper is that if and else blocks must always have the same type. Since we return different future types in the blocks, we must use the wrapper type to unify them into a single type. The [`ready`] function wraps a value into a future, which is immediately ready. The function is required here because the `Either` wrapper expects that the wrapped value implements `Future`.

[`move` keyword]: https://doc.rust-lang.org/std/keyword.move.html
[`Either`]: https://docs.rs/futures/0.3.4/futures/future/enum.Either.html
[`ready`]: https://docs.rs/futures/0.3.4/futures/future/fn.ready.html

As you can imagine, this can quickly lead to very complex code for larger projects. It gets especially complicated if borrowing and different lifetimes are involved. For this reason, a lot of work was invested to add support for async/await to Rust, with the goal of making asynchronous code radically simpler to write.

### The Async/Await Pattern

The idea behind async/await is to let the programmer write code that _looks_ like normal synchronous code, but is turned into asynchronous code by the compiler. It works based on the two keywords `async` and `await`. The `async` keyword can be used in a function signature to turn a synchronous function into an asynchronous function that returns a future:

```rust
async fn foo() -> u32 {
    0
}

// the above is roughly translated by the compiler to:
fn foo() -> impl Future<Output = u32> {
    future::ready(0)
}
```

This keyword alone wouldn't be that useful. However, inside `async` functions, the `await` keyword can be used to retrieve the asynchronous value of a future:

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

This function is a direct translation of the `example` function above, which used combinator functions. Using the `.await` operator, we can retrieve the value of a future without needing any closures or `Either` types. As a result, we can write our code like we write normal synchronous code, with the difference that _this is still asynchronous code_.

#### State Machine Transformation

What the compiler does behind this scenes is to transform the body of the `async` function into a [_state machine_], with each `.await` call representing a different state. For the above `example` function, the compiler creates a state machine with the following four states:

[_state machine_]: https://en.wikipedia.org/wiki/Finite-state_machine

![Four states: start, waiting on foo.txt, waiting on bar.txt, end](async-state-machine-states.svg)

Each state represents a different pause point of the function. The _"Start"_ and _"End"_ states represent the function at the beginning and end of its execution. The _"Waiting on foo.txt"_ state represents that the function is currently waiting for the first `async_read_file` result. Similarly, the _"Waiting on bar.txt"_ state represents the pause point where the function is waiting on the second `async_read_file` result.

The state machine implements the `Future` trait by making each `poll` call a possible state transition:

![Four states: start, waiting on foo.txt, waiting on bar.txt, end](async-state-machine-basic.svg)

The diagram uses arrows to represent state switches and diamond shapes to represent alternative ways. For example, if the `foo.txt` file is not ready, the path marked with _"no"_ is takes and the _"Waiting on foo.txt"_ state is reached. Otherwise, the _"yes"_ path is taken. The small red diamond without caption represents the `if content.len() < 100` branch of the `example` function.

We see that the first `poll` call starts the function and lets it run until it reaches a future that is not ready yet. If all futures on the path are ready, the function can run till the _"End"_ state, where it returns its result wrapped in `Poll::Ready`. Otherwise, the state machine enters a waiting state and returns `Poll::Pending`. On the next `poll` call, the state machine then starts from the last waiting state and retries the last operation.

#### Saving State

In order to be able to continue from the last waiting state, the state machine must save it internally. In addition, it must save all the variables that it needs to continue execution on the next `poll` call. This is where the compiler can really shine: Since it knows which variables are used when, it can automatically generate structs with exactly the variables that are needed.

As an example, the compiler generates the following structs for the above `example` function:

```rust
// The `example` function again so that you don't have to scroll up
async fn example(min_len: usize) -> String {
    let content = async_read_file("foo.txt").await;
    if content.len() < min_len {
        content + &async_read_file("bar.txt").await
    } else {
        content
    }
}

// The compiler-generated state structs:

struct StartState {
    min_len: usize,
}

struct WaitingOnFooTxtState {
    min_len: usize,
}

struct WaitingOnBarTxtState {
    content: String,
}

struct EndState {}
```

In the "start" and _"Waiting on foo.txt"_ states, the `min_len` parameter needs to be stored because it is required for the comparison with `content.len()` later. It is no longer stored in the _"Waiting on bar.txt"_ state because `min_len` is no longer needed after the comparison. In the _"end"_ state, no variables are stored because the function did already run to completion.

Keep in mind that this is only an example for the code that the compiler could generate. The struct names and the field layout are an implementation detail and might be different.

#### The Full State Machine Type











### The Async Keyword

The purpose of the async/await pattern is to make working with futures easier. Rust has language-level support for this pattern built on the two keywords `async` and `await`. We will explain them individually, starting with `async`.

The purpose of the `async` keyword is to turn a synchronous function into an asynchronous function that returns a `Future`:

```rust
fn synchronous() -> u32 {
    42
}

async fn asynchronous() -> u32 {
    42
}
```

While both functions specify a return type of `u32`, the `async` keyword turns the return type of the second function into `impl Future<Output = u32>`. So instead of returning an `u32` directly, the `asynchronous` function returns a type that implements the `Future` trait with output type `u32`. We can see this when we try to assign the result to a variable of type `u32`:

```rust
let val: u32 = asynchronous();
```

The compiler responds with the following error ([try it on the playground](https://play.rust-lang.org/?version=nightly&mode=debug&edition=2018&gist=590273d2f4ef75eb890c5354f788e29c)):

```
error[E0308]: mismatched types
  --> src/main.rs:3:23
   |
3  |     let val: u32 = asynchronous();
   |              ---   ^^^^^^^^^^^^^^ expected `u32`, found opaque type
   |              |
   |              expected due to this
...
10 | async fn asynchronous() -> u32 {
   |                            --- the `Output` of this `async fn`'s found opaque type
   |
   = note:     expected type `u32`
           found opaque type `impl std::future::Future`
```

The relevant part of that error message are the last two lines: It expects an `u32` because of the type annotation, but the function returned an implementation of the `Future` trait instead.

Of course, changing the return type alone would not work. Instead, the compiler also needs to convert the function body, which is `42` in our case, into a future. Since `42` is not asynchronous, the compiler just generates a future that returns the result on the first `poll`. The generated code _could_ look something like this:

```rust
struct GeneratedFuture;

impl Future for GeneratedFuture {
    type Output = u32;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        Poll::Ready(42)
    }
}

fn asynchronous() -> impl Future<Output = u32> {
    GeneratedFuture
}
```

Instead of returning `u32`, the `asynchronous` function now returns an instance of a new `GeneratedFuture` struct. This struct implements the `Future` trait by returning `Poll::Ready(42)` on `poll`. The `42` is the body of `asynchronous` in this case.

Note that this is just an example implementation. The actual code generated by the compiler uses a much more powerful approach, which we will explain in a moment.

In addition to `async` futures, Rust also supports `async` blocks:

```rust
let future = async {
    42
};
```

The `future` variable also has the type `impl Future<Output = u32>` in this case. The generated code is very similar to the `async fn`, only without a function call: `let future = GeneratedFuture;`.

We now know roughly what the `async` keyword does, but we still don't know why it's useful yet. After all, there is no advantage of returning a `impl Future<Output = u32>` instead of returning the `u32` directly. To answer this question, we have to explore different ways to work with futures.




#### Await

### Generators





