+++
title = "Double Faults"
date = "2016-11-08"
+++

In this post we will make our kernel completely exception-proof by catching double faults on a separate kernel stack.

<!--more--><aside id="toc"></aside>

## What is a Double Fault?
In simplified terms, a double fault is a special exception that occurs when the CPU can't invoke an exception handler. For example, it occurs when a page fault is triggered but there is no page fault handler registered in the [IDT]. So it's kind of similar to catch-all blocks in programming languages with exceptions, e.g. `catch(...)` in C++ or `catch(Exception e)` in Java or C#.

[IDT]: {{% relref "09-catching-exceptions.md#the-interrupt-descriptor-table" %}}

A double fault behaves like a normal exception. It has the vector number `8` and we can define a normal handler function for it in the IDT. It is really important to provide a double fault handler, because if a double faults is unhandled a fatal _triple fault_ occurs. Triple faults can't be caught and most hardware reacts with a system reset.

### Triggering a Double Fault
Let's provoke a double fault by triggering an exception for that we didn't define a handler function yet:

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

So in order to prevent this triple fault, we need to either provide a handler function for `Debug` exceptions or a double fault handler. We will do the latter, since this post is all about the double fault.

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

Our handler prints a short error message and dumps the exception stack frame. The error code of the double fault handler is _always zero_, so there's no reason to print it.

When we start our kernel now, we should see that the double fault handler is invoked:

![QEMU printing `EXCEPTION: DOUBLE FAULT` and the exception stack frame](images/qemu-catch-double-fault.png)

It worked! Here is what happens this time:

1. The CPU executes the `int 1` instruction macro, which causes a software-invoked `Debug` exception.
2. Like before, the CPU looks at the corresponding entry in the IDT and sees that the present bit isn't set. Thus, it can't call the debug exception handler and a double fault occurs.
3. The CPU jumps to the – now present – double fault handler.

The triple fault (and the boot-loop) no longer occurs, since the CPU can now call the double fault handler.

That was pretty straightforward! So why do we need a whole post for this topic? Well, we're now able to catch _most_ double faults, but there are some cases where our current approach doesn't suffice.

## Causes of Double Faults
Before we look at the special cases, we need to know the exact causes of double faults. Above, we used a pretty vague definition:

> A double fault is a special exception that occurs when the CPU can't invoke an exception handler.

What does _“can't invoke”_ mean exactly? The handler is not present? The handler is [swapped out]? And what happens if a handler causes exceptions itself?

[swapped out]: http://pages.cs.wisc.edu/~remzi/OSTEP/vm-beyondphys.pdf

For example, what happens if… :

1. a divide-by-zero exception occurs, but the corresponding handler function is swapped out?
2. a page fault occurs, but the page fault handler is swapped out?
3. a divide-by-zero handler invokes a breakpoint exception, but the breakpoint handler is swapped out?
4. our kernel overflows its stack and the [guard page] is hit?

[guard page]: {{% relref "07-remap-the-kernel.md#creating-a-guard-page" %}}

Fortunately, the AMD64 manual ([PDF][AMD64 manual]) has an exact definition (in Section 8.2.9). According to it, a “double fault exception _can_ occur when a second exception occurs during the handling of a prior (first) exception handler”. The _“can”_ is important: Only very specific combinations of exceptions lead to a double fault. These combinations are:

First Exception | Second Exception
----------------|-----------------
divide-by-zero,<br>invalid-tss,<br>segment-not-present,<br>stack,<br>general-protection | invalid-tss,<br>segment-not-present,<br>stack,<br>general-protection
page fault | page fault,<br>invalid-tss,<br>segment-not-present,<br>stack,<br>general-protection

[AMD64 manual]: http://developer.amd.com/wordpress/media/2012/10/24593_APM_v21.pdf

So for example a divide-by-zero fault followed by a page fault is fine, but a divide-by-zero fault followed by a general-protection fault leads to a double fault. With the help of this table, we can answer the first three of the above questions:

1. When a divide-by-zero exception occurs and the corresponding handler function is swapped out, a _page fault_ occurs and the _page fault handler_ is invoked.
2. When a page fault occurs and the page fault handler is swapped out, a _double fault_ occurs and the _double fault_ handler is invoked.
3. When a divide-by-zero handler invokes a breakpoint exception and the breakpoint handler is swapped out, a _breakpoint exception_ occurs first. However, the corresponding handler is swapped out, so a _page fault_ occurs and the _page fault handler_ is invoked.

In fact, even the case of a non-present handler follows this scheme: A non-present handler causes a _segment-not-present_ exception. We didn't define a segment-not-present handler, so another segment-not-present exception occurs. According to the table, this leads to a double fault.

### Kernel Stack Overflow
Let's look at the fourth question:

> What happens if our kernel overflows its stack and the [guard page] is hit?

When our kernel overflows its stack and hits the guard page, a _page fault_ occurs and the CPU invokes the page fault handler. However, the CPU also tries to push the [exception stack frame] onto the stack. This fails of course, since our current stack pointer still points to the guard page. Thus, a second page fault occurs, which causes a double fault (according to the above table).

[exception stack frame]: http://os.phil-opp.com/better-exception-messages.html#exceptions-in-detail

So the CPU tries to call our _double fault handler_ now. However, on a double fault the CPU tries to push the exception stack frame, too. Thus, a _third_ page fault occurs, which causes a _triple fault_ and a system reboot. So our current double fault handler can't avoid a triple fault in this case.

Let's try it ourselves! We can easily provoke a kernel stack overflow by calling a function that recurses endlessly:

{{< highlight rust "hl_lines=9 10 11 14" >}}
// in src/lib.rs

#[no_mangle]
pub extern "C" fn rust_main(multiboot_information_address: usize) {
    ...
    // initialize our IDT
    interrupts::init();

    fn stack_overflow() {
        stack_overflow();
    }

    // trigger a stack overflow
    stack_overflow();

    println!("It did not crash!");
    loop {}
}
{{< / highlight >}}

When we try this code in QEMU, we see that the system enters a boot-loop again. Here is what happens: When the `stack_overflow` function is called, the whole stack gets filled with return addresses. At some point, we overflow the stack and hit the guard page, which we [set up][set up guard page] for exactly this case. Thus, a _page fault_ occurs.

Now the CPU pushes the exception stack frame and the registers and invokes the page fault handler… wait… this can't work. We overflowed our stack, so the stack pointer points to the guard page. And now the CPU tries to push to it, which causes another page fault. At this point, a double fault occurs, since an exception occurred while calling an exception handler.

So the CPU tries to invoke the double fault handler now. But first, it tries to push the exception stack frame, since exceptions on x86 work that way. Of course, this is still not possible (the stack pointer still points to the guard page), so another page fault occurs while calling the double fault handler. Thus, a triple fault occurs and QEMU issues a system reset.

So how can we avoid this problem? We can't omit the pushing of the exception stack frame, since it's the CPU itself that does it. So we need to ensure somehow that the stack is always valid when a double fault exception occurs. Fortunately, the x86_64 architecture has a trick for this problem.

## Switching Stacks
The x86_64 architecture is able to switch to a predefined stack when an exception occurs. However, it is a bit cumbersome to setup this mechanism.

The mechanism consists of two main components: An _Interrupt Stack Table_ and a _Task State Segment_.


Switching stacks
The Interrupt Stack Table
The Task State Segment
The Global Descriptor Table (again)
Putting it together
What’s next?

In the previous post, we learned how to return from exceptions correctly. In this post, we will explore a special type of exception: the double fault. The double fault occurs whenever the invokation of an exception handler fails. For example, if we didn't declare any exception hanlder in the IDT.

Let's start by creating a handler function for double faults:

```rust

```

Next, we need to register the double fault handler in our IDT:


Double faults also occur when an exception occurs while the CPU is trying to invoke an exception handler. For example, let's assume a divide-by-zero exception occurs but the OS accidentally [swapped out] the corresponding handler function. Now the CPU tries to call the divide-by-zero handler, which


A double fault occurs whenever the CPU fails to call an exception handler. On a high level it's like a catch-all handler, similar to `catch(...)` in C++ or `catch(Exception e)` in Java or C#.

The most common case is that there isn't a handler defined in the IDT. However, a double fault also occurs if the exception handler lies on a unaccessible page of if the CPU fails to push the exception stack frame.
