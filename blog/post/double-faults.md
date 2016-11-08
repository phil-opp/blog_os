+++
title = "Double Faults"
date = "2016-11-08"
+++

In this post we will make our kernel completely exception-proof by catching double faults on a separate kernel stack.

<!--more-->

## Triggering a Double Fault
A double fault occurs whenever the CPU fails to call the handler function for an exception. On a high level it's like a catch-all handler, similar to `catch(...)` in C++ or `catch(Exception e)` in Java or C#.

The most common case is that there isn't a handler defined in the IDT. However, a double fault also occurs if the exception handler lies on a unaccessible page of if the CPU fails to push the exception stack frame.

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

We use the [int! macro] of the [x86 crate] to trigger the exception with vector number `1`. The exception with that vector number is the [debug exception]. Like the [breakpoint exception], it is mainly used for debuggers. We haven't registered a handler function in our [IDT], so this line should cause a double fault in the CPU.

[int! macro]: https://docs.rs/x86/0.8.0/x86/macro.int!.html
[x86 crate]: https://github.com/gz/rust-x86
[debug exception]: http://wiki.osdev.org/Exceptions#Debug
[breakpoint exception]: http://wiki.osdev.org/Exceptions#Breakpoint

[IDT]: https://en.wikipedia.org/wiki/Interrupt_descriptor_table

When we start our kernel now, we see that it enters an endless loop:

![boot loop](images/boot-loop.gif)

The reason for the boot loop is the following:

1. The CPU executes the `int 1` instruction macro, which causes a software-invoked `Debug` exception.
2. The CPU looks at the corresponding entry in the IDT and sees that the present bit isn't set. Thus, it can't call the debug exception handler and a double fault occurs.
3. The CPU looks at the IDT entry of the double fault handler, but this entry is also non-present. Thus, a _triple_ fault occurs.
4. A triple fault is fatal. QEMU reacts to it like most real hardware and issues a system reset.

So in order to prevent this triple fault, we need to either provide a handler function for `Debug` exceptions or a double fault handler. We will do the latter, since this post is all about the double fault.

## A Double Fault Handler
A double fault is a normal exception with an error code, so we can use our `handler_with_error_code` macro to create a wrapper function:

{{< highlight rust "hl_lines=10 17 18 19 20 21 22" >}}
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

extern "C" fn double_fault_handler(stack_frame: &ExceptionStackFrame,
    _error_code: u64)
{
    println!("\nEXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
    loop {}
}
{{< / highlight >}}<!--end_-->

The error code of the double fault handler is always zero, so we don't print it.

When we start our kernel now, we should see that the double fault handler is invoked:

![QEMU printing `EXCEPTION: DOUBLE FAULT` and the exception stack frame](images/qemu-catch-double-fault.png)

It worked! Here is what happens this time:

1. The CPU executes the `int 1` instruction macro, which causes a software-invoked `Debug` exception.
2. The CPU looks at the corresponding entry in the IDT and sees that the present bit isn't set. Thus, it can't call the debug exception handler and a double fault occurs.
3. The CPU jumps to the – now present – double fault handler.

The triple fault (and the boot-loop) no longer occurs, since the CPU can now call the double fault handler.

That was pretty straightforward! So why do we need a whole post for this topic? Well, we're now able to catch _most_ double faults, but there are some edge cases where our current approach doesn't suffice.

## Stack Overflows
An example for such an edge case is a kernel stack a kernel stack overflow. We can easily provoke one through a function with endless recursion:

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

In the previous post, we learned how to return from exceptions correctly. In this post, we will explore a special type of exception: the double fault. The double fault occurs whenever the invokation of an excpption handler fails. For example, if we didn't declare any exception hanlder in the IDT.

Let's start by creating a handler function for double faults:

```rust

```

Next, we need to register the double fault handler in our IDT:
