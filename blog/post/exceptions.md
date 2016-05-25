+++
title = "Catching Exceptions"
date = "2016-05-10"
+++

TODO We will catch page faults,

<!--more-->

## Interrupts
Whenever a device (e.g. the keyboard contoller) needs

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

Bits    | Name                              | Description
--------|-----------------------------------|-----------------------------------
0-15    | Function Pointer [0:15]           | The lower bits of the pointer to the handler function.
16-31   | GDT selector                      | Selector of a code segment in the GDT.
32-34   | Interrupt Stack Table Index       | 0: Don't switch stacks, 1-7: Switch to the n-th stack in the Interrupt Stack Table when this handler is called.
35-39   | Reserved (ignored)                |
40      | 0: Interrupt Gate, 1: Trap Gate   | If this bit is 0, interrupts are disabled when this handler is called.
41-43   | must be one                       |
44      | must be zero                      |
45-46   | Descriptor Privilege Level (DPL)  | The minimal required privilege level required for calling this handler.
47      | Present                           |
48-95   | Function Pointer [16:63]          | The remaining bits of the pointer to the handler function.
95-127  | Reserved (ignored)                |

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

    pub fn new() -> Self {
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
Note that the ranges are _exclusive_ the upper bound. The bit indexes are different from the values in the [above table], because the `option` field starts at bit 32. Thus e.g. the privilege level bits are bits 13 (`= 45‑32`) and 14 (`= 46‑32`).

The `minimal` function creates an `EntryOptions` type with only the “must-be-one” bits set. The `new` function, on the other hand, chooses reasonable defaults: It sets the present bit (why would you want to create a non-present entry?) and disables interrupts (normally we don't want that our exception handlers can be interrupted). By returning the self pointer from the `set_*` methods, we allow easy method chaining such as `options.set_present(true).disable_interrupts(true)`.

[above table]: {{% relref "#the-interrupt-descriptor-table" %}}

### Creating IDT Entries
Now we can add a function to create new IDT entries:

```rust
impl Entry {
    pub fn new(gdt_selector: SegmentSelector, handler: HandlerFunc) -> Self {
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

[^fn-must-diverge]: Another reason is that overwrite the current register values by executing the handler function. Thus, the interrupted function looses its state and can't proceed anyway.

### IDT methods
TODO

```rust
impl Idt {
    pub fn new() -> Idt {
        Idt([Entry::missing(); 16])
    }

    pub fn set_handler(&mut self, entry: u8, handler: extern "C" fn() -> !) {
        self.0[entry as usize] = Entry::new(segmentation::cs(), handler);
    }

    pub fn options(&mut self, entry: u8) -> &mut EntryOptions {
        &mut self.0[entry as usize].options
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

### A static IDT
TODO lazy_static etc

### Loading the IDT
TODO

### Testing it
TODO page fault, some other fault to trigger double fault, kernel stack overflow

## Switching stacks

### The Interrupt Stack Table

### The Task State Segment

### The Global Descriptor Table (again)

### Putting it together

## What's next?
