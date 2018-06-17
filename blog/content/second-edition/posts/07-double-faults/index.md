+++
title = "Double Faults"
order = 7
path = "double-fault-exceptions"
date  = 2018-06-17
template = "second-edition/page.html"
+++

In this post we explore double faults in detail. We also set up an _Interrupt Stack Table_ to catch double faults on a separate kernel stack. This way, we can completely prevent triple faults, even on kernel stack overflow.

<!-- more -->

As always, the complete source code is available on [Github]. Please file [issues] for any problems, questions, or improvement suggestions. There is also a [gitter chat] and a comment section at the end of this page.

[Github]: https://github.com/phil-opp/blog_os/tree/post_10
[issues]: https://github.com/phil-opp/blog_os/issues
[gitter chat]: https://gitter.im/phil-opp/blog_os

## What is a Double Fault?
In simplified terms, a double fault is a special exception that occurs when the CPU fails to invoke an exception handler. For example, it occurs when a page fault is triggered but there is no page fault handler registered in the [Interrupt Descriptor Table][IDT] (IDT). So it's kind of similar to catch-all blocks in programming languages with exceptions, e.g. `catch(...)` in C++ or `catch(Exception e)` in Java or C#.

[IDT]: ./first-edition/posts/09-handling-exceptions/index.md#the-interrupt-descriptor-table

A double fault behaves like a normal exception. It has the vector number `8` and we can define a normal handler function for it in the IDT. It is really important to provide a double fault handler, because if a double fault is unhandled a fatal _triple fault_ occurs. Triple faults can't be caught and most hardware reacts with a system reset.

### Triggering a Double Fault
Let's provoke a double fault by triggering an exception for that we didn't define a handler function:

```rust
// in src/lib.rs

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    init_idt();

    // trigger a page fault
    unsafe {
        *(0xdeadbeaf as *mut u64) = 42;
    };

    println!("It did not crash!");
    loop {}
}
```

We try to write to address `0xdeadbeaf`, but the corresponding page is not present in the page tables. Thus, a page fault occurs. We haven't registered a page fault handler in our [IDT], so a double fault occurs.

When we start our kernel now, we see that it enters an endless boot loop. The reason for the boot loop is the following:

1. The CPU tries to write to `0xdeadbeaf`, which causes a page fault.
2. The CPU looks at the corresponding entry in the IDT and sees that the present bit isn't set. Thus, it can't call the page fault handler and a double fault occurs.
3. The CPU looks at the IDT entry of the double fault handler, but this entry is also non-present. Thus, a _triple_ fault occurs.
4. A triple fault is fatal. QEMU reacts to it like most real hardware and issues a system reset.

So in order to prevent this triple fault, we need to either provide a handler function for page faults or a double fault handler. We want to avoid triple faults in all cases, so let's start with a double fault handler that is invoked for all unhandled exception types.

## A Double Fault Handler
A double fault is a normal exception with an error code, so we can specify a handler function similar to our breakpoint handler:

```rust
// in src/main.rs

lazy_static! {
    static ref IDT: Idt = {
        let mut idt = Idt::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler); // new
        idt
    };
}

// new
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut ExceptionStackFrame, _error_code: u64)
{
    println!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
    loop {}
}
```

Our handler prints a short error message and dumps the exception stack frame. The error code of the double fault handler is always zero, so there's no reason to print it.

When we start our kernel now, we should see that the double fault handler is invoked:

![QEMU printing `EXCEPTION: DOUBLE FAULT` and the exception stack frame](qemu-catch-double-fault.png)

It worked! Here is what happens this time:

1. The CPU executes tries to write to `0xdeadbeaf`, which causes a page fault.
2. Like before, the CPU looks at the corresponding entry in the IDT and sees that the present bit isn't set. Thus, a double fault occurs.
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

[guard page]: ./first-edition/posts/07-remap-the-kernel/index.md#creating-a-guard-page

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

```rust
// in src/lib.rs

#[cfg(not(test))]
#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    init_idt();

    fn stack_overflow() {
        stack_overflow(); // for each recursion, the return address is pushed
    }

    // trigger a stack overflow
    stack_overflow();

    println!("It did not crash!");
    loop {}
}
```

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

[IDT entry]: ./first-edition/posts/09-handling-exceptions/index.md#the-interrupt-descriptor-table

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
Let's create a new TSS that contains a separate double fault stack in its interrupt stack table. For that we need a TSS struct. Fortunately, the `x86_64` crate already contains a [`TaskStateSegment` struct] that we can use:

[`TaskStateSegment` struct]: https://docs.rs/x86_64/0.2.3/x86_64/structures/tss/struct.TaskStateSegment.html

```rust
// in src/lib.rs

pub mod tss;

// in src/tss.rs

use x86_64::structures::tss::TaskStateSegment;

static DOUBLE_FAULT_STACK: [u8; 4096] = [0; 4096];
const DOUBLE_FAULT_IST_INDEX: usize = 0;

pub fn init() {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] = {
        let stack_start = &DOUBLE_FAULT_STACK as *const [u8; _] as usize;
        let stack_size = DOUBLE_FAULT_STACK.len();
        let stack_end = stack_start + stack_size;
        stack_end
    };
}
```

The TSS initialization code will be useful for our integration tests too, so we put it in our `src/lib.rs` instead of our `src/bin.rs`. To keep things organized, we create a new `tss` module for it.

We don't have implemented memory management yet, so we don't have a proper way to allocate a new stack. Instead, we use a `static` array as stack storage for now. We will replace this with a proper stack allocation in a later post.

We define that the 0th IST entry is the double fault stack (any other IST index would work too) and create a new TSS through the `TaskStateSegment::new` function. Then we write the top address of a the double fault stack to the 0th entry. We write the top address because stacks on x86 grow downwards, i.e. from high addresses to low addresses.

#### Loading the TSS
Now that we created a new TSS, we need a way to tell the CPU that it should use it. Unfortunately this is a bit cumbersome, since the TSS uses the segmentation system for historical reasons. So instead of loading the table directly, we need to add a new segment descriptor to the [Global Descriptor Table] \(GDT). Then we can load our TSS invoking the [`ltr` instruction] with the respective GDT index.

[Global Descriptor Table]: http://www.flingos.co.uk/docs/reference/Global-Descriptor-Table/
[`ltr` instruction]: http://x86.renejeschke.de/html/file_module_x86_id_163.html

### The Global Descriptor Table
The Global Descriptor Table (GDT) is a relict that was used for [memory segmentation] before paging became the de facto standard. It is still needed in 64-bit mode for various things such as kernel/user mode configuration or TSS loading.

[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation


TODO x86_64

We start by creating a new `interrupts::gdt` submodule. For that we need to rename the `src/interrupts.rs` file to `src/interrupts/mod.rs`. Then we can create a new submodule:

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
// in src/interrupts/gdt.rs

pub enum Descriptor {
    UserSegment(u64),
    SystemSegment(u64, u64),
}
```

The flag bits are common between all descriptor types, so we create a general `DescriptorFlags` type (using the [bitflags] macro):

[bitflags]: https://doc.rust-lang.org/bitflags/bitflags/macro.bitflags.html

```rust
// in src/interrupts/gdt.rs

bitflags! {
    struct DescriptorFlags: u64 {
        const CONFORMING        = 1 << 42;
        const EXECUTABLE        = 1 << 43;
        const USER_SEGMENT      = 1 << 44;
        const PRESENT           = 1 << 47;
        const LONG_MODE         = 1 << 53;
    }
}
```

We only add flags that are relevant in 64-bit mode. For example, we omit the read/write bit, since it is completely ignored by the CPU in 64-bit mode.

#### Code Segments
We add a function to create kernel mode code segments:

```rust
// in src/interrupts/gdt.rs

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
// in src/interrupts/gdt.rs

use x86_64::structures::tss::TaskStateSegment;

impl Descriptor {
    pub fn tss_segment(tss: &'static TaskStateSegment) -> Descriptor {
        use core::mem::size_of;
        use bit_field::BitField;

        let ptr = tss as *const _ as u64;

        let mut low = PRESENT.bits();
        // base
        low.set_bits(16..40, ptr.get_bits(0..24));
        low.set_bits(56..64, ptr.get_bits(24..32));
        // limit (the `-1` in needed since the bound is inclusive)
        low.set_bits(0..16, (size_of::<TaskStateSegment>() - 1) as u64);
        // type (0b1001 = available 64-bit tss)
        low.set_bits(40..44, 0b1001);

        let mut high = 0;
        high.set_bits(0..32, ptr.get_bits(32..64));

        Descriptor::SystemSegment(low, high)
    }
}
```

The `set_bits` and `get_bits` methods are provided by the [`BitField` trait] of the `bit_fields` crate. They allow us to easily get or set specific bits in an integer without using bit masks or shift operations. For example, we can do `x.set_bits(8..12, 42)` instead of `x = (x & 0xfffff0ff) | (42 << 8)`.

[`BitField` trait]: https://docs.rs/bit_field/0.6.0/bit_field/trait.BitField.html#method.get_bit

To link the `bit_fields` crate, we modify our `Cargo.toml` and our `src/lib.rs`:

```toml
[dependencies]
bit_field = "0.7.0"
```

```rust
extern crate bit_field;
```

We require the `'static` lifetime for the `TaskStateSegment` reference, since the hardware might access it on every interrupt as long as the OS runs.


#### Adding Descriptors to the GDT
In order to add descriptors to the GDT, we add a `add_entry` method:

```rust
// in src/interrupts/gdt.rs

use x86_64::structures::gdt::SegmentSelector;
use x86_64::PrivilegeLevel;

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
// in src/interrupts/gdt.rs

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
// in src/interrupts/gdt.rs

impl Gdt {
    pub fn load(&'static self) {
        use x86_64::instructions::tables::{DescriptorTablePointer, lgdt};
        use core::mem::size_of;

        let ptr = DescriptorTablePointer {
            base: self.table.as_ptr() as u64,
            limit: (self.table.len() * size_of::<u64>() - 1) as u16,
        };

        unsafe { lgdt(&ptr) };
    }
}
```
We use the [`DescriptorTablePointer` struct] and the [`lgdt` function] provided by the `x86_64` crate to load our GDT. Again, we require a `'static` reference since the GDT possibly needs to live for the rest of the run time.

[`DescriptorTablePointer` struct]: https://docs.rs/x86_64/0.2.3/x86_64/instructions/tables/struct.DescriptorTablePointer.html
[`lgdt` function]: https://docs.rs/x86_64/0.2.3/x86_64/instructions/tables/fn.lgdt.html

### Putting it together
We now have a double fault stack and are able to create and load a TSS (which contains an IST). So let's put everything together to catch kernel stack overflows.

We already created a new TSS in our `interrupts::init` function. Now we can load this TSS by creating a new GDT:

```rust
// in src/interrupts/mod.rs

pub fn init(memory_controller: &mut MemoryController) {
    let double_fault_stack = memory_controller.alloc_stack(1)
        .expect("could not allocate double fault stack");

    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] = VirtualAddress(
        double_fault_stack.top());

    let mut gdt = gdt::Gdt::new();
    let code_selector = gdt.add_entry(gdt::Descriptor::kernel_code_segment());
    let tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
    gdt.load();

    IDT.load();
}
```

However, when we try to compile it, the following errors occur:

```
error: `tss` does not live long enough
   --> src/interrupts/mod.rs:118:68
    |
118 |    let tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
    |                                         does not live long enough ^^^
...
122 | }
    | - borrowed value only lives until here
    |
    = note: borrowed value must be valid for the static lifetime...

error: `gdt` does not live long enough
   --> src/interrupts/mod.rs:119:5
    |
119 |    gdt.load();
    |    ^^^ does not live long enough
...
122 | }
    | - borrowed value only lives until here
    |
    = note: borrowed value must be valid for the static lifetime...
```
The problem is that we require that the TSS and GDT are valid for the rest of the run time (i.e. for the `'static` lifetime). But our created `tss` and `gdt` live on the stack and are thus destroyed at the end of the `init` function. So how do we fix this problem?

We could allocate our TSS and GDT on the heap using `Box` and use [into_raw] and a bit of `unsafe` to convert it to `&'static` references ([RFC 1233] was closed unfortunately).

Alternatively, we could store them in a `static` somehow. The [`lazy_static` macro] doesn't work here, since we need access to the `MemoryController` for initialization. However, we can use its fundamental building block, the [`spin::Once` type].

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

(The `Once` was added in spin 0.4, so you're probably need to update your spin dependency.)

So let's rewrite our `interrupts::init` function to use the static `TSS` and `GDT`:

```rust
pub fn init(memory_controller: &mut MemoryController) {
    let double_fault_stack = memory_controller.alloc_stack(1)
        .expect("could not allocate double fault stack");

    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] = VirtualAddress(
            double_fault_stack.top());
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
```

Now it should compile again!

#### The final Steps
We're almost done. We successfully loaded our new GDT, which contains a TSS descriptor. Now there are just a few steps left:

1. We changed our GDT, so we should reload the `cs`, the code segment register. This required since the old segment selector could point a different GDT descriptor now (e.g. a TSS descriptor).
2. We loaded a GDT that contains a TSS selector, but we still need to tell the CPU that it should use that TSS.
3. As soon as our TSS is loaded, the CPU has access to a valid interrupt stack table (IST). Then we can tell the CPU that it should use our new double fault stack by modifying our double fault IDT entry.

For the first two steps, we need access to the `code_selector` and `tss_selector` variables outside of the closure. We can achieve this by moving the `let` declarations out of the closure:

```rust
// in src/interrupts/mod.rs
pub fn init(memory_controller: &mut MemoryController) {
    use x86_64::structures::gdt::SegmentSelector;
    use x86_64::instructions::segmentation::set_cs;
    use x86_64::instructions::tables::load_tss;
    ...

    let mut code_selector = SegmentSelector(0);
    let mut tss_selector = SegmentSelector(0);
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
        load_tss(tss_selector);
    }

    IDT.load();
}
```

We first set the descriptors to `empty` and then update them from inside the closure (which implicitly borrows them as `&mut`). Now we're able to reload the code segment register using [`set_cs`] and to load the TSS using [`load_tss`].

[`set_cs`]: https://docs.rs/x86/0.8.0/x86/shared/segmentation/fn.set_cs.html
[`load_tss`]: https://docs.rs/x86/0.8.0/x86/shared/task/fn.load_tss.html

Now that we loaded a valid TSS and interrupt stack table, we can set the stack index for our double fault handler in the IDT:

```rust
// in src/interrupt/mod.rs

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();
        ...
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX as u16);
        }
        ...
    };
}
```

The `set_stack_index` method is unsafe because the the caller must ensure that the used index is valid and not already used for another exception.

That's it! Now the CPU should switch to the double fault stack whenever a double fault occurs. Thus, we are able to catch _all_ double faults, including kernel stack overflows:

![QEMU printing `EXCEPTION: DOUBLE FAULT` and a dump of the exception stack frame](qemu-double-fault-on-stack-overflow.png)

From now on we should never see a triple fault again!

## What's next?
The next posts will explain how to handle interrupts from external devices such as timers, keyboards, or network controllers. These hardware interrupts are very similar to exceptions, e.g. they are also dispatched through the IDT. However, unlike exceptions, they don't arise directly on the CPU. Instead, an _interrupt controller_ aggregates these interrupts and forwards them to CPU depending on their priority. In the next posts we will explore the two interrupt controller variants on x86: the [Intel 8259] \(“PIC”) and the [APIC]. This will allow us to react to keyboard and mouse input.

[Intel 8259]: https://en.wikipedia.org/wiki/Intel_8259
[APIC]: https://en.wikipedia.org/wiki/Advanced_Programmable_Interrupt_Controller


TODO update date