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

In the previous section, we talked about _tasks_ without specifying them further. The most common task abstraction in operating systems is a [_thread of execution_], or "thread" for short. A thread is an independent unit of processing, with an own instruction pointer and stack. The instruction pointer points to the program code and specifies the assembly instruction that should be executed next. The stack pointer points to a [call stack] that is exclusive to the thread, i.e. no other thread uses it.

[call stack]: https://en.wikipedia.org/wiki/Call_stack

A thread can be executed by a CPU core by loading the instruction and stack pointer registers of the thread:

[_thread of execution_]: https://en.wikipedia.org/wiki/Thread_%28computing%29

![Two CPU cores with instruction and stack pointer registers and a set of general-purpose registers. Four threads with a instruction and stack pointer fields. Thread 2 is loaded to core 1, thread 4 is loaded to core 2.](thread.svg)

The graphic shows the two CPU cores from the time slicing example above and four threads. Each thread has an instruction pointer field `IP` and a stack pointer field `SP`. The CPU cores have hardware registers for the instruction and stack pointers and a set of additional registers, e.g. for performing calculations. Thread 2 is loaded to core 1 and thread 4 is loaded to core 2.

To switch to a different thread, the current values of the instruction and stack pointer registers are written back to the `IP` and `SP` field of the thread structure. Then the `IP` and `SP` fields of the next thread are loaded. To ensure that the thread can correctly continue when resumed, the contents of the other CPU registers need to be stored too. One way to implement this is to store them on the call stack when pausing a thread.

Depending on the operating system design, the thread structure typically has some additional fields. For example, it is common to give each thread an unique ID to identify it. Also, thread structures often store an priority for the thread, the ID of the parent thread, or information about the thread state. Some implementations also store the register contents in the thread structure instead of pushing them to the call stack.

It is common to expose the concept of threads to userspace programs, thereby giving the program the ability to launch concurrent tasks. Most programming languages thus have support for threads, even high-level languages such as [Java], [Python], or [Ruby]. For normal Rust applications (not `#![no_std]`), thread support is available in the [`std::thread`] module.

[Java]: https://docs.oracle.com/javase/10/docs/api/java/lang/Thread.html
[Python]: https://docs.python.org/3/library/threading.html
[Ruby]: https://docs.ruby-lang.org/en/master/Thread.html
[`std::thread`]: https://doc.rust-lang.org/std/thread/index.html

## Implementation

We start our implementation by creating a new `multitasking` module:

```rust
// in src/lib.rs

pub mod multitasking;
```

The file for the `multitasking` module can be named either `src/multitasking.rs` or `src/multitasking/mod.rs`, whatever you prefer. (The same is true for the `allocator` module from the [previous post], so you can rename your `src/allocator.rs` to `src/allocator/mod.rs` if you like to have the complete module contained in a folder.)

[previous post]: @/second-edition/posts/11-allocator-designs/index.md

### A Thread Type

Since the `multitasking` module will become quite large, we organize its components into submodules. For our thread type, we create a `multitasking::thread` submodule:

```rust
// in src/multitasking/mod.rs (or src/multitasking.rs)

pub mod thread;
```

```rust
// in src/multitasking/thread.rs

use x86_64::VirtAddr;

#[derive(Debug)]
pub struct Thread {
    id: ThreadId,
    stack_pointer: Option<VirtAddr>,
    stack_bounds: Option<StackBounds>,
}
```

We define a `Thread` struct with three fields. It contains an unique `id` to identify the thread, an optional `stack_pointer` of type [`VirtAddr`], and an optional `stack_bounds` field, which contains the lower and upper bounds of the thread's stack. We don't add a field for the instruction pointer, as we will save it on the call stack instead.

[`VirtAddr`]: https://docs.rs/x86_64/0.8.1/x86_64/struct.VirtAddr.html

#### Stack Bounds

Since the `StackBounds` is related to memory management, we define it in the `memory` module:

```rust
// in src/memory.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackBounds {
    start: VirtAddr,
    end: VirtAddr,
}

impl StackBounds {
    pub fn start(&self) -> VirtAddr {
        self.start
    }

    pub fn end(&self) -> VirtAddr {
        self.end
    }
}
```

The type has fields for the start and end address of the stack. It also defines equally-named methods to read the fields. It does this instead of making the fields public to prevent other modules from modifying the fields.

#### Thread ID

The `ThreadId` type is a wrapper for an `u64` that atomically counts the thread ID up to ensure uniqueness:

```rust
// in src/multitasking/mod.rs


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadId(u64);

impl ThreadId {
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    fn new() -> Self {
        use core::sync::atomic::{AtomicU64, Ordering};
        static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);
        ThreadId(NEXT_THREAD_ID.fetch_add(1, Ordering::Relaxed))
    }
}
```

We [derive] a number of standard traits for the type to make it usable like an `u64`. To convert ID to an `u64`, we provide a simple `as_u64` method.

[derive]: https://doc.rust-lang.org/rust-by-example/trait/derive.html

The actual work happens in the `new` function: It uses an internal `NEXT_THREAD_ID` static of type [`AtomicU64`] to ensure globally unique IDs. Through the [`fetch_add`] method, we atomically read the value of the static and increase it by 1. Since we used an atomic function, this operation will also work in multithreaded contexts without handing out the same ID twice. The [`Ordering::Relaxed`] parameter tells the compiler that we only care about the atomicity of the single instruction and not about the ordering with preceding and subsequent operations.

[`AtomicU64`]: https://doc.rust-lang.org/core/sync/atomic/struct.AtomicU64.html
[`fetch_add`]: https://doc.rust-lang.org/core/sync/atomic/struct.AtomicU64.html#method.fetch_add
[`Ordering::Relaxed`]: https://doc.rust-lang.org/core/sync/atomic/enum.Ordering.html

Note that we initialize the `NEXT_THREAD_ID` static with 1 instead of 0. We do this to reserve the thread ID `0` for the root thread of our kernel, i.e. the thread that executes the `rust_main` function.

Before we show how to create new `Thread` instances, we create a function to allocate a new call stack.

### Stack Allocation

As we learned [above](#threads), each thread has its separate stack. This means that in order to create new threads, we need to allocate a new stack for them. To do this, we extend our `memory` module. First, we add a `reserve_stack_memory` function to get an unique virtual address range for a new stack:

```rust
// in src/memory.rs

/// Reserve the specified amount of virtual memory. Returns the start page.
fn reserve_stack_memory(size_in_pages: u64) -> Page {
    use core::sync::atomic::{AtomicU64, Ordering};
    static STACK_ALLOC_NEXT: AtomicU64 = AtomicU64::new(0x_5555_5555_0000);
    let start_addr = VirtAddr::new(STACK_ALLOC_NEXT.fetch_add(
        size_in_pages * Page::<Size4KiB>::SIZE,
        Ordering::Relaxed,
    ));
    Page::from_start_address(start_addr)
        .expect("`STACK_ALLOC_NEXT` not page aligned")
}
```

The function takes the stack size in form of a number of pages as argument and returns the first page of the reserved virtual memory region. Its implementation is quite similar to our `ThreadId::new` function: It uses a static [`AtomicU64`] to keep track of the next available virtual memory address and uses [`fetch_add`] to atomically increase it. The static is initialized to address `0x_5555_5555_0000` so that we can easily recognize allocated stack memory later. To convert the `size_in_pages` to a byte number, we multiply it with the page number specified by `Page::<Size4KiB>::SIZE`.

To convert the `u64` returned by `fetch_add` to a `Page`, we first convert it to a [`VirtAddr`] and then use the [`Page::from_start_address`] method. This method returns an error if the address is not aligned on a page boundary. Since we start at an aligned address and only add multiples of the page size, this error should not occur, therefore we use `expect` to panic in this case.

[`Page::from_start_address`]: https://docs.rs/x86_64/0.8.1/x86_64/structures/paging/page/struct.Page.html#method.from_start_address

With the help of the `reserve_stack_memory` function we can now define a `alloc_stack` function to create a new stack:

```rust
// in src/memory.rs

pub fn alloc_stack(
    size_in_pages: u64,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<StackBounds, mapper::MapToError> {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let guard_page = reserve_stack_memory(size_in_pages + 1);
    let stack_start = guard_page + 1;
    let stack_end = stack_start + size_in_pages;

    for page in Page::range(stack_start, stack_end) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(mapper::MapToError::FrameAllocationFailed)?;
        let flags = Flags::PRESENT | Flags::WRITABLE;
        mapper.map_to(page, frame, flags, frame_allocator)?.flush();
    }

    Ok(StackBounds {
        start: stack_start.start_address(),
        end: stack_end.start_address(),
    })
}
```

To ensure that stack overflows can't cause memory corruptions, we use a so-called _guard page_ at the bottom of the stack. A guard page is a deliberately unmapped page so that every access to it causes a [page fault] exception, which is much better than the corruption of the preceding memory page. To implement the guard page, we simply reserve an additional page and start the stack on the page after the guard page.

[page fault]: https://en.wikipedia.org/wiki/Page_fault

We then loop over each stack page using the [`Page::range`] function. For each page, we allocate a frame from the given [`FrameAllocator`] and then use the [`map_to`] function of the given [`Mapper`] instance to create the mapping in the page table. We set the `PRESENT` and `WRITABLE` flags for the mapping because stack pages should be accessible and writable. Finally, we return the bounds of the created stack wrapped in a `StackBounds` struct.

[`Page::range`]: https://docs.rs/x86_64/0.8.1/x86_64/structures/paging/page/struct.Page.html#method.range
[`FrameAllocator`]: https://docs.rs/x86_64/0.8.1/x86_64/structures/paging/trait.FrameAllocator.html
[`map_to`]: https://docs.rs/x86_64/0.8.1/x86_64/structures/paging/mapper/trait.Mapper.html#tymethod.map_to
[`Mapper`]: https://docs.rs/x86_64/0.8.1/x86_64/structures/paging/mapper/trait.Mapper.html

### Switching Stacks

### Saving Registers

### Scheduler

## Summary
TODO

## What's next?

TODO