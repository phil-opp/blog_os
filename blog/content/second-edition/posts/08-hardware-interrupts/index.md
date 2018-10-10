+++
title = "Hardware Interrupts"
order = 8
path = "hardware-interrupts"
date = 2018-07-26
template = "second-edition/page.html"
+++

In this post we set up the programmable interrupt controller to correctly forward hardware interrupts to the CPU. To handle these interrups we add new entries to our interrupt descriptor table, just like we did for our exception handlers. We will learn how to get periodic timer interrupts and how to get input from the keyboard.

<!-- more -->

This blog is openly developed on [Github]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom].

[Github]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments

## Overview

Interrupts provide a way to notify the CPU from attached hardware devices. So instead of letting the kernel periodically check the keyboard for new characters (a process called [_polling_]), the keyboard can notify the kernel of each keypress. This is much more efficient because the kernel only needs to act when something happened. It also allows faster reaction times, because the kernel can react immediately and not only at the next poll.

[_polling_]: https://en.wikipedia.org/wiki/Polling_(computer_science)

Connecting all hardware devices directly to the CPU is not possible. Instead, a separate _interrupt controller_ aggregates the interrupts from all devices and then notifies the CPU:

```
                                    ____________             _____
               Timer ------------> |            |           |     |
               Keyboard ---------> | Interrupt  |---------> | CPU |
               Other Hardware ---> | Controller |           |_____|
               Etc. -------------> |____________|

```

Most interrupt controllers are programmable, which means that they support different priority levels for interrupts. For example, this allows to give timer interrupts a higher priority than keyboard interrupts to ensure accurate timekeeping.

Unlike exceptions, hardware interrupts occur _asynchronously_. This means that they are completely independent from the executed code and can occur at any time. Thus we suddenly have a form of concurrency in our kernel with all the potential concurrency-related bugs. Rust's strict ownership model helps us here because it forbids mutable global state. However, deadlocks are still possible, as we will see later in this post.

## The 8259 PIC

The [Intel 8259] is a programmable interrupt controller (PIC) introduced in 1976. It has long been replaced by the newer [APIC], but its interface is still supported on current systems for backwards compatibiliy reasons. The 8259 PIC is significantly easier to set up than the APIC, so we will use it to introduce ourselves to interrupts before we switch to the APIC in a later post.

[APIC]: https://en.wikipedia.org/wiki/Intel_APIC_Architecture

The 8259 has 8 interrupt lines and several lines for communicating with the CPU. The typical systems back then where equipped with two instances of the 8259 PIC, one acting as master and the other as slave connected to one of the masters interrupt lines:

[Intel 8259]: https://en.wikipedia.org/wiki/Intel_8259

```
                     ____________                          ____________
Real Time Clock --> |            |   Timer -------------> |            |
ACPI -------------> |            |   Keyboard-----------> |            |      _____
Available --------> | Slave      |----------------------> | Master     |     |     |
Available --------> | Interrupt  |   Serial Port 2 -----> | Interrupt  |---> | CPU |
Mouse ------------> | Controller |   Serial Port 1 -----> | Controller |     |_____|
Co-Processor -----> |            |   Parallel Port 2/3 -> |            |
Primary ATA ------> |            |   Floppy disk -------> |            |
Secondary ATA ----> |____________|   Parallel Port 1----> |____________|

```

This graphic shows the typical assignment of interrupt lines. We see that most of the 15 lines have a fixed mapping, e.g. line 4 of the slave PIC is assigned to the mouse.

Each controller can be configured through two [I/O ports], one “command” port and one “data” port. For the master controller these ports are `0x20` (command) and `0x21` (data). For the slave they are `0xa0` (command) and `0xa1` (data). For more information on how the PICs can be configured see the [article on osdev.org].

[I/O ports]: ./second-edition/posts/05-integration-tests/index.md#port-i-o
[article on osdev.org]: https://wiki.osdev.org/8259_PIC

### Implementation

The default configuration of the PICs is not usable, because it sends interrupt vector numbers in the range 0–15 to the CPU. These numbers are already occupied by CPU exceptions, for example number 8 corresponds to a double fault. To fix this overlapping issue, we need to remap the PIC interrupts to different numbers. The actual range doesn't matter as long as it does not overlap with the exceptions, but typically the range 32–47 is chosen, because these are the first free numbers after the 32 exception slots.

The configuration happens by writing special values to the command and data ports of the PICs. Fortunately there is already a crate called [`pic8259_simple`], so we don't need to write the initialization sequence ourselves. In case you are interested how it works, check out [its source code][pic crate source], it's fairly small and well documented.

[pic crate source]: https://docs.rs/crate/pic8259_simple/0.1.1/source/src/lib.rs

To add the crate as dependency, we add the following to our project:

[`pic8259_simple`]: https://docs.rs/pic8259_simple/0.1.1/pic8259_simple/

```toml
# in Cargo.toml

[dependencies]
pic8259_simple = "0.1.1"
```

```rust
// in src/lib.rs

extern crate pic8259_simple;
```

The main abstraction provided by the crate is the [`ChainedPics`] struct that represents the master/slave PIC layout we saw above. It is designed to be used in the following way:

[`ChainedPics`]: https://docs.rs/pic8259_simple/0.1.1/pic8259_simple/struct.ChainedPics.html

```rust
// in src/lib.rs

pub mod interrupts;

// in src/interrupts.rs

use pic8259::ChainedPics;
use spin;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });
```

We're setting the offsets for the pics to the range 32–47 as we noted above. By wrapping the `ChainedPics` struct in a `Mutex` we are able to get safe mutable access (through the [`lock` method][spin mutex lock]), which we need in the next step. The `ChainedPics::new` function is unsafe because wrong offsets could cause undefined behavior.

[spin mutex lock]: https://docs.rs/spin/0.4.8/spin/struct.Mutex.html#method.lock

We can now initialize the 8259 PIC from our `_start` function:

```rust
// in src/main.rs

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::gdt::init();
    init_idt();
    unsafe { PICS.lock().initialize() }; // new

    println!("It did not crash!");
    loop {}
}
```

We use the [`initialize`] function to perform the PIC initialization. Like the `ChainedPics::new` function, this function is also unsafe because it can cause undefined behavior if the PIC is misconfigured.

[`initialize`]: https://docs.rs/pic8259_simple/0.1.1/pic8259_simple/struct.ChainedPics.html#method.initialize

If all goes well we should continue to see the "It did not crash" message when executing `bootimage run`.

## Enabling Interrupts

Until now nothing happened because interrupts are still disabled in the CPU configuration. This means that the CPU does not listen to the interrupt controller at all, so no interrupts can reach the CPU. Let's change that:

```rust
// in src/main.rs

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::gdt::init();
    init_idt();
    unsafe { PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable(); // new

    println!("It did not crash!");
    loop {}
}
```

The `interrupts::enable` function of the `x86_64` crate executes the special `sti` instruction (“set interrupts”) to enable external interrupts. When we try `bootimage run` now, we see that a double fault occurs:

TODO screenshot

The reason for this double fault is that the hardware timer (the [Intel 8253] to be exact) is enabled by default, so we start receiving timer interrupts as soon as we enable interrupts. Since we didn't define a handler function for it yet, our double fault handler is invoked.

[Intel 8253]: https://en.wikipedia.org/wiki/Intel_8253

## Handling Timer Interrupts

As we see from the graphic [above](#the-8259-pic), the timer uses line 0 of the master PIC. This means that it arrives at the CPU as interrupt 32 (0 + offset 32). Therefore we need to add a handler for interrupt 32 if we want to handle the timer interrupt:

```rust
// in src/interrupts.rs

pub const TIMER_INTERRUPT_ID: u8 = PIC_1_OFFSET; // new

// in src/main.rs

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        let timer_interrupt_id = usize::from(interrupts::TIMER_INTERRUPT_ID); // new
        idt[timer_interrupt_id].set_handler_fn(timer_interrupt_handler); // new

        idt
    };
}

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: &mut ExceptionStackFrame)
{
    print!(".");
}
```

We introduce a `TIMER_INTERRUPT_ID` constant to keep things organized. Our `timer_interrupt_handler` has the same signature as our exception handlers, because the CPU reacts identically to exceptions and external interrupts (the only difference is that some exceptions push an error code). The [`InterruptDescriptorTable`] struct implements the [`IndexMut`] trait, so we can access individual entries through array indexing syntax.

[`InterruptDescriptorTable`]: https://docs.rs/x86_64/0.2.11/x86_64/structures/idt/struct.InterruptDescriptorTable.html
[`IndexMut`]: https://doc.rust-lang.org/core/ops/trait.IndexMut.html

In our timer interrupt handler, we print a dot to the screen. As the timer interrupt happens periodically, we would expect to see a dot appearing on each timer tick. However, when we run it we see that only a single dot is printed:

TODO screenshot

### End of Interrupt

The reason is that the PIC expects an explicit “end of interrupt” (EOI) signal from our interrupt handler. This signal tells the controller that the interrupt was processed and that the system is ready to receive the next interrupt. So the PIC thinks we're still busy processing the first timer interrupt and waits patiently for the EOI signal before sending the next one.

To send the EOI, we use our static `PICS` struct again:

```rust
// in src/main.rs

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: &mut ExceptionStackFrame)
{
    print!(".");
    unsafe { PICS.lock().notify_end_of_interrupt(interrupts::TIMER_INTERRUPT_ID) }
}
```

The `notify_end_of_interrupt` figures out wether the master or slave PIC sent the interrupt and then uses the `command` and `data` ports to send an EOI signal to respective controllers. If the slave PIC sent the interrupt both PICs need to be notified because the slave PIC is connected to an input line of the master PIC.

We need to be careful to use the correct interrupt vector number, otherwise we could accidentally delete an important unsent interrupt or cause our system to hang. This is the reason that the function is unsafe.

When we now execute `bootimage run` we see dots periodically appearing on the screen:

TODO screenshot gif
