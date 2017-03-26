+++
title = "Handling Exceptions"
date = "2017-03-26"
+++

In this post, we start exploring CPU exceptions. Exceptions occur in various erroneous situations, for example when accessing an invalid memory address or when dividing by zero. To catch them, we have to set up an _interrupt descriptor table_ that provides handler functions. At the end of this post, our kernel will be able to catch [breakpoint exceptions] and to resume normal execution afterwards.

[breakpoint exceptions]: http://wiki.osdev.org/Exceptions#Breakpoint

<!--more--><aside id="toc"></aside>

As always, the complete source code is available on [Github]. Please file [issues] for any problems, questions, or improvement suggestions. There is also a comment section at the end of this page.

[Github]: https://github.com/phil-opp/blog_os/tree/handling_exceptions
[issues]: https://github.com/phil-opp/blog_os/issues

## Exceptions
An exception signals that something is wrong with the current instruction. For example, the CPU issues an exception if the current instruction tries to divide by 0. When an exception occurs, the CPU interrupts its current work and immediately calls a specific exception handler function, depending on the exception type.

We've already seen several types of exceptions in our kernel:

- **Invalid Opcode**: This exception occurs when the current instruction is invalid. For example, this exception occurred when we tried to use SSE instructions before enabling SSE. Without SSE, the CPU didn't know the `movups` and `movaps` instructions, so it throws an exception when it stumbles over them.
- **Page Fault**: A page fault occurs on illegal memory accesses. For example, if the current instruction tries to read from an unmapped page or tries to write to a read-only page.
- **Double Fault**: When an exception occurs, the CPU tries to call the corresponding handler function. If another exception exception occurs _while calling the exception handler_, the CPU raises a double fault exception. This exception also occurs when there is no handler function registered for an exception.
- **Triple Fault**: If an exception occurs while the CPU tries to call the double fault handler function, it issues a fatal _triple fault_. We can't catch or handle a triple fault. Most processors react by resetting themselves and rebooting the operating system. This causes the bootloops we experienced in the previous posts.

For the full list of exceptions check out the [OSDev wiki][exceptions].

[exceptions]: http://wiki.osdev.org/Exceptions

### The Interrupt Descriptor Table
In order to catch and handle exceptions, we have to set up a so-called _Interrupt Descriptor Table_ (IDT). In this table we can specify a handler function for each CPU exception. The hardware uses this table directly, so we need to follow a predefined format. Each entry must have the following 16-byte structure:

Type| Name                     | Description
----|--------------------------|-----------------------------------
u16 | Function Pointer [0:15]  | The lower bits of the pointer to the handler function.
u16 | GDT selector             | Selector of a code segment in the GDT.
u16 | Options                  | (see below)
u16 | Function Pointer [16:31] | The middle bits of the pointer to the handler function.
u32 | Function Pointer [32:63] | The remaining bits of the pointer to the handler function.
u32 | Reserved                 |

The options field has the following format:

Bits  | Name                              | Description
------|-----------------------------------|-----------------------------------
0-2   | Interrupt Stack Table Index       | 0: Don't switch stacks, 1-7: Switch to the n-th stack in the Interrupt Stack Table when this handler is called.
3-7   | Reserved              |
8     | 0: Interrupt Gate, 1: Trap Gate   | If this bit is 0, interrupts are disabled when this handler is called.
9-11  | must be one                       |
12    | must be zero                      |
13‑14 | Descriptor Privilege Level (DPL)  | The minimal privilege level required for calling this handler.
15    | Present                           |

Each exception has a predefined IDT index. For example the invalid opcode exception has table index 6 and the page fault exception has table index 14. Thus, the hardware can automatically load the corresponding IDT entry for each exception. The [Exception Table][exceptions] in the OSDev wiki shows the IDT indexes of all exceptions in the “Vector nr.” column.

When an exception occurs, the CPU roughly does the following:

1. Push some registers on the stack, including the instruction pointer and the [RFLAGS] register. (We will use these values later in this post.)
2. Read the corresponding entry from the Interrupt Descriptor Table (IDT). For example, the CPU reads the 14-th entry when a page fault occurs.
3. Check if the entry is present. Raise a double fault if not.
4. Disable interrupts if the entry is an interrupt gate (bit 40 not set).
5. Load the specified GDT selector into the CS segment.
6. Jump to the specified handler function.

[RFLAGS]: https://en.wikipedia.org/wiki/FLAGS_register

## An IDT Type
Instead of creating our own IDT type, we will use the [`Idt` struct] of the `x86_64` crate, which looks like this:

[`Idt` struct]: https://docs.rs/x86_64/0.1.1/x86_64/structures/idt/struct.Idt.html

``` rust
#[repr(C)]
pub struct Idt {
    pub divide_by_zero: IdtEntry<HandlerFunc>,
    pub debug: IdtEntry<HandlerFunc>,
    pub non_maskable_interrupt: IdtEntry<HandlerFunc>,
    pub breakpoint: IdtEntry<HandlerFunc>,
    pub overflow: IdtEntry<HandlerFunc>,
    pub bound_range_exceeded: IdtEntry<HandlerFunc>,
    pub invalid_opcode: IdtEntry<HandlerFunc>,
    pub device_not_available: IdtEntry<HandlerFunc>,
    pub double_fault: IdtEntry<HandlerFuncWithErrCode>,
    pub invalid_tss: IdtEntry<HandlerFuncWithErrCode>,
    pub segment_not_present: IdtEntry<HandlerFuncWithErrCode>,
    pub stack_segment_fault: IdtEntry<HandlerFuncWithErrCode>,
    pub general_protection_fault: IdtEntry<HandlerFuncWithErrCode>,
    pub page_fault: IdtEntry<PageFaultHandlerFunc>,
    pub x87_floating_point: IdtEntry<HandlerFunc>,
    pub alignment_check: IdtEntry<HandlerFuncWithErrCode>,
    pub machine_check: IdtEntry<HandlerFunc>,
    pub simd_floating_point: IdtEntry<HandlerFunc>,
    pub virtualization: IdtEntry<HandlerFunc>,
    pub security_exception: IdtEntry<HandlerFuncWithErrCode>,
    pub interrupts: [IdtEntry<HandlerFunc>; 224],
    // some fields omitted
}
```

The fields have the type [`IdtEntry<F>`], which is a struct that represents the fields of an IDT entry (see the table above). The type parameter `F` defines the expected handler function type. We see that some entries require a [`HandlerFunc`] and some entries require a [`HandlerFuncWithErrCode`]. The page fault even has its own special type: [`PageFaultHandlerFunc`].

[`IdtEntry<F>`]: https://docs.rs/x86_64/0.1.1/x86_64/structures/idt/struct.IdtEntry.html
[`HandlerFunc`]: https://docs.rs/x86_64/0.1.1/x86_64/structures/idt/type.HandlerFunc.html
[`HandlerFuncWithErrCode`]: https://docs.rs/x86_64/0.1.1/x86_64/structures/idt/type.HandlerFuncWithErrCode.html
[`PageFaultHandlerFunc`]: https://docs.rs/x86_64/0.1.1/x86_64/structures/idt/type.PageFaultHandlerFunc.html

Let's look at the `HandlerFunc` type first:

```rust
type HandlerFunc = extern "x86-interrupt" fn(_: &mut ExceptionStackFrame);
```

It's a [type alias] for an `extern "x86-interrupt" fn` type. The `extern` keyword defines a function with a [foreign calling convention] and is often used to communicate with C code (`extern "C" fn`). But what is the `x86-interrupt` calling convention?

[type alias]: https://doc.rust-lang.org/book/type-aliases.html
[foreign calling convention]: https://doc.rust-lang.org/book/ffi.html#foreign-calling-conventions

## The Interrupt Calling Convention
Exceptions are quite similar to function calls: The CPU jumps to the first instruction of the called function and executes it. Afterwards, if the function is not diverging, the CPU jumps to the return address and continues the execution of the parent function.

However, there is a major difference between exceptions and function calls: A function call is invoked voluntary by a compiler inserted `call` instruction, while an exception might occur at _any_ instruction. In order to understand the consequences of this difference, we need to examine function calls in more detail.

[Calling conventions] specify the details of a function call. For example, they specify where function parameters are placed (e.g. in registers or on the stack) and how results are returned. On x86_64 Linux, the following rules apply for C functions (specified in the [System V ABI]):

[Calling conventions]: https://en.wikipedia.org/wiki/Calling_convention
[System V ABI]: http://refspecs.linuxbase.org/elf/x86-64-abi-0.99.pdf

- the first six integer arguments are passed in registers `rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9`
- additional arguments are passed on the stack
- results are returned in `rax` and `rdx`

Note that Rust does not follow the C ABI (in fact, [there isn't even a Rust ABI yet][rust abi]). So these rules apply only to functions declared as `extern "C" fn`.

[rust abi]: https://github.com/rust-lang/rfcs/issues/600

### Preserved and Scratch Registers
The calling convention divides the registers in two parts: _preserved_ and _scratch_ registers.

The values of _preserved_ registers must remain unchanged across function calls. So a called function (the _“callee”_) is only allowed to overwrite these registers if it restores their original values before returning. Therefore these registers are called _“callee-saved”_. A common pattern is to save these registers to the stack at the function's beginning and restore them just before returning.

In contrast, a called function is allowed to overwrite _scratch_ registers without restrictions. If the caller wants to preserve the value of a scratch register across a function call, it needs to backup and restore it before the function call (e.g. by pushing it to the stack). So the scratch registers are _caller-saved_.

On x86_64, the C calling convention specifies the following preserved and scratch registers:

preserved registers | scratch registers
---|---
`rbp`, `rbx`, `rsp`, `r12`, `r13`, `r14`, `r15` | `rax`, `rcx`, `rdx`, `rsi`, `rdi`, `r8`, `r9`, `r10`, `r11`
_callee-saved_ | _caller-saved_

The compiler knows these rules, so it generates the code accordingly. For example, most functions begin with a `push rbp`, which backups `rbp` on the stack (because it's a callee-saved register).

### Preserving all Registers
In contrast to function calls, exceptions can occur on _any_ instruction. In most cases we don't even know at compile time if the generated code will cause an exception. For example, the compiler can't know if an instruction causes a stack overflow or a page fault.

Since we don't know when an exception occurs, we can't backup any registers before. This means that we can't use a calling convention that relies on caller-saved registers for exception handlers. Instead, we need a calling convention means that preserves _all registers_. The `x86-interrupt` calling convention is such a calling convention, so it guarantees that all register values are restored to their original values on function return.

### The Exception Stack Frame
On a normal function call (using the `call` instruction), the CPU pushes the return address before jumping to the target function. On function return (using the `ret` instruction), the CPU pops this return address and jumps to it. So the stack frame of a normal function call looks like this:

![function stack frame](images/function-stack-frame.svg)

For exception and interrupt handlers, however, pushing a return address would not suffice, since interrupt handlers often run in a different context (stack pointer, CPU flags, etc.). Instead, the CPU performs the following steps when an interrupt occurs:

1. **Aligning the stack pointer**: An interrupt can occur at any instructions, so the stack pointer can have any value, too. However, some CPU instructions (e.g. some SSE instructions) require that the stack pointer is aligned on a 16 byte boundary, therefore the CPU performs such an alignment right after the interrupt.
2. **Switching stacks** (in some cases): A stack switch occurs when the CPU privilege level changes, for example when a CPU exception occurs in an user mode program. It is also possible to configure stack switches for specific interrupts using the so-called _Interrupt Stack Table_ (described in the next post).
3. **Pushing the old stack pointer**: The CPU pushes the values of the stack pointer (`rsp`) and the stack segment (`ss`) registers at the time when the interrupt occured (before the alignment). This makes it possible to restore the original stack pointer when returning from an interrupt handler.
4. **Pushing and updating the `RFLAGS` register**: The [`RFLAGS`] register contains various control and status bits. On interrupt entry, the CPU changes some bits and pushes the old value.
5. **Pushing the instruction pointer**: Before jumping to the interrupt handler function, the CPU pushes the instruction pointer (`rip`) and the code segment (`cs`). This is comparable to the return address push of a normal function call.
6. **Pushing an error code** (for some exceptions): For some specific exceptions such as page faults, the CPU pushes an error code, which describes the cause of the exception.
7. **Invoking the interrupt handler**: The CPU reads the address and the segment descriptor of the interrupt handler function from the corresponding field in the IDT. It then invokes this handler by loading the values into the `rip` and `cs` registers.

[`RFLAGS`]: https://en.wikipedia.org/wiki/FLAGS_register

So the _exception stack frame_ looks like this:

![exception stack frame](images/exception-stack-frame.svg)

In the `x86_64` crate, the exception stack frame is represented by the [`ExceptionStackFrame`] struct. It is passed to interrupt handlers as `&mut` and can be used to retrieve additional information about the exception's cause. The struct contains no error code field, since only some few exceptions push an error code. These exceptions use the separate [`HandlerFuncWithErrCode`] function type, which has an additional `error_code` argument.

[`ExceptionStackFrame`]: https://docs.rs/x86_64/0.1.1/x86_64/structures/idt/struct.ExceptionStackFrame.html

### Behind the Scenes
The `x86-interrupt` calling convention is a powerful abstraction that hides almost all of the messy details of the exception handling process. However, sometimes it's useful to know what's happening behind the curtain. Here is a short overview of the things that the `x86-interrupt` calling convention takes care of:

- **Retrieving the arguments**: Most calling conventions expect that the arguments are passed in registers. This is not possible for exception handlers, since we must not overwrite any register values before backing them up on the stack. Instead, the `x86-interrupt` calling convention is aware that the arguments already lie on the stack at a specific offset.
- **Returning using `iretq`**: Since the exception stack frame completely differs from stack frames of normal function calls, we can't return from handlers functions through the normal `ret` instruction. Instead, the `iretq` instruction must be used.
- **Handling the error code**: The error code, which is pushed for some exceptions, makes things much more complex. It changes the stack alignment (see the next point) and needs to be popped off the stack before returning. The `x86-64` handles all that complexity. However, it doesn't know which handler function is used for which exception, so it needs to deduce that information from the number of function arguments. That means that the programmer is still responsible to use the correct function type for each exception. Luckily, the `Idt` type defined by the `x86_64` crate ensures that the correct function types are used.
- **Aligning the stack**: There are some instructions (especially SSE instructions) that require a 16-byte stack alignment. The CPU ensures this alignment whenever an exception occurs, but for some exceptions it destroys it again later when it pushes an error code. The `x86-interrupt` calling convention takes care of this by realigning the stack in this case.

If you are interested in more details: We also have a series of posts that explains exception handling using [naked functions] linked [at the end of this post][too-much-magic].

[naked functions]: https://github.com/rust-lang/rfcs/blob/master/text/1201-naked-fns.md
[too-much-magic]: {{% relref "#too-much-magic" %}}

## Implementation
Now that we've understood the theory, it's time to handle CPU exceptions in our kernel. We start by creating a new `interrupts` module:

``` rust
// in src/lib.rs
...
mod interrupts;
...
```

In the new module, we create an `init` function, that creates a new `Idt`:

``` rust
// in src/interrupts.rs

use x86_64::structures::idt::Idt;

pub fn init() {
    let mut idt = Idt::new();
}
```

Now we can add handler functions. We start by adding a handler for the [breakpoint exception]. The breakpoint exception is the perfect exception to test exception handling. Its only purpose is to temporary pause a program when the breakpoint instruction `int3` is executed.

[breakpoint exception]: http://wiki.osdev.org/Exceptions#Breakpoint

The breakpoint exception is commonly used in debuggers: When the user sets a breakpoint, the debugger overwrites the corresponding instruction with the `int3` instruction so that the CPU throws the breakpoint exception when it reaches that line. When the user wants to continue the program, the debugger replaces the `int3` instruction with the original instruction again and continues the program. For more details, see the ["_How debuggers work_"] series.

["_How debuggers work_"]: http://eli.thegreenplace.net/2011/01/27/how-debuggers-work-part-2-breakpoints

For our use case, we don't need to overwrite any instructions (it wouldn't even be possible since we [set the page table flags] to read-only). Instead, we just want to print a message when the breakpoint instruction is executed and then continue the program.

[set the page table flags]: {{% relref "07-remap-the-kernel.md#using-the-correct-flags" %}}

So let's create a simple `breakpoint_handler` function and add it to our IDT:

```rust
/// in src/interrupts.rs

use x86_64::structures::idt::ExceptionStackFrame;

pub fn init() {
    let mut idt = Idt::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: &mut ExceptionStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}
```

Our handler just outputs a message and pretty-prints the exception stack frame.

### Loading the IDT
In order that the CPU uses our new interrupt descriptor table, we need to load it using the [`lidt`] instruction. The `Idt` struct of the `x86_64` provides a [`load`][Idt::load] method function for that. Let's try to use it:

[`lidt`]: http://x86.renejeschke.de/html/file_module_x86_id_156.html
[Idt::load]: https://docs.rs/x86_64/0.1.1/x86_64/structures/idt/struct.Idt.html#method.load

```rust
pub fn init() {
    let mut idt = Idt::new();
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

In fact, this is exactly what happens here. Our `idt` is created on the stack, so it is only valid inside the `init` function. Afterwards the stack memory is reused for other functions, so the CPU would interpret random stack memory as IDT. Luckily, the `Idt::load` method encodes this lifetime requirement in its function definition, so that the Rust compiler is able to prevent this possible bug at compile time.

In order to fix this problem, we need to store our `idt` at a place where it has a `'static` lifetime. To achieve this, we could either allocate our IDT on the heap using `Box` and then convert it to a `'static` reference or we can store the IDT as a `static`. Let's try the latter:

```rust
static IDT: Idt = Idt::new();

pub fn init() {
    IDT.breakpoint.set_handler_fn(breakpoint_handler);
    IDT.load();
}
```

There are two problems with this. First, statics are immutable, so we can't modify the breakpoint entry from our `init` function. Second, the `Idt::new` function is not a [`const` function], so it can't be used to initialize a `static`. We could solve this problem by using a [`static mut`] of type `Option<Idt>`:

[`const` function]: https://github.com/rust-lang/rfcs/blob/master/text/0911-const-fn.md
[`static mut`]: https://doc.rust-lang.org/book/const-and-static.html#mutability

```rust
static mut IDT: Option<Idt> = None;

pub fn init() {
    unsafe {
        let IDT = Some(Idt::new());
        let idt = IDT.as_mut_ref().unwrap();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.load();
    }
}
```

This variant compiles without errors but it's far from idiomatic. `static mut`s are very prone to data races, so we need an [`unsafe` block] on each access. Also, we need to explicitly `unwrap` the `IDT` on each use, since might be `None`.

[`unsafe` block]: https://doc.rust-lang.org/book/unsafe.html#unsafe-superpowers

#### Lazy Statics to the Rescue
The one-time initialization of statics with non-const functions is a common problem in Rust. Fortunately, there already exists a good solution in a crate named [lazy_static]. This crate provides a `lazy_static!` macro that defines a lazily initialized `static`. Instead of computing its value at compile time, the `static` laziliy initializes itself when it's accessed the first time. Thus, the initialization happens at runtime so that arbitrarily complex initialization code is possible.

[lazy_static]: https://docs.rs/lazy_static/0.2.4/lazy_static/

Let's add the `lazy_static` crate to our project:

```rust
// in src/lib.rs

#[macro_use]
extern crate lazy_static;
```

```toml
# in Cargo.toml

[dependencies.lazy_static]
version = "0.2.4"
features = ["spin_no_std"]
```
We need the `spin_no_std` feature, since we don't link the standard library. We also need the `#[macro_use]` attribute on the `extern crate` line to import the `lazy_static!` macro.

Now we can create our static IDT using `lazy_static`:

```rust
lazy_static! {
    static ref IDT: Idt = {
        let mut idt = Idt::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    };
}

pub fn init() {
    IDT.load();
}
```

Note how this solution requires no `unsafe` blocks or `unwrap` calls.

> ##### Aside: How does the `lazy_static!` macro work?
>
> The macro generates a `static` of type `Once<Idt>`. The [`Once`][spin::Once] type is provided by the `spin` crate and allows deferred one-time initialization. It is implemented using an [`AtomicUsize`] for synchronization and an [`UnsafeCell`] for storing the (possibly unitialized) value. So this solution also uses `unsafe` behind the scenes, but it is abstracted away in a safe interface.

[spin::Once]: https://docs.rs/spin/0.4.5/spin/struct.Once.html
[`AtomicUsize`]: https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicUsize.html
[`UnsafeCell`]: https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html

### Testing it
Now we should be able to handle breakpoint exceptions! Let's try it in our `rust_main`:

```rust
// in src/lib.rs

pub extern "C" fn rust_main(...) {
    ...
    memory::init(boot_info);

    // initialize our IDT
    interrupts::init();

    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();

    println!("It did not crash!");
    loop {}
}
```

When we run it in QEMU now (using `make run`), we see the following:

![QEMU printing `EXCEPTION: BREAKPOINT` and the exception stack frame](images/qemu-breakpoint-exception.png)

It works! The CPU successfully invokes our breakpoint handler, which prints the message, and then returns back to the `rust_main` function, where the `It did not crash!` message is printed.

> **Aside**: If it doesn't work and a boot loop occurs, this might be caused by a kernel stack overflow. Try increasing the stack size to at least 16kB (4096 * 4 bytes) in the `boot.asm` file.

We see that the exception stack frame tells us the instruction and stack pointers at the time when the exception occured. This information is very useful when debugging unexpected exceptions. For example, we can look at the corresponding assembly line using `objdump`:

```
> objdump -d build/kernel-x86_64.bin | grep -B5 "1140a6:"
00000000001140a0 <x86_64::instructions::interrupts::int3::h015bf61815bb8afe>:
  1140a0:	55                   	push   %rbp
  1140a1:	48 89 e5             	mov    %rsp,%rbp
  1140a4:	50                   	push   %rax
  1140a5:	cc                   	int3
  1140a6:	48 83 c4 08          	add    $0x8,%rsp
```

The `-d` flags disassembles the `code` section and `-C` flag makes function names more readable by [demangling] them. The `-B` flag of `grep` specifies the number of preceding lines that should be shown (5 in our case).

[demangling]: https://en.wikipedia.org/wiki/Name_mangling

We clearly see the `int3` exception that caused the breakpoint exception at address `1140a5`. Wait… the stored instruction pointer was `1140a6`, which is a normal `add` operation. What's happening here?

### Faults, Aborts, and Traps
The answer is that the stored instruction pointer only points to the causing instruction for _fault_ type exceptions, but not for _trap_ or _abort_ type exceptions. The difference between these types is the following:

- **Faults** are exceptions that can be corrected so that the program can continue as if nothing happened. An example is the [page fault], which can often be resolved by loading the accessed page from the disk into memory.
- **Aborts** are fatal exceptions that can't be recovered. Examples are [machine check exception] or the [double fault].
- **Traps** are only reported to the kernel, but don't hinder the continuation of the program. Examples are the breakpoint exception and the [overflow exception].

[page fault]: http://wiki.osdev.org/Exceptions#Page_Fault
[machine check exception]: http://wiki.osdev.org/Exceptions#Machine_Check
[double fault]: http://wiki.osdev.org/Exceptions#Double_Fault
[overflow exception]: http://wiki.osdev.org/Exceptions#Overflow

The reason for the diffent instruction pointer values is that the stored value is also the return address. So for faults, the instruction that caused the exception is restarted and might cause the same exception again if it's not resolved. This would not make much sense for traps, since invoking the breakpoint exception again would just cause another breakpoint exception[^fn-breakpoint-restart-use-cases]. Thus the instruction pointer points to the _next_ instruction for these exceptions.

[^fn-breakpoint-restart-use-cases]: There are valid use cases for restarting an instruction that caused a breakpoint. The most common use case is a debugger: When setting a breakpoint on some code line, the debugger overwrites the corresponding instruction with an `int3` instruction, so that the CPU traps when that line is executed. When the user continues execution, the debugger swaps in the original instruction and continues the program from the replaced instruction.

In some cases, the distinction between faults and traps is vague. For example, the [debug exception] behaves like a fault in some cases, but like a trap in others. So to find out the meaning of the saved instruction pointer, it is a good idea to read the official documentation for the exception, which can be found in the [AMD64 manual] in Section 8.2. For example, for the breakpoint exception it says:

[debug exception]: http://wiki.osdev.org/Exceptions#Debug
[AMD64 manual]: http://developer.amd.com/wordpress/media/2012/10/24593_APM_v21.pdf

> `#BP` is a trap-type exception. The saved instruction pointer points to the byte after the `INT3` instruction.

The documentation of the [`Idt`] struct and the [OSDev Wiki][osdev wiki exceptions] also contain this information.

[`Idt`]: https://docs.rs/x86_64/0.1.1/x86_64/structures/idt/struct.Idt.html
[osdev wiki exceptions]: http://wiki.osdev.org/Exceptions

## Too much Magic?
The `x86-interrupt` calling convention and the [`Idt`] type made the exception handling process relatively straightforward and painless. If this was too much magic for you and you like to learn all the gory details of exception handling, we got you covered: Our [“Handling Exceptions with Naked Functions”] series shows how to handle exceptions without the `x86-interrupt` calling convention and also creates its own `Idt` type. Historically, these posts were the main exception handling posts before the `x86-interrupt` calling convention and the `x86_64` crate existed.

[“Handling Exceptions with Naked Functions”]: {{% relref "handling-exceptions-with-naked-fns.html" %}}

## What's next?
We've successfully caught our first exception and returned from it! The next step is to add handlers for other common exceptions such as page faults. We also need to make sure that we never cause a [triple fault], since it causes a complete system reset. The next post explains how we can avoid this by correctly catching [double faults].

[triple fault]: http://wiki.osdev.org/Triple_Fault
[double faults]: http://wiki.osdev.org/Double_Fault#Double_Fault
