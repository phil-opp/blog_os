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

Preemptive multitasking and threads are fundamental components of an operating system because they make it possible to run untrusted userspace programs. We will therefore discuss these concepts in full detail in future posts. For this post, however, we will focus on cooperative multitasking, which also provides useful capabilities for our kernel.

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

However, the strong performance and memory benefits of cooperative multitasking make it a good approach for usage _within_ a program, especially in combination with asynchronous operations. Since an operating system kernel is a performance-critical program that interacts with asynchronous hardware, cooperative multitasking seems like a good approach for concurrency in our kernel. In the remainder of this post, we will therefore implement a basic async/await based multitasking system.

## Async/Await in Rust









