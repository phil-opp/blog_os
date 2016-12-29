+++
title = "Double Faults"
date = "2016-11-08"
+++

In this post we explore double faults in detail. We also set up an _Interrupt Stack Table_ to catch double faults on a separate kernel stack. This way, we can completely prevent triple faults, even on kernel stack overflow.

<!--more--><aside id="toc"></aside>

As always, the complete source code is available on [Github]. Please file [issues] for any problems, questions, or improvement suggestions. There is also a [gitter chat] and a [comment section] at the end of this page.

[Github]: https://github.com/phil-opp/blog_os/tree/double_faults
[issues]: https://github.com/phil-opp/blog_os/issues
[gitter chat]: https://gitter.im/phil-opp/blog_os
[comment section]: #disqus_thread

## What is a Double Fault?
In simplified terms, a double fault is a special exception that occurs when the CPU fails to invoke an exception handler. For example, it occurs when a page fault is triggered but there is no page fault handler registered in the [Interrupt Descriptor Table][IDT] (IDT). So it's kind of similar to catch-all blocks in programming languages with exceptions, e.g. `catch(...)` in C++ or `catch(Exception e)` in Java or C#.

[IDT]: {{% relref "09-catching-exceptions.md#the-interrupt-descriptor-table" %}}

A double fault behaves like a normal exception. It has the vector number `8` and we can define a normal handler function for it in the IDT. It is really important to provide a double fault handler, because if a double fault is unhandled a fatal _triple fault_ occurs. Triple faults can't be caught and most hardware reacts with a system reset.

### Triggering a Double Fault
Let's provoke a double fault by triggering an exception for that we didn't define a handler function:

{{< highlight rust "hl_lines=10" >}}
// in src/lib.rs

#[no_mangle]
pub extern "C" fn rust_main(multiboot_information_address: usize) {
    ...
    // initialize our IDT
    interrupts::init();

    // trigger a debug exception
    unsafe { int!(1) };

    println!("It did not crash!");
    loop {}
}
{{< / highlight >}}

We use the [int! macro] of the [x86 crate] to trigger the exception with vector number `1`, which is the [debug exception]. The debug exception occurs for example when a breakpoint defined in the [debug registers] is hit. Like the [breakpoint exception], it is mainly used for [implementing debuggers].

[int! macro]: https://docs.rs/x86/0.8.0/x86/macro.int!.html
[x86 crate]: https://github.com/gz/rust-x86
[debug exception]: http://wiki.osdev.org/Exceptions#Debug
[debug registers]: https://en.wikipedia.org/wiki/X86_debug_register
[breakpoint exception]: http://wiki.osdev.org/Exceptions#Breakpoint
[implementing debuggers]: http://www.ksyash.com/2011/01/210/

We haven't registered a handler function for the debug exception in our [IDT], so the `int!(1)` line should cause a double fault in the CPU.

When we start our kernel now, we see that it enters an endless boot loop:

![boot loop](images/boot-loop.gif)

The reason for the boot loop is the following:

1. The CPU executes the [int 1] instruction, which causes a software-invoked `Debug` exception.
2. The CPU looks at the corresponding entry in the IDT and sees that the present bit isn't set. Thus, it can't call the debug exception handler and a double fault occurs.
3. The CPU looks at the IDT entry of the double fault handler, but this entry is also non-present. Thus, a _triple_ fault occurs.
4. A triple fault is fatal. QEMU reacts to it like most real hardware and issues a system reset.

[int 1]: https://en.wikipedia.org/wiki/INT_(x86_instruction)

So in order to prevent this triple fault, we need to either provide a handler function for debug exceptions or a double fault handler. We will do the latter, since we want to avoid triple faults in all cases.

### A Double Fault Handler
A double fault is a normal exception with an error code, so we can use our `handler_with_error_code` macro to create a wrapper function:

{{< highlight rust "hl_lines=10 17" >}}
// in src/interrupts/mod.rs

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();

        idt.set_handler(0, handler!(divide_by_zero_handler));
        idt.set_handler(3, handler!(breakpoint_handler));
        idt.set_handler(6, handler!(invalid_opcode_handler));
        idt.set_handler(8, handler_with_error_code!(double_fault_handler));
        idt.set_handler(14, handler_with_error_code!(page_fault_handler));

        idt
    };
}

// our new double fault handler
extern "C" fn double_fault_handler(stack_frame: &ExceptionStackFrame,
    _error_code: u64)
{
    println!("\nEXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
    loop {}
}
{{< / highlight >}}<!--end_-->

Our handler prints a short error message and dumps the exception stack frame. The error code of the double fault handler is always zero, so there's no reason to print it.

When we start our kernel now, we should see that the double fault handler is invoked:

![QEMU printing `EXCEPTION: DOUBLE FAULT` and the exception stack frame](images/qemu-catch-double-fault.png)

It worked! Here is what happens this time:

1. The CPU executes the `int 1` instruction macro, which causes a software-invoked `Debug` exception.
2. Like before, the CPU looks at the corresponding entry in the IDT and sees that the present bit isn't set. Thus, it can't call the debug exception handler and a double fault occurs.
3. The CPU jumps to the – now present – double fault handler.

The triple fault (and the boot-loop) no longer occurs, since the CPU can now call the double fault handler.

That was quite straightforward! So why do we need a whole post for this topic? Well, we're now able to catch _most_ double faults, but there are some cases where our current approach doesn't suffice.

## Causes of Double Faults
Before we look at the special cases, we need to know the exact causes of double faults. Above, we used a pretty vague definition:

> A double fault is a special exception that occurs when the CPU fails to invoke an exception handler.

What does _“fails to invoke”_ mean exactly? The handler is not present? The handler is [swapped out]? And what happens if a handler causes exceptions itself?

[swapped out]: http://pages.cs.wisc.edu/~remzi/OSTEP/vm-beyondphys.pdf

For example, what happens if… :

1. a divide-by-zero exception occurs, but the corresponding handler function is swapped out?
2. a page fault occurs, but the page fault handler is swapped out?
3. a divide-by-zero handler causes a breakpoint exception, but the breakpoint handler is swapped out?
4. our kernel overflows its stack and the [guard page] is hit?

[guard page]: {{% relref "07-remap-the-kernel.md#creating-a-guard-page" %}}

Fortunately, the AMD64 manual ([PDF][AMD64 manual]) has an exact definition (in Section 8.2.9). According to it, a “double fault exception _can_ occur when a second exception occurs during the handling of a prior (first) exception handler”. The _“can”_ is important: Only very specific combinations of exceptions lead to a double fault. These combinations are:

First Exception | Second Exception
----------------|-----------------
[Divide-by-zero],<br>[Invalid TSS],<br>[Segment Not Present],<br>[Stack-Segment Fault],<br>[General Protection Fault] | [Invalid TSS],<br>[Segment Not Present],<br>[Stack-Segment Fault],<br>[General Protection Fault]
[Page Fault] | [Page Fault],<br>[Invalid TSS],<br>[Segment Not Present],<br>[Stack-Segment Fault],<br>[General Protection Fault]

[Divide-by-zero]: http://wiki.osdev.org/Exceptions#Divide-by-zero_Error
[Invalid TSS]: http://wiki.osdev.org/Exceptions#Invalid_TSS
[Segment Not Present]: http://wiki.osdev.org/Exceptions#Segment_Not_Present
[Stack-Segment Fault]: http://wiki.osdev.org/Exceptions#Stack-Segment_Fault
[General Protection Fault]: http://wiki.osdev.org/Exceptions#General_Protection_Fault
[Page Fault]: http://wiki.osdev.org/Exceptions#Page_Fault


[AMD64 manual]: http://developer.amd.com/wordpress/media/2012/10/24593_APM_v21.pdf

So for example a divide-by-zero fault followed by a page fault is fine (the page fault handler is invoked), but a divide-by-zero fault followed by a general-protection fault leads to a double fault.

With the help of this table, we can answer the first three of the above questions:

1. If a divide-by-zero exception occurs and the corresponding handler function is swapped out, a _page fault_ occurs and the _page fault handler_ is invoked.
2. If a page fault occurs and the page fault handler is swapped out, a _double fault_ occurs and the _double fault handler_ is invoked.
3. If a divide-by-zero handler causes a breakpoint exception, the CPU tries to invoke the breakpoint handler. If the breakpoint handler is swapped out, a _page fault_ occurs and the _page fault handler_ is invoked.

In fact, even the case of a non-present handler follows this scheme: A non-present handler causes a _segment-not-present_ exception. We didn't define a segment-not-present handler, so another segment-not-present exception occurs. According to the table, this leads to a double fault.

### Kernel Stack Overflow
Let's look at the fourth question:

> What happens if our kernel overflows its stack and the [guard page] is hit?

When our kernel overflows its stack and hits the guard page, a _page fault_ occurs. The CPU looks up the page fault handler in the IDT and tries to push the [exception stack frame] onto the stack. However, our current stack pointer still points to the non-present guard page. Thus, a second page fault occurs, which causes a double fault (according to the above table).

[exception stack frame]: http://os.phil-opp.com/better-exception-messages.html#exceptions-in-detail

So the CPU tries to call our _double fault handler_ now. However, on a double fault the CPU tries to push the exception stack frame, too. Our stack pointer still points to the guard page, so a _third_ page fault occurs, which causes a _triple fault_ and a system reboot. So our current double fault handler can't avoid a triple fault in this case.

Let's try it ourselves! We can easily provoke a kernel stack overflow by calling a function that recurses endlessly:

{{< highlight rust "hl_lines=9 10 11 14" >}}
// in src/lib.rs

#[no_mangle]
pub extern "C" fn rust_main(multiboot_information_address: usize) {
    ...
    // initialize our IDT
    interrupts::init();

    fn stack_overflow() {
        stack_overflow(); // for each recursion, the return address is pushed
    }

    // trigger a stack overflow
    stack_overflow();

    println!("It did not crash!");
    loop {}
}
{{< / highlight >}}

When we try this code in QEMU, we see that the system enters a boot-loop again.

So how can we avoid this problem? We can't omit the pushing of the exception stack frame, since the CPU itself does it. So we need to ensure somehow that the stack is always valid when a double fault exception occurs. Fortunately, the x86_64 architecture has a solution to this problem.

## Switching Stacks
The x86_64 architecture is able to switch to a predefined, known-good stack when an exception occurs. This switch happens at hardware level, so it can be performed before the CPU pushes the exception stack frame.

This switching mechanism is implemented as an _Interrupt Stack Table_ (IST). The IST is a table of 7 pointers to known-good stacks. In Rust-like pseudo code:

```rust
struct InterruptStackTable {
    stack_pointers: [Option<StackPointer>; 7],
}
```

For each exception handler, we can choose an stack from the IST through the `options` field in the corresponding [IDT entry]. For example, we could use the first stack in the IST for our double fault handler. Then the CPU would automatically switch to this stack whenever a double fault occurs. This switch would happen before anything is pushed, so it would prevent the triple fault.

[IDT entry]: {{% relref "09-catching-exceptions.md#the-interrupt-descriptor-table" %}}

### Allocating a new Stack
In order to fill an Interrupt Stack Table later, we need a way to allocate new stacks. Therefore we extend our `memory` module with a new `stack_allocator` submodule:

```rust
// in src/memory/mod.rs

mod stack_allocator;

```

First, we create a new `StackAllocator` struct and a constructor function:

```rust
// in src/memory/stack_allocator.rs

use memory::paging::PageIter;

pub struct StackAllocator {
    range: PageIter,
}

impl StackAllocator {
    pub fn new(page_range: PageIter) -> StackAllocator {
        StackAllocator { range: page_range }
    }
}
```
We create a simple `StackAllocator` that allocates stacks from a given range of pages (`PageIter` is an Iterator over a range of pages; we introduced it [in the kernel heap post].).

[in the kernel heap post]:  {{% relref "08-kernel-heap.md#mapping-the-heap" %}}

We add a `alloc_stack` method that allocates a new stack:

```rust
// in src/memory/stack_allocator.rs

use memory::paging::{self, Page, ActivePageTable};
use memory::{PAGE_SIZE, FrameAllocator};

impl StackAllocator {
    pub fn alloc_stack<FA: FrameAllocator>(&mut self,
                                           active_table: &mut ActivePageTable,
                                           frame_allocator: &mut FA,
                                           size_in_pages: usize)
                                           -> Option<Stack> {
        if size_in_pages == 0 {
            return None; /* a zero sized stack makes no sense */
        }

        // clone the range, since we only want to change it on success
        let mut range = self.range.clone();

        // try to allocate the stack pages and a guard page
        let guard_page = range.next();
        let stack_start = range.next();
        let stack_end = if size_in_pages == 1 {
            stack_start
        } else {
            // choose the (size_in_pages-2)th element, since index
            // starts at 0 and we already allocated the start page
            range.nth(size_in_pages - 2)
        };

        match (guard_page, stack_start, stack_end) {
            (Some(_), Some(start), Some(end)) => {
                // success! write back updated range
                self.range = range;

                // map stack pages to physical frames
                for page in Page::range_inclusive(start, end) {
                    active_table.map(page, paging::WRITABLE, frame_allocator);
                }

                // create a new stack
                let top_of_stack = end.start_address() + PAGE_SIZE;
                Some(Stack::new(top_of_stack, start.start_address()))
            }
            _ => None, /* not enough pages */
        }
    }
}
```
The method takes mutable references to the [ActivePageTable] and a [FrameAllocator], since it needs to map the new virtual stack pages to physical frames. We define that the stack size is a multiple of the page size.

[ActivePageTable]: {{% relref "06-page-tables.md#page-table-ownership" %}}
[FrameAllocator]: {{% relref "05-allocating-frames.md#a-frame-allocator" %}}

Instead of operating directly on `self.range`, we [clone] it and only write it back on success. This way, subsequent stack allocations can still succeed if there are pages left (e.g., a call with `size_in_pages = 3` can still succeed after a failed call with `size_in_pages = 100`). In order to be able to clone `PageIter`, we add a `#[derive(Clone)]` to its definition in `src/memory/paging/mod.rs`.

[clone]: https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html#tymethod.clone

The actual allocation is straightforward: First, we choose the next page as [guard page]. Then we choose the next `size_in_pages` pages as stack pages using [Iterator::nth]. If all three variables are `Some`, the allocation succeeded and we map the stack pages to physical frames using [ActivePageTable::map]. The guard page remains unmapped.

[Iterator::nth]: https://doc.rust-lang.org/nightly/core/iter/trait.Iterator.html#method.nth
[ActivePageTable::map]: {{% relref "06-page-tables.md#more-mapping-functions" %}}

Finally, we create and return a new `Stack`, which we define as follows:

```rust
// in src/memory/stack_allocator.rs

#[derive(Debug)]
pub struct Stack {
    top: usize,
    bottom: usize,
}

impl Stack {
    fn new(top: usize, bottom: usize) -> Stack {
        assert!(top > bottom);
        Stack {
            top: top,
            bottom: bottom,
        }
    }

    pub fn top(&self) -> StackPointer {
        self.top
    }

    pub fn bottom(&self) -> StackPointer {
        self.bottom
    }
}
```
The `Stack` struct describes a stack though its top and bottom addresses.

#### The Memory Controller
Now we're able to allocate a new double fault stack. However, we add one more level of abstraction to make things easier. For that we add a new `MemoryController` type to our `memory` module:

```rust
// in src/memory/mod.rs

pub use self::stack_allocator::Stack;

pub struct MemoryController {
    active_table: paging::ActivePageTable,
    frame_allocator: AreaFrameAllocator,
    stack_allocator: stack_allocator::StackAllocator,
}

impl MemoryController {
    pub fn alloc_stack(&mut self, size_in_pages: usize) -> Option<Stack> {
        let &mut MemoryController { ref mut active_table,
                                    ref mut frame_allocator,
                                    ref mut stack_allocator } = self;
        stack_allocator.alloc_stack(active_table, frame_allocator,
                                    size_in_pages)
    }
}
```
The `MemoryController` struct holds the three types that are required for `alloc_stack` and provides a simpler interface (only one argument). The `alloc_stack` wrapper just takes the tree types as `&mut` through [destructuring] and forwards them to the `stack_allocator`. The [ref mut]-s are needed to take the inner fields by mutable reference. Note that we're re-exporting the `Stack` type since it is returned by `alloc_stack`.

[destructuring]: http://rust-lang.github.io/book/chXX-patterns.html#Destructuring
[ref mut]: http://rust-lang.github.io/book/chXX-patterns.html#ref-and-ref-mut

The last step is to create a `StackAllocator` and return a `MemoryController` from `memory::init`:

```rust
// in src/memory/mod.rs

pub fn init(boot_info: &BootInformation) -> MemoryController {
    ...

    let stack_allocator = {
        let stack_alloc_start = heap_end_page + 1;
        let stack_alloc_end = stack_alloc_start + 100;
        let stack_alloc_range = Page::range_inclusive(stack_alloc_start,
                                                      stack_alloc_end);
        stack_allocator::new_stack_allocator(stack_alloc_range)
    };

    MemoryController {
        active_table: active_table,
        frame_allocator: frame_allocator,
        stack_allocator: stack_allocator,
    }
}
```
We create a new `StackAllocator` with a range of 100 pages starting right after the last heap page.

In order to do arithmetic on pages (e.g. calculate the hundredth page after `stack_alloc_start`), we implement `Add<usize>` for `Page`:

```rust
// in src/memory/paging/mod.rs

impl Add<usize> for Page {
    type Output = Page;

    fn add(self, rhs: usize) -> Page {
        Page { number: self.number + rhs }
    }
}
```

#### Allocating a Double Fault Stack
Now we can allocate a new double fault stack by passing the memory controller to our `interrupts::init` function:

{{< highlight rust "hl_lines=8 11 12 21 22 23" >}}
// in src/lib.rs

#[no_mangle]
pub extern "C" fn rust_main(multiboot_information_address: usize) {
    ...

    // set up guard page and map the heap pages
    let mut memory_controller = memory::init(boot_info); // new return type

    // initialize our IDT
    interrupts::init(&mut memory_controller); // new argument

    ...
}


// in src/interrupts/mod.rs

use memory::MemoryController;

pub fn init(memory_controller: &mut MemoryController) {
    let double_fault_stack = memory_controller.alloc_stack(1)
        .expect("could not allocate double fault stack");

    IDT.load();
}
{{< / highlight >}}

We allocate a 4096 bytes stack (one page) for our double fault handler. Now we just need some way to tell the CPU that it should use this stack for handling double faults.

### The IST and TSS
The Interrupt Stack Table (IST) is part of an old legacy structure called _[Task State Segment]_ \(TSS). The TSS used to hold various information (e.g. processor register state) about a task in 32-bit mode and was for example used for [hardware context switching]. However, hardware context switching is no longer supported in 64-bit mode and the format of the TSS changed completely.

[Task State Segment]: https://en.wikipedia.org/wiki/Task_state_segment
[hardware context switching]: http://wiki.osdev.org/Context_Switching#Hardware_Context_Switching

On x86_64, the TSS no longer holds any task specific information at all. Instead, it holds two stack tables (the IST is one of them). The only common field between the 32-bit and 64-bit TSS is the pointer to the [I/O port permissions bitmap].

[I/O port permissions bitmap]: https://en.wikipedia.org/wiki/Task_state_segment#I.2FO_port_permissions

The 64-bit TSS has the following format:

Field  | Type
------ | ----------------
<span style="opacity: 0.5">(reserved)</span> | `u32`
Privilege Stack Table | `[u64; 3]`
<span style="opacity: 0.5">(reserved)</span> | `u64`
Interrupt Stack Table | `[u64; 7]`
<span style="opacity: 0.5">(reserved)</span> | `u64`
<span style="opacity: 0.5">(reserved)</span> | `u16`
I/O Map Base Address | `u16`

The _Privilege Stack Table_ is used by the CPU when the privilege level changes. For example, if an exception occurs while the CPU is in user mode (privilege level 3), the CPU normally switches to kernel mode (privilege level 0) before invoking the exception handler. In that case, the CPU would switch to the 0th stack in the Privilege Stack Table (since 0 is the target privilege level). We don't have any user mode programs yet, so we ignore this table for now.

#### Creating a TSS
Let's create a new TSS that contains our double fault stack in its interrupt stack table. For that we need a TSS struct. Fortunately, the `x86` crate already contains a [`TaskStateSegment` struct] that we can use:

[`TaskStateSegment` struct]: https://docs.rs/x86/0.7.1/x86/task/struct.TaskStateSegment.html

```rust
// in src/interrupts/mod.rs

use x86::task::TaskStateSegment;
```

Let's create a new TSS in our `interrupts::init` function:

{{< highlight rust "hl_lines=3 9 10" >}}
// in src/interrupts/mod.rs

const DOUBLE_FAULT_IST_INDEX: usize = 0;

pub fn init(memory_controller: &mut MemoryController) {
    let double_fault_stack = memory_controller.alloc_stack(1)
        .expect("could not allocate double fault stack");

    let mut tss = TaskStateSegment::new();
    tss.ist[DOUBLE_FAULT_IST_INDEX] = double_fault_stack.top() as u64;

    IDT.load();
}
{{< / highlight >}}

We define that the 0th IST entry is the double fault stack (any other IST index would work too). We create a new TSS through the `TaskStateSegment::new` function and load the top address (stacks grow downwards) of the double fault stack into the 0th entry.

#### Loading the TSS
Now that we created a new TSS, we need a way to tell the CPU that it should use it. Unfortunately, this is a bit cumbersome, since the TSS is a Task State _Segment_ (for historical reasons). So instead of loading the table directly, we need to add a new segment descriptor to the [Global Descriptor Table] (GDT). Then we can load our TSS invoking the [`ltr` instruction] with the respective GDT index.

[Global Descriptor Table]: http://www.flingos.co.uk/docs/reference/Global-Descriptor-Table/
[`ltr` instruction]: http://x86.renejeschke.de/html/file_module_x86_id_163.html

### The Global Descriptor Table (again)
The Global Descriptor Table (GDT) is a relict that was used for [memory segmentation] before paging became the de facto standard. It is still needed in 64-bit mode for various things such as kernel/user mode configuration or TSS loading.

[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation

We already created a GDT [when switching to long mode]. Back then, we used assembly to create valid code and data segment descriptors, which were required to enter 64-bit mode. We could just edit that assembly file and add an additional TSS descriptor. However, we now have the expressiveness of Rust, so let's do it in Rust instead.

[when switching to long mode]: {{% relref "02-entering-longmode.md#the-global-descriptor-table" %}}

We start by creating a new `interrupts::gdt` submodule:

```rust
// in src/interrupts/mod.rs

mod gdt;
```

```rust
// src/interrupts/gdt.rs

pub struct Gdt {
    table: [u64; 8],
    next_free: usize,
}

impl Gdt {
    pub fn new() -> Gdt {
        Gdt {
            table: [0; 8],
            next_free: 1,
        }
    }
}
```
We create a simple `Gdt` struct with two fields. The `table` field contains the actual GDT modeled as a `[u64; 8]`. Theoretically, a GDT can have up to 8192 entries, but this doesn't make much sense in 64-bit mode (since there is no real segmentation support). Eight entries should be more than enough for our system.

The `next_free` field stores the index of the next free entry. We initialize it with `1` since the 0th entry needs always needs to be 0 in a valid GDT.

#### User and System Segments
There are two types of GDT entries in long mode: user and system segment descriptors. Descriptors for code and data segment segments are user segment descriptors. They contain no addresses since segments always span the complete address space on x86_64 (real segmentation is no longer supported). Thus, user segment descriptors only contain a few flags (e.g. present or user mode) and fit into a single `u64` entry.

System descriptors such as TSS descriptors are different. They often contain a base address and a limit (e.g. TSS start and length) and thus need more than 64 bits. Therefore, system segments are 128 bits. They are stored as two consecutive entries in the GDT.

Consequently, we model a `Descriptor` as an `enum`:

```rust
pub enum Descriptor {
    UserSegment(u64),
    SystemSegment(u64, u64),
}
```

The flag bits are common between all descriptor types, so we create a general `DescriptorFlags` type:

```rust
bitflags! {
    flags DescriptorFlags: u64 {
        const CONFORMING        = 1 << 42,
        const EXECUTABLE        = 1 << 43,
        const USER_SEGMENT      = 1 << 44,
        const PRESENT           = 1 << 47,
        const LONG_MODE         = 1 << 53,
    }
}
```

We only add flags that are relevant in 64-bit mode. For example, we omit the read/write bit, since it is completely ignored by the CPU in 64-bit mode.

#### Code Segments
We add a function to create kernel mode code segments:

```rust
impl Descriptor {
    pub fn kernel_code_segment() -> Descriptor {
        let flags = USER_SEGMENT | PRESENT | EXECUTABLE | LONG_MODE;
        Descriptor::UserSegment(flags.bits())
    }
}
```
We set the `USER_SEGMENT` bit to indicate a 64 bit user segment descriptor (otherwise the CPU expects a 128 bit system segment descriptor). The `PRESENT`, `EXECUTABLE`, and `LONG_MODE` bits are also needed for a 64-bit mode code segment.

The data segment registers `ds`, `ss`, and `es` are completely ignored in 64-bit mode, so we don't need any data segment descriptors in our GDT.

#### TSS Segments
A TSS descriptor is a system segment descriptor with the following format:

Bit(s)                | Name | Meaning
--------------------- | ------ | ----------------------------------
0-15 | **limit 0-15** | the first 2 byte of the TSS's limit
16-39 | **base 0-23** | the first 3 byte of the TSS's base address
40-43 | **type** | must be `0b1001` for an available 64-bit TSS
44    | zero | must be 0
45-46 | privilege | the [ring level]: 0 for kernel, 3 for user
47 | **present** | must be 1 for valid selectors
48-51 | limit 16-19 | bits 16 to 19 of the segment's limit
52 | available | freely available to the OS
53-54 | ignored |
55 | granularity | if it's set, the limit is the number of pages, else it's a byte number
56-63 | **base 24-31** | the fourth byte of the base address
64-95 | **base 32-63** | the last four bytes of the base address
96-127 | ignored/must be zero | bits 104-108 must be zero, the rest is ignored

[ring level]: http://wiki.osdev.org/Security#Rings

We only need the bold fields for our TSS descriptor. For example, we don't need the `limit 16-19` field since a TSS has a fixed size that is smaller than `2^16`.

Let's add a function to our descriptor that creates a TSS descriptor for a given TSS:

```rust
impl Descriptor {
    pub fn tss_segment(tss: &'static TaskStateSegment) -> Descriptor {
        use core::mem::size_of;

        let ptr = tss as *const _ as u64;

        let mut low = PRESENT.bits();
        // base
        low.set_range(16..40, ptr.get_range(0..24));
        low.set_range(56..64, ptr.get_range(24..32));
        // limit (the `-1` in needed since the bound is inclusive)
        low.set_range(0..16, (size_of::<TaskStateSegment>() - 1) as u64);
        // type (0b1001 = available 64-bit tss)
        low.set_range(40..44, 0b1001);

        let mut high = 0;
        high.set_range(0..32, ptr.get_range(32..64));

        Descriptor::SystemSegment(low, high)
    }
}
```
We convert the passed `TaskStateSegment` reference to an `u64` and use the methods of the [`BitField` trait] to set the needed fields.
We require the `'static` lifetime for the `TaskStateSegment` reference, since the hardware might access it on every interrupt as long as the OS runs.

[`BitField` trait]: https://docs.rs/bit_field/0.6.0/bit_field/trait.BitField.html#method.get_bit

#### Adding Descriptors to the GDT
In order to add descriptors to the GDT, we add a `add_entry` method:

```rust
impl Gdt {
    pub fn add_entry(&mut self, entry: Descriptor) -> SegmentSelector {
        let index = match entry {
            Descriptor::UserSegment(value) => self.push(value),
            Descriptor::SystemSegment(value_low, value_high) => {
                let index = self.push(value_low);
                self.push(value_high);
                index
            }
        };
        SegmentSelector::new(index as u16, PrivilegeLevel::Ring0)
    }
}
```
For an user segment we just push the `u64` and remember the index. For a system segment, we push the low and high `u64` and use the index of the low value. We then use this index to return a new [SegmentSelector].

[SegmentSelector]: https://docs.rs/x86/0.8.0/x86/shared/segmentation/struct.SegmentSelector.html#method.new

The `push` method looks like this:

```rust
impl Gdt {
    fn push(&mut self, value: u64) -> usize {
        if self.next_free < self.table.len() {
            let index = self.next_free;
            self.table[index] = value;
            self.next_free += 1;
            index
        } else {
            panic!("GDT full");
        }
    }
}
```
The method just writes to the `next_free` entry and returns the corresponding index. If there is no free entry left, we panic since this likely indicates a programming error (we should never need to create more than two or three GDT entries for our kernel).

#### Loading the GDT
To load the GDT, we add a new `load` method:

```rust
impl Gdt {
    pub fn load(&'static self) {
        use x86::shared::dtables::{DescriptorTablePointer, lgdt};
        use x86::shared::segmentation;
        use core::mem::size_of;

        let ptr = DescriptorTablePointer {
            base: self.table.as_ptr() as
                *const segmentation::SegmentDescriptor,
            limit: (self.table.len() * size_of::<u64>() - 1) as u16,
        };

        unsafe { lgdt(&ptr) };
    }
}
```
We use the [`DescriptorTablePointer` struct] and the [`lgdt` function] provided by the `x86` crate to load our GDT. Again, we require a `'static` reference since the GDT possibly needs to live for the rest of the run time.

[`DescriptorTablePointer` struct]: https://docs.rs/x86/0.8.0/x86/shared/dtables/struct.DescriptorTablePointer.html
[`lgdt` function]: https://docs.rs/x86/0.8.0/x86/shared/dtables/fn.lgdt.html

### Putting it together
We now have a double fault stack and are able to create and load a TSS (which contains an IST). So let's put everything together to catch kernel stack overflows.

We already created a new TSS in our `interrupts::init` function. Now we can load this TSS by creating a new GDT:

{{< highlight rust "hl_lines=10 11 12 13" >}}
// in src/interrupts/mod.rs

pub fn init(memory_controller: &mut MemoryController) {
    let double_fault_stack = memory_controller.alloc_stack(1)
        .expect("could not allocate double fault stack");

    let mut tss = TaskStateSegment::new();
    tss.ist[DOUBLE_FAULT_IST_INDEX] = double_fault_stack.top() as u64;

    let mut gdt = gdt::Gdt::new();
    let code_selector = gdt.add_entry(gdt::Descriptor::kernel_code_segment());
    let tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
    gdt.load();

    IDT.load();
}
{{< / highlight >}}

However, when we try to compile it, the following error occurs:

```
error: `tss` does not live long enough
   --> src/interrupts/mod.rs:108:68
    |
108 |    let tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
    |                                         does not live long enough ^^^
...
111 | }
    | - borrowed value only lives until here
    |
    = note: borrowed value must be valid for the static lifetime...
```
The problem is that we require that the TSS is valid for the rest of the run time (i.e. for the `'static` lifetime). But our created `tss` lives on the stack and is thus destroyed at the end of the `init` function. So how do we fix this problem?

We could allocate our TSS on the heap using `Box` and use [into_raw] and a bit of `unsafe` to convert it to a `&'static` ([RFC 1233] was closed unfortunately).

Alternatively, we could store it in a `static` somehow. The [`lazy_static` macro] doesn't work here, since we need access to the `MemoryController` for initialization. However, we can use its fundamental building block, the [`spin::Once` type].

[into_raw]: https://doc.rust-lang.org/std/boxed/struct.Box.html#method.into_raw
[RFC 1233]: https://github.com/rust-lang/rfcs/pull/1233
[`lazy_static` macro]: https://docs.rs/lazy_static/0.2.2/lazy_static/
[`spin::Once` type]: https://docs.rs/spin/0.4.5/spin/struct.Once.html

#### spin::Once
Let's try to solve our problem using [`spin::Once`][`spin::Once` type]:

```rust
// in src/interrupts/mod.rs

use spin::Once;

static TSS: Once<TaskStateSegment> = Once::new();
static GDT: Once<gdt::Gdt> = Once::new();
```
The `Once` type allows us to initialize a `static` at runtime. It is safe because the only way to access the static value is through the provided methods ([call_once][Once::call_once], [try][Once::try], and [wait][Once::wait]). Thus, no value can be read before initialization and the value can only be initialized once.

[Once::call_once]: https://docs.rs/spin/0.4.5/spin/struct.Once.html#method.call_once
[Once::try]: https://docs.rs/spin/0.4.5/spin/struct.Once.html#method.try
[Once::wait]: https://docs.rs/spin/0.4.5/spin/struct.Once.html#method.wait

So let's rewrite our `interrupts::init` function to use the static `TSS` and `GDT`:

{{< highlight rust "hl_lines=5 8 9 11 16 17" >}}
pub fn init(memory_controller: &mut MemoryController) {
    let double_fault_stack = memory_controller.alloc_stack(1)
        .expect("could not allocate double fault stack");

    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.ist[DOUBLE_FAULT_IST_INDEX] = double_fault_stack.top() as u64;
        tss
    });

    let gdt = GDT.call_once(|| {
        let mut gdt = gdt::Gdt::new();
        let code_selector = gdt.add_entry(gdt::Descriptor::
                            kernel_code_segment());
        let tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
        gdt
    });
    gdt.load();

    IDT.load();
}
{{< / highlight >}}

Now it should compile again!

#### The final Steps
We're almost done. We successfully loaded our new GDT, which contains a TSS descriptor. Now there are just a few steps left:

1. We changed our GDT, so we should reload the `cs`, the code segment register. This required since the old segment selector could point a different GDT descriptor now (e.g. a TSS descriptor).
2. We loaded a GDT that contains a TSS selector, but we still need to tell the CPU that it should use that TSS.
3. As soon as our TSS is loaded, the CPU has access to a valid interrupt stack table (IST). Then we can tell the CPU that it should use our new double fault stack by modifying our double fault IDT entry.

For the first two steps, we need access to the `code_selector` and `tss_selector` variables outside of the closure. We can achieve this by moving the `let` declarations out of the closure:

{{< highlight rust "hl_lines=3 4 7 8 11 12 19 21" >}}
// in src/interrupts/mod.rs
pub fn init(memory_controller: &mut MemoryController) {
    use x86::shared::segmentation::{SegmentSelector, set_cs};
    use x86::shared::task::load_tr;
    ...

    let mut code_selector = SegmentSelector::empty();
    let mut tss_selector = SegmentSelector::empty();
    let gdt = GDT.call_once(|| {
        let mut gdt = gdt::Gdt::new();
        code_selector = gdt.add_entry(gdt::Descriptor::kernel_code_segment());
        tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
        gdt
    });
    gdt.load();

    unsafe {
        // reload code segment register
        set_cs(code_selector);
        // load TSS
        load_tr(tss_selector);
    }

    IDT.load();
}
{{< / highlight >}}

We first set the descriptors to `empty` and then update them from inside the closure (which implicitly borrows them as `&mut`). Now we're able to reload the code segment register using [`set_cs`] and to load the TSS using [`load_tr`].

[`set_cs`]: https://docs.rs/x86/0.8.0/x86/shared/segmentation/fn.set_cs.html
[`load_tr`]: https://docs.rs/x86/0.8.0/x86/shared/task/fn.load_tr.html

Now that we loaded a valid TSS and interrupt stack table, we can set the stack index for our double fault handler in the IDT:

{{< highlight rust "hl_lines=8" >}}
// in src/interrupt/mod.rs

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();
        ...
        idt.set_handler(8, handler_with_error_code!(double_fault_handler))
            .set_stack_index(DOUBLE_FAULT_IST_INDEX as u8);
        ...
    };
}

{{< / highlight >}}

TODO `set_stack_index` method?

That's it! Now the CPU should switch to the double fault stack whenever a double fault occurs. Thus, we are able to catch _all_ double faults, including kernel stack overflows:

![QEMU printing `EXCEPTION: DOUBLE FAULT` and a dump of the exception stack frame](images/qemu-double-fault-on-stack-overflow.png)

From now on we should never see a triple fault again!

## What's next?
Now that we mastered exceptions, it's time to explore another kind of interrupts: interrupts from external devices such as timers, keyboards, or network controllers. These hardware interrupts are very similar to exceptions, e.g. they are also dispatched through the IDT.

However, unlike exceptions, they don't arise directly on the CPU. Instead, an _interrupt controller_ aggregates these interrupts and forwards them to CPU depending on their priority. In the next posts we will explore the two interrupt controller variants on x86: the [Intel 8259] \(“PIC”) and the [APIC]. This will allow us to react to keyboard and mouse input.

[Intel 8259]: https://en.wikipedia.org/wiki/Intel_8259
[APIC]: https://en.wikipedia.org/wiki/Advanced_Programmable_Interrupt_Controller
