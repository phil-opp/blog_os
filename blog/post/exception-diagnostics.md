+++
title = "Exception Diagnostics"
date = "2016-06-15"
+++

In the [previous post], we've set up an interrupt descriptor table in order to catch divide by zero faults. In this post, we will explore exceptions in more detail. Our goal is to print additional information when an exception occurs, for example the values of the instruction and stack pointer at that time. We will also add handler functions for page and double faults.

[previous post]: {{% relref "2016-05-28-catching-exceptions.md" %}}

<!--more-->

As always, the complete source code is on [Github]. Please file [issues] for any problems, questions, or improvement suggestions. There is also a comment section at the end of this page.

[Github]: https://github.com/phil-opp/blog_os/tree/TODO
[issues]: https://github.com/phil-opp/blog_os/issues

## Exceptions in Detail
An exception signals that something is wrong with the current instruction. So an exception is always caused by a specific assembly instruction. When an exception occurs, the CPU interrupts its current work and starts an internal exception routine.

This routine involves reading the interrupt descriptor table and invoking the registered handler function. But first, the CPU pushes various information onto the stack, which describe the current state and provide information about the cause of the exception:

![exception stack frame](images/exception-stack-frame.svg)

The pushed information contain the instruction and stack pointer, the current CPU flags, and (for some exceptions) an error code, which gives information about the exceptions cause. Let's look at the fields in detail:

- First, the CPU aligns the stack pointer on a 16-byte boundary. This allows us to use some SSE instructions, which expect such an alignment.
- After that, the CPU pushes the stack segment descriptor (SS) and the old stack pointer (from before the alignment) onto the stack. This allows us to restore the previous stack pointer when we want to continue the interrupted program.
- Then the CPU pushes the contents of the RFLAGS register. This register contains various state information of the interrupted program. For example, it indicates if interrupts were enabled and whether the last executed instruction returned zero.
- Next the CPU pushes the instruction pointer and its code segment descriptor onto the stack. This tells us the address of the last executed instruction, which caused the exception.
- Finally, the CPU pushes an error code for some exceptions. This error code only exists for some exceptions such as page faults or general protection faults and provides additional information. For example, it tells us whether a page fault was caused by a read or a write request.

## Printing the Exception Stack Frame
Let's create a struct that represents the exception stack frame:

```rust
// in src/interrupts/mod.rs

#[derive(Debug)]
#[repr(C)]
struct ExceptionStackFrame {
    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}
```
The divide-by-zero fault pushes no error code, so we leave it out. Note that the stack grows downwards in memory, so we need to declare the fields in reverse order.

Now we need a way to find the memory address of this stack frame. When we look at the above graphic again, we see that the start address of the exception stack frame is the new stack pointer. So we just need to read the value of `rsp` at the very beginning of our handler function:

```rust
// in src/interrupts/mod.rs

extern "C" fn divide_by_zero_handler() -> ! {
    let stack_frame: *const ExceptionStackFrame;
    unsafe {
        asm!("mov $0, rsp" : "=r"(stack_frame) ::: "intel");
        print_error(format_args!("EXCEPTION: DIVIDE BY ZERO\n{:#?}",
            *stack_frame));
    };
    loop {}
}
```
We're using [inline assembly] here to load the value from the `rsp` register into `stack_frame`. The syntax is a bit strange, therefore a quick explanation:

[inline assembly]: https://doc.rust-lang.org/book/inline-assembly.html

- The asm! macro emits raw assembly instructions. This is the only way to read raw register values in Rust.
- We insert a single assembly instruction here: `mov $0, rsp`. It moves the value of `rsp` to some register (the `$0` is a placeholder which is filled by the compiler).
- The colons are separators. The `asm!` macro expects output operands after the first colon. We're specifying our `stack_frame` variable as a single output operand here. The `=r` tells the compiler that it should use any register for the first placeholder `$0`.
- We don't need any input operands or so-called [clobbers], so we leave the blocks after the second and third colon empty.
- The last block (after the 4th colon) specifies options. The `intel` option tells the compiler that our code is in Intel assembly syntax (instead of the default AT&T syntax).

[clobbers]: https://doc.rust-lang.org/book/inline-assembly.html#clobbers

So we're loading the value stack pointer to `stack_frame` at the very beginning of our function. Thus we have a pointer to the exception stack frame in that variable and are able to pretty-print its `Debug` formatting through the `{:#?}` argument.

### Testing it
Let's try it by executing `make run`:

![qemu printing an ExceptionStackFrame with strange values](images/qemu-print-stack-frame-try.png)

Those values look very wrong. The instruction pointer is definitely not 1 and the code segment should be `0x8`. So what's going on here?

It seems like we somehow got the pointer wrong. The exception stack frame graphic and our inline assembly seem correct, so something must be modifying `rsp` before we load it into `stack_frame`.

Let's see what's happening by looking at the disassembly of our function:

```
> objdump -d build/kernel-x86_64.bin | grep -A20 "divide_by_zero_handler"

 [...]
000000000010ced0 <_ZN7blog_os10interrupts22divide_by_zero_handler17h621c1e80480189e8E>:
 10ced0:	55                   	push   %rbp
 10ced1:	48 89 e5             	mov    %rsp,%rbp
 10ced4:	48 81 ec b0 00 00 00 	sub    $0xb0,%rsp
 10cedb:	48 8d 45 98          	lea    -0x68(%rbp),%rax
 10cedf:	48 b9 1d 1d 1d 1d 1d 	movabs $0x1d1d1d1d1d1d1d1d,%rcx
 10cee6:	1d 1d 1d
 10cee9:	48 89 4d 98          	mov    %rcx,-0x68(%rbp)
 10ceed:	48 89 4d f8          	mov    %rcx,-0x8(%rbp)
 10cef1:	48 89 e1             	mov    %rsp,%rcx
 10cef4:	48 89 4d f8          	mov    %rcx,-0x8(%rbp)
 10cef8:  ...
[...]
```
Our `divide_by_zero_handler` starts at address `0x10ced0`. Let's look at the instruction at address `0x10cef1`:

```
mov %rsp,%rcx
```
It's in AT&T syntax and contains `rcx` instead of our `$0` placeholder, but it is in fact our inline assembly instruction, which loads the stack pointer into the `stack_frame` variable. It moves `rsp` to `rcx` first, and then the next instruction at `0x10cef8` moves `rcx` to the variable on the stack.

We can clearly see the problem here: The compiler inserted various other instructions before our inline assembly. These instructions modify the stack pointer so that we don't read the original `rsp` value and get a wrong pointer. But why is the compiler doing this?

The reason is that we need some place on the stack to store things like variables. Therefore the compiler inserts a so-called function _prologue_ which prepares the stack and reserves space for all variables. In our case, the compiler subtracts from the stack to make room for i.a. our `stack_frame` variable. This prologue is the first thing in every function and comes before every other code. So in order to correctly load the exception frame pointer, we need some way to circumvent the automatic prologue generation.

### Naked Functions
Fortunately there is a way to disable the prologue: [naked functions]. A naked function has no prologue and immediately starts with the first instruction of its body. However, most Rust code requires the prologue. Therefore naked functions should only contain inline assembly.

[naked functions]: https://github.com/rust-lang/rfcs/blob/master/text/1201-naked-fns.md

A naked function looks like this:

```rust
#[naked]
extern "C" fn naked_function_example() {
    unsafe {
        asm!("mov rax, 0x42" :::: "intel");
    };
}
```
Naked functions are highly unstable, so we need to add `#![feature(naked_functions)]` to our `src/lib.rs`.

If you want to try it, insert it in `src/lib.rs` and call it from `rust_main`. When we inspect the disassembly, we see that the function prologue is missing:

```
> objdump -d build/kernel-x86_64.bin | grep -A5 "naked_function_example"
[...]
000000000010df90 <_ZN7blog_os22naked_function_example17ha9f733dfe42b595dE>:
  10df90:	48 c7 c0 2a 00 00 00 	mov    $0x42,%rax
  10df97:	c3                   	retq   
  10df98:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  10df9f:	00
```
It contains just the specified inline assembly and a return instruction (you can ignore the junk values after the return statement). So let's try to use a naked function to retrieve the exception frame pointer.

### A Naked Exception Handler
We can't use Rust code in naked functions, but we still want to use Rust in our exception handler. Therefore we split our handler function in two parts. A main exception handler in Rust and a small naked wrapper function, which just loads the exception frame pointer and then calls the main handler.

Our new two-stage exception handler looks like this:

```rust
#[naked]
extern "C" fn divide_by_zero_handler() -> ! {
    unsafe {
        asm!(/* load exception frame pointer and call main_handler */);
    }
    ::core::intrinsics::unreachable();

    extern "C" fn main_handler(stack_frame: *const ExceptionStackFrame) -> ! {
        unsafe {
            print_error(format_args!("EXCEPTION: DIVIDE BY ZERO\n{:#?}",
                *stack_frame));
        }
        loop {}
    }
}
```

TODO:

- unreachable
- pointer as argument
- inner function

-----

## Failure on real Hardware

- reproduce using `-enable-kvm`
- debugging using `loop {}` and gdb
- frame pointer and thus stack pointer alignment wrong
- requirements system v
- stack frame high level (xx bytes)
- hacky workaround (`push 0`)
- `extern "C" fn() -> !` not the correct handler function type
- assembly stub required to ensure correct stack alignment
- naked functions for handlers with and without error code (`push 0`, `call`)

## Exception Stack Frame
In order to read values such as the error code or the address of the interrupted instruction, we need to know how the CPU modifies the stack when an exception occurs:


When an exception occurs, the CPU:

1. Aligns the stack pointer on a 16-byte boundary.
2. Pushes the stack segment descriptor (SS) and the old stack pointer (from before the alignment) onto the stack. The SS value is padded to 8 bytes.
3. Pushes the 64-bit RFLAGS register onto the stack.
4. Pushes the previous CS register and RIP register onto the stack. The CS value is padded to 8 bytes.
5. If the interrupt vector number has an error code associated with it, pushes the error code onto the stack. The error code is padded with four bytes to form a quadword.
6. Loads the offset field from the gate descriptor into the target RIP. The interrupt handler begins execution when control is transferred to the instruction referenced by the new RIP.

```rust
#[repr(C)]
struct ExceptionStackFrame {
    stack_segment: u64,
    stack_pointer: u64,
    cpu_flags: u64,
    code_segment: u64,
    instruction_pointer: u64,
}
```

## What's next?
Now TODO. However, some page faults still cause a triple fault and a bootloop. For example, try the following code:

```rust
pub extern "C" fn rust_main(...) {
    ...
    interrupts::init();

    // provoke a kernel stack overflow, which hits the guard page
    fn recursive() {
        recursive();
    }
    recursive();

    println!("It did not crash!");
    loop {}
}
```

The next post will explore and fix this triple fault by creating a double fault handler. After that, we should never again experience a triple fault in our kernel.
