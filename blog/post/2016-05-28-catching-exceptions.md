+++
title = "Catching Exceptions"
date = "2016-05-28"
+++

In this post, we start exploring exceptions. We set up an interrupt descriptor table and add handler functions. At the end of this post, our kernel will be able to catch page faults.

<!--more-->

As always, the complete source code is on [Github]. Please file [issues] for any problems, questions, or improvement suggestions. There is also a comment section at the end of this page.

[Github]: https://github.com/phil-opp/blog_os/tree/catching_exceptions
[issues]: https://github.com/phil-opp/blog_os/issues

## Exceptions
An exception signals that something is wrong with the current instruction. For example, the CPU issues an exception if the current instruction tries to divide by 0. When an exception occurs, the CPU interrupts its current work and immediately calls a specific exception handler function, depending on the exception type.

We've already seen several types of exceptions in our kernel:

- **Invalid Opcode**: This exception occurs when the current instruction is invalid. For example, this exception occurred when we tried to use SSE instructions before enabling SSE. Without SSE, the CPU didn't know the `movups` and `movaps` instructions, so it throws an exception when it stumbles over them.
- **Page Fault**: A page fault occurs on illegal memory accesses. For example, if the current instruction tries to read from an unmapped page or tries to write to a read-only page.
- **Double Fault**: When an exception occurs, the CPU tries to call the corresponding handler function. If another exception exception occurs _while calling the exception handler_, the CPU raises a double fault exception. This exception also occurs when there is no handler function registered.
- **Triple Fault**: If another exception occurs when the CPU tries to call the double fault handler function, it issues a fatal _triple fault_. We can't catch or handle a triple fault. Most processors react by resetting themselves and rebooting the operating system. This causes the bootloops we experienced in the previous posts.

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
13‑14 | Descriptor Privilege Level (DPL)  | The minimal required privilege level required for calling this handler.
15    | Present                           |

Each exception has a predefined IDT index. For example the invalid opcode exception has table index 6 and the page fault exception has table index 14. Thus, the hardware can automatically load the corresponding IDT entry for each exception. The [Exception Table][exceptions] in the OSDev wiki shows the IDT indexes of all exceptions in the “Vector nr.” column.

When an exception occurs, the CPU roughly does the following:

1. Read the corresponding entry from the Interrupt Descriptor Table (IDT). For example, the CPU reads the 14-th entry when a page fault occurs.
2. Check if the entry is present. Raise a double fault if not.
3. Push some registers on the stack, including the instruction pointer and the [EFLAGS] register. (We will use these values in a future post.)
4. Disable interrupts if the entry is an interrupt gate (bit 40 not set).
5. Load the specified GDT selector into the CS segment.
6. Jump to the specified handler function.

[EFLAGS]: https://en.wikipedia.org/wiki/FLAGS_register

## Handling Exceptions
Let's try to catch and handle CPU exceptions. We start by creating a new `interrupts` module with an `idt` submodule:

``` rust
// in src/lib.rs
...
mod interrupts;
...
```
``` rust
// src/interrupts/mod.rs

mod idt;
```

Now we create types for the IDT and its entries:

```rust
// src/interrupts/idt.rs

use x86::segmentation::SegmentSelector;

pub struct Idt([Entry; 16]);

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Entry {
    pointer_low: u16,
    gdt_selector: SegmentSelector,
    options: EntryOptions,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}
```

The IDT is variable sized and can have up to 256 entries. We only need the first 16 entries in this post, so we define the table as `[Entry; 16]`. The remaining 240 handlers are treated as non-present by the CPU.

The `Entry` type is the translation of the above table to Rust. The `repr(C, packed)` attribute ensures that the compiler keeps the field ordering and does not add any padding between them. Instead of describing the `gdt_selector` as a plain `u16`, we use the `SegmentSelector` type of the `x86` crate. We also merge bits 32 to 47 into an `option` field, because Rust has no `u3` or `u1` type. The `EntryOptions` type is described below:

### Entry Options
The `EntryOptions` type has the following skeleton:

``` rust
#[derive(Debug, Clone, Copy)]
pub struct EntryOptions(u16);

impl EntryOptions {
    fn new() -> Self {...}

    fn set_present(&mut self, present: bool) {...}

    fn disable_interrupts(&mut self, disable: bool) {...}

    fn set_privilege_level(&mut self, dpl: u16) {...}

    fn set_stack_index(&mut self, index: u16) {...}
}
```

The implementations of these methods need to modify the correct bits of the `u16` without touching the other bits. For example, we would need the following bit-fiddling to set the stack index:

``` rust
self.0 = (self.0 & 0xfff8) | stack_index;
```

Or alternatively:

``` rust
self.0 = (self.0 & (!0b111)) | stack_index;
```

Or:

``` rust
self.0 = ((self.0 >> 3) << 3) | stack_index;
```

Well, none of these variants is really _readable_ and it's very easy to make mistakes somewhere. Therefore I created a `BitField` type with the following API:

``` rust
self.0.set_range(0..3, stack_index);
```

I think it is much more readable, since we abstracted away all bit-masking details. The `BitField` type is contained in the [bit_field] crate. (It's pretty new, so it might still contain bugs.) To add it as dependency, we run `cargo add bit_field` and add `extern crate bit_field;` to our `src/lib.rs`.

[bit_field]: TODO

Now we can use the crate to implement the methods of `EntryOptions`:

```rust
// in src/interrupts/idt.rs

use bit_field::BitField;

#[derive(Debug, Clone, Copy)]
pub struct EntryOptions(BitField<u16>);

impl EntryOptions {
    fn minimal() -> Self {
        let mut options = BitField::new(0);
        options.set_range(9..12, 0b111); // 'must-be-one' bits
        EntryOptions(options)
    }

    fn new() -> Self {
        let mut options = Self::minimal();
        options.set_present(true).disable_interrupts(true);
        options
    }

    fn set_present(&mut self, present: bool) -> &mut Self {
        self.0.set_bit(15, present);
        self
    }

    fn disable_interrupts(&mut self, disable: bool) -> &mut Self {
        self.0.set_bit(8, !disable);
        self
    }

    fn set_privilege_level(&mut self, dpl: u16) -> &mut Self {
        self.0.set_range(13..15, dpl);
        self
    }

    fn set_stack_index(&mut self, index: u16) -> &mut Self {
        self.0.set_range(0..3, index);
        self
    }
}
```
Note that the ranges are _exclusive_ the upper bound. The `minimal` function creates an `EntryOptions` type with only the “must-be-one” bits set. The `new` function, on the other hand, chooses reasonable defaults: It sets the present bit (why would you want to create a non-present entry?) and disables interrupts (normally we don't want that our exception handlers can be interrupted). By returning the self pointer from the `set_*` methods, we allow easy method chaining such as `options.set_present(true).disable_interrupts(true)`.

### Creating IDT Entries
Now we can add a function to create new IDT entries:

```rust
impl Entry {
    fn new(gdt_selector: SegmentSelector, handler: HandlerFunc) -> Self {
        let pointer = handler as u64;
        Entry {
            gdt_selector: gdt_selector,
            pointer_low: pointer as u16,
            pointer_middle: (pointer >> 16) as u16,
            pointer_high: (pointer >> 32) as u32,
            options: EntryOptions::new(),
            reserved: 0,
        }
    }
}
```
We take a GDT selector and a handler function as arguments and create a new IDT entry for it. The `HandlerFunc` type is described below. It is a function pointer that can be converted to an `u64`. We choose the lower 16 bits for `pointer_low`, the next 16 bits for `pointer_middle` and the remaining 32 bits for `pointer_high`. For the options field we choose our default options, i.e. present and disabled interrupts.

### The Handler Function Type

The `HandlerFunc` type is a type alias for a function type:

``` rust
type HandlerFunc = extern "C" fn() -> !;
```
It needs to be a function with a defined [calling convention], as it called directly by the hardware. The C calling convention is the de facto standard in OS development, so we're using it, too. The function takes no arguments, since the hardware doesn't supply any arguments when jumping to the handler function.

[calling convention]: https://en.wikipedia.org/wiki/Calling_convention

It is important that the function is [diverging], i.e. it must never return. The reason is that the hardware doesn't _call_ the handler functions, it just _jumps_ to them after pushing some values to the stack. So our stack might look different:

[diverging]: https://doc.rust-lang.org/book/functions.html#diverging-functions

![normal function return vs interrupt function return](/images/normal-vs-interrupt-function-return.svg)

If our handler function returned normally, it would try to pop the return address from the stack. But it might get some completely different value then. For example, the CPU pushes an error code for some exceptions. Bad things would happen if we interpreted this error code as return address and jumped to it. Therefore interrupt handler functions must diverge[^fn-must-diverge].

[^fn-must-diverge]: Another reason is that we overwrite the current register values by executing the handler function. Thus, the interrupted function looses its state and can't proceed anyway.

### IDT methods
Let's add a function to create new interrupt descriptor tables:

```rust
impl Idt {
    pub fn new() -> Idt {
        Idt([Entry::missing(); 16])
    }
}

impl Entry {
    fn missing() -> Self {
        Entry {
            gdt_selector: 0,
            pointer_low: 0,
            pointer_middle: 0,
            pointer_high: 0,
            options: EntryOptions::minimal(),
            reserved: 0,
        }
    }
}
```
The `missing` function creates a non-present Entry. We could choose any values for the pointer and GDT selector fields as long as the present bit is not set.

However, a table with non-present entries is not very useful. So we create a `set_handler` method to add new handler functions:

```rust
impl Idt {
    pub fn set_handler(&mut self, entry: u8, handler: HandlerFunc)
        -> &mut EntryOptions
    {
        self.0[entry as usize] = Entry::new(segmentation::cs(), handler);
        &mut self.0[entry as usize].options
    }
}
```
The method overwrites the specified entry with the given handler function. We use the `segmentation::cs`[^fn-segmentation-cs] function of the [x86 crate] to get the current code segment descriptor. There's no need for different kernel code segments in long mode, so the current `cs` value should be always the right choice.

[x86 crate]: https://github.com/gz/rust-x86
[^fn-segmentation-cs]: The `segmentation::cs` function was [added](https://github.com/gz/rust-x86/pull/12) in version 0.7.0, so you might need to update your `x86` version in your `Cargo.toml`.

By returning a mutual reference to the entry's options, we allow the caller to override the default settings. For example, the caller could add a non-present entry by executing: `idt.set_handler(11, handler_fn).set_present(false)`.

### Loading the IDT
Now we're able to create new interrupt descriptor tables with registered handler functions. We just need a way to load an IDT, so that the CPU uses it. The x86 architecture uses a special register to store the active IDT and its length. In order to load a new IDT we need to update this register through the [lidt] instruction.

[lidt]: http://x86.renejeschke.de/html/file_module_x86_id_156.html

The `lidt` instruction expects a pointer to a special data structure, which specifies the start address of the IDT and its length:


Type    | Name    | Description
--------|---------|-----------------------------------
u16     | Limit   | The maximum addressable byte in the table. Equal to the table size in bytes minus 1.
u64     | Offset  | Virtual start address of the table.

This structure is already contained [in the x86 crate], so we don't need to create it ourselves. The same is true for the [lidt function]. So we just need to put the pieces together to create a `load`  method:

[in the x86 crate]: http://gz.github.io/rust-x86/x86/dtables/struct.DescriptorTablePointer.html
[lidt function]: http://gz.github.io/rust-x86/x86/dtables/fn.lidt.html

```rust
impl Idt {
    pub fn load(&self) {
        use x86::dtables::{DescriptorTablePointer, lidt};
        use core::mem::size_of;

        let ptr = DescriptorTablePointer {
            base: self as *const _ as u64,
            limit: (size_of::<Self>() - 1) as u16,
        };

        unsafe { lidt(&ptr) };
    }
}
```
The method does not need to modify the IDT, so it takes `self` by immutable reference. We convert this reference to an u64 and calculate the table size using [mem::size_of]. The additional `-1` is needed because the limit field has to be the maximum addressable byte.

[mem::size_of]: https://doc.rust-lang.org/nightly/core/mem/fn.size_of.html

Then we pass a pointer to our `ptr` structure to the `lidt` function, which calls the `lidt` assembly instruction in order to reload the IDT register. We need an unsafe block here, because the `lidt` assumes that the specified handler addresses are valid.

### Safety
But can we really guarantee that handler addresses are always valid? Let's see:

- The `Idt::new` function creates a new table populated with non-present entries. There's no way to set these entries to present from outside of this module, so this function is fine.
- The `set_handler` method allows us to overwrite a specified entry and point it to some handler function. Rust's type system guarantees that function pointers are always valid (as long as no `unsafe` is involved), so this function is fine, too.

There are no other public functions in the `idt` module (except `load`), so it should be safe… right?

Wrong! Imagine the following scenario:

```rust
pub fn init() {
    load_idt();
    cause_page_fault();
}

fn load_idt() {
    let mut idt = idt::Idt::new();
    idt.set_handler(14, page_fault_handler);
    idt.load();
}

fn cause_page_fault() {
    let x = [1,2,3,4,5,6,7,8,9];
    unsafe{ *(0xdeadbeaf as *mut u64) = x[4] };
}
```
This won't work. If we're lucky, we get a triple fault and a boot loop. If we're unlucky, our kernel does strange things and fails at some completely unrelated place. So what's the problem here?

Well, we construct an IDT _on the stack_ and load it. It is perfectly valid until the end of the `load_idt` function. But as soon as the function returns, its stack frame can be reused by other functions. Thus, the IDT gets overwritten by the stack frame of the `cause_page_fault` function. So when the page fault occurs and the CPU tries to read the entry, it only sees some garbage values and issues a double fault, which escalates to a triple fault and a CPU reset.

Now imagine that the `cause_page_fault` function declared an array of pointers instead. If the present was coincidentally set, the CPU would jump to some random pointer and interpret random memory as code. This would be a clear violation of memory safety.

### Fixing the load method
So how do we fix it? We could make the load function itself `unsafe` and push the unsafety to the caller. However, there is a much better solution in this case. In order to see it, we formulate the requirement for the `load` method:

> The referenced IDT must be valid until a new IDT is loaded.

We can't know when the next IDT will be loaded. Maybe never. So in the worst case:

> The referenced IDT must be valid as long as our kernel runs.

This is exactly the definition of a [static lifetime]. So we can easily ensure that the IDT lives long enough by adding a `'static` requirement to the signature of the `load` function:

[static lifetime]: http://rustbyexample.com/scope/lifetime/static_lifetime.html

```rust
pub fn load(&'static self) {...}
//           ^^^^^^^ ensure that the IDT reference has the 'static lifetime
```

That's it! Now the Rust compiler ensures that the above error can't happen anymore:

```
error: `idt` does not live long enough
  --> src/interrupts/mod.rs:78:5
78 |>     idt.load();
   |>     ^^^
note: reference must be valid for the static lifetime...
note: ...but borrowed value is only valid for the block suffix following
          statement 0 at 75:34
  --> src/interrupts/mod.rs:75:35
75 |>     let mut idt = idt::Idt::new();
   |>                                   ^
```

### A static IDT
So a valid IDT needs to have the `'static` lifetime. We can either create a `static` IDT or [deliberately leak a Box][into_raw]. We will most likely only need a single IDT for the foreseeable future, so let's try the `static` approach:

[into_raw]: https://doc.rust-lang.org/nightly/alloc/boxed/struct.Box.html#method.into_raw

```rust
// in src/interrupts/mod.rs

static IDT: idt::Idt = {
    let mut idt = idt::Idt::new();

    idt.set_handler(14, page_fault_handler);

    idt
};

extern "C" fn page_fault_handler() -> ! {
    println!("EXCEPTION: PAGE FAULT");
    loop {}
}
```
We register a single handler function for a page fault (index 14). The handler function just prints an error message and enters a `loop`. However, it doesn't work this way:

```
error: calls in statics are limited to constant functions, struct and enum
       constructors [E0015]
...
error: blocks in statics are limited to items and tail expressions [E0016]
...
error: references in statics may only refer to immutable values [E0017]
...
```
The reason is that the Rust compiler is not able to evaluate the value of the `static` at compile time. Maybe it will work someday when `const` functions become more powerful. But until then, we have to find another solution.

### Lazy Statics to the Rescue
Fortunately the `lazy_static` macro exists. Instead of evaluating a `static` at compile time, the macro performs the initialization when the `static` is referenced the first time. Thus, we can do almost everything in the initialization block and are even able to read runtime values (e.g. the number of cores).

With `lazy_static`, we can define our IDT without problems:

```rust
lazy_static! {
  static ref IDT: idt::Idt = {
    let mut idt = idt::Idt::new();

    idt.set_handler(14, page_fault_handler);

    idt
  };
}
```

Now we're ready to load our IDT! Therefore we add a `interrupts::init` function:

```rust
// in src/interrupts/mod.rs

pub fn init() {
    IDT.load();
}
```
We don't need our `assert_has_not_been_called` macro here, since nothing bad happens when `init` is called twice. It just reloads the same IDT again.

## Testing it
Now we should be able to catch page faults! Let's try it in our `rust_main`:

```rust
// in src/lib.rs

pub extern "C" fn rust_main(...) {
    ...
    memory::init(boot_info);

    // initialize our IDT
    interrupts::init();

    // provoke a page fault by writing to some random address
    unsafe{ *(0xdeadbeaf as *mut u64) = 42 };

    println!("It did not crash!");
    loop {}
}
```
It works! We see a `EXCEPTION: PAGE FAULT` message at the bottom of our screen:

![QEMU screenshot with `EXCEPTION: PAGE FAULT` message](images/qemu-page-fault-println.png)

Let's try something else:

```rust
pub extern "C" fn rust_main(...) {
    ...
    interrupts::init();

    // provoke a page fault inside println
    println!("{:?}", unsafe{ *(0xdeadbeaf as *mut u64) = 42 });

    println!("It did not crash!");
    loop {}
}
```
Now the output ends on the `guard page` line. No `EXCEPTION` message and no `It did not crash` message either. What's happening?

### Debugging
Let's debug it using [GDB]. It is a console debugger and works with nearly everything, including QEMU. To make QEMU listen for a debugger connection, we start it with the `-s` flag:

[GDB]: https://www.gnu.org/software/gdb/

```Makefile
# in `Makefile`

run: $(iso)
	@qemu-system-x86_64 -cdrom $(iso) -s
```

Then we can launch GDB in another console window:

```
> gdb build/kernel-x86_64.bin
[some version, copyright, and usage information]
Reading symbols from build/kernel-x86_64.bin...done.
(gdb)
```
Now we can connect to our running QEMU instance on port `1234`:

```
(gdb) target extern :1234
Remote debugging using :1234
0x00000000001031dd in spin::mutex::cpu_relax ()
    at /home/.../spin-0.3.5/src/mutex.rs:102
102	    unsafe { asm!("pause" :::: "volatile"); }
```
So we're locked in a function named `mutex::cpu_relax` inside the `spin` crate. Let's try a backtrace:

```
(gdb) backtrace
#0  0x00000000001031dd in spin::mutex::cpu_relax ()
    at /home/.../spin-0.3.5/src/mutex.rs:102
#1  spin::mutex::{{impl}}::obtain_lock<blog_os::vga_buffer::Writer> (
    self=0x117220 <blog_os::vga_buffer::WRITER::h702c3f466147ac3b>)
    at /home/.../spin-0.3.5/src/mutex.rs:142
#2  0x0000000000103163 in spin::mutex::{{impl}}::lock<blog_os::vga_buffer::
    Writer> (
    self=0x117220 <blog_os::vga_buffer::WRITER::h702c3f466147ac3b>)
    at /home/.../spin-0.3.5/src/mutex.rs:163
#3  0x000000000010de09 in blog_os::interrupts::page_fault_handler ()
    at src/vga_buffer.rs:31
...
```
Pretty verbose… but very useful. Let's clean it up a bit:

- `spin::mutex::cpu_relax`
- `spin::mutex::obtain_lock<vga_buffer::Writer>`
- `spin::mutex::lock<vga_buffer::Writer>`
- `blog_os::interrupts::page_fault_handler`
- ...

It's a _back_-trace, so it goes from the innermost function to the outermost function. We see that our page fault handler was called successfully. It then tried to write its error message. Therefore, it tried to `lock` the static `WRITER`, which in turn called `obtain_lock` and `cpu_relax`.

So our kernel tries to lock the output `WRITER`, which is already locked by the interrupted `println`. Thus, our exception handler waits forever and we don't see what error occurred. Yay, that's our first deadlock! :)

(As you see, GDB can be very useful sometimes. For more GDB information check out our [Set Up GDB] page.)

[Set Up GDB]: {{% relref "set-up-gdb.md" %}}

## Printing Errors Reliably
In order to guarantee that we always see error messages, we add a `print_error` function to our `vga_buffer` module:

```rust
// in src/vga_buffer.rs

pub unsafe fn print_error(fmt: fmt::Arguments) {
    use core::fmt::Write;

    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Red, Color::Black),
        buffer: Unique::new(0xb8000 as *mut _),
    };
    writer.new_line();
    writer.write_fmt(fmt);
}
```

Instead of using the static `WRITER`, this function creates a new `Writer` on each invocation. Thereby it ignores the mutex and is always able to print to the screen without deadlocking. We print in red to highlight the error and add a newline to avoid overwriting unfinished lines.

### Safety
This function clearly violates the invariants of the `vga_buffer` module, as it creates another `Unique` pointing to `0xb8000`. Thus, we deliberately introduce a data race on the VGA buffer. For this reason, the function is marked as `unsafe` and should only be used if absolutely necessary.

However, the situation is not _that_ bad. The VGA buffer only stores characters (no pointers) and we never rely on the buffer's values. So the function might cause mangled output, but should never be able to violate memory safety.

### Using print_error
Let's use the new `print_error` function to print the page fault error:

```rust
// in src/interrupts/mod.rs

extern "C" fn page_fault_handler() -> ! {
    unsafe { print_error(format_args!("EXCEPTION: PAGE FAULT")) };
    loop {}
}
```
We use the built-in [format_args] macro to translate the error string to a `fmt::Arguments` type. Now we should always see the error message, even if the exception occurred inside `println`:

[format_args]: https://doc.rust-lang.org/nightly/std/macro.format_args!.html

![QEMU screenshot with new red `EXCEPTION: PAGE FAULT` message](images/qemu-page-fault-red.png)

## What's next?
Now we're able to catch _almost_ all page faults. However, some page faults still cause a triple fault and a bootloop. For example, try the following code:

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
