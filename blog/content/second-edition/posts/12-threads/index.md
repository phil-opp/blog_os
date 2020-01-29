+++
title = "Threads"
weight = 12
path = "threads"
date = 0000-01-01

[extra]
chapter = "Multitasking"
+++

TODO

<!-- more -->

This blog is openly developed on [GitHub]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom]. The complete source code for this post can be found in the [`post-12`][post branch] branch.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-12

<!-- toc -->

## Multitasking

One of the fundamental features of most operating systems is [_multitasking_], which is the ability to execute multiple tasks concurrently. For example, you probably have other programs open while looking at this blog post, such as a text editor or a terminal window. Even if you have only a single browser window open, there are probably various background tasks for managing your desktop windows, checking for updates, or indexing files.

[_multitasking_]: https://en.wikipedia.org/wiki/Computer_multitasking

While it seems like all tasks run in parallel, only a single task can be executed on a CPU core at a time. This means that there can be at most 4 active tasks on a quad-core CPU and only a single active task on a single core CPU. A common technique to work around this hardware limitation is _time slicing_.

### Time Slicing

The idea of time slicing is to rapidly switch between tasks multiple times per second. Each task is allowed to run for a short time, then it is paused and another task becomes active. The time until the next task switch is called a _time slice_. By setting the time slice as low as 10ms, it appears like the tasks run in parallel.

![Visualization of time slices for two CPU cores. Core 1 uses fixed time slices, while core 2 uses variable timeslices.](time_slicing.svg)

The above graphic shows an example for time slicing on two CPU cores. Each color in the graphic hereby represents a different task. CPU core 1 uses a fixed time slice length, which gives each task exactly the same execution time until it is paused again. CPU core 2, on the other hand, uses a variable time slice length. It also does not switch between tasks in a varying order and even executes tasks from core 1 at some times. As we will learn below, both variants have their advantages, so it's up to the operating system designer to decide.

### Preemption

In order to enforce time slices, the operating system must be able to pause a task when its time slice is used up. For this, it must first regain control of the CPU core. Remember, a CPU core can only execute a single task at a time, so the OS kernel can't "run in background" either.

A common way to regain control after a specific time is to program a hardware timer. After the time is elapsed, the timer sends an [interrupt] to the CPU, which in turn invokes an interrupt handler in the kernel. Now the kernel has control again and perform the necessary work to switch to the next task. This technique of forcibly interrupting a running task is called _preemption_ or _preemptive multitasking_.

[interrupt]: @/second-edition/posts/07-hardware-interrupts/index.md

### Cooperative Multitasking

An alternative to enforcing time slices and preempting tasks is to make the tasks _cooperate_. The idea is that each task periodically relinquishes control of the CPU to the kernel, so that the kernel can switch between tasks without forcibly interrupting them. This action of giving up control of the CPU is often called [_yield_].

[_yield_]: https://en.wikipedia.org/wiki/Yield_(multithreading)

The advantage of cooperating multitasking is that a task can specify its pause points itself, which can lead to less memory use and better performance. The drawback is that an uncooperative task can hold onto the CPU as long as it desires, thereby stalling other tasks. Since a single malicious or buggy task can be enough to block or considerably slow down the system, cooperative multitasking is seldom used at the operating system level today. It is, however, often used at the language level in form of [coroutines] or [async/await].

[coroutines]: https://en.wikipedia.org/wiki/Coroutine#Implementations_for_Rust
[async/await]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html

Cooperative multitasking and `async/await` are complex topics on their own, so we will explore them in a separate post. For this post, we will focus on preemtive multitasking.

## Threads

In the previous section, we talked about _tasks_ without specifying them further. The most common task abstraction in operating systems is a _thread of execution_, which is commonly just called "thread". A thread describes a series of instructions that should be executed and a stack for storing itermediate data.

## Thread Creation

### Stack Allocation

## Switching Stacks

## Saving Registers

## Scheduler

## Summary
TODO

## What's next?

TODO