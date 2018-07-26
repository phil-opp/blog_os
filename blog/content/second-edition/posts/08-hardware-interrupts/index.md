+++
title = "Hardware Interrupts"
order = 8
path = "hardware-interrupts"
date = 2018-07-26
template = "second-edition/page.html"
+++

In this post we set up the programmable interrupt controller to correctly forward hardware interrupts to the CPU. This allows us to create handler functions for these, which work in almost the same way as our exception handlers. We will then learn how to configure a hardware timer so that we get periodic interrupts and also how to add keyboard support.

<!-- more -->

This blog is openly developed on [Github]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom].

[Github]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments

## Overview

Interrupts provide a way to notify the CPU from attached hardware devices. So instead of letting the kernel periodically check the keyboard for new characters (a process called [_polling_]), the keyboard can notify the kernel of each keypress. This is much more efficient because the kernel only needs to act when something happened. It also allows faster reaction times, because the kernel can react immediately and not only at the next poll.

[_polling_]: https://en.wikipedia.org/wiki/Polling_(computer_science)


```
                                    ____________             _____
               Timer ------------> |            |           |     |
               Keyboard ---------> | Interrupt  | --------> | CPU |
               Other Hardware ---> | Controller |           |_____|
               Etc. -------------> |____________|

```

Connecting all hardware devices directly to the CPU is not possible. Instead, a separate _interrupt controller_ aggregates the interrupts from all devices and then notifies the CPU. Most interrupt controllers are programmable, which means that they support different priority levels for interrupts. For example, we could give timer interrupts a higher priority than keyboard interrupts to ensure accurate timekeeping.

Unlike exceptions, hardware interrupts occur asynchronously. This means that they are completely independent from the executed code and can occur at any time. Thus we suddenly have a form of concurrency in our kernel with all the potential concurrency-related bugs. Rust's strict ownership model helps us here because it forbids mutable global state. However, deadlocks are still possible, as we will see later in this post.

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
Available --------> | Interrupt  |   Serial Port 2 -----> | Interrupt  | --> | CPU |
Mouse ------------> | Controller |   Serial Port 1 -----> | Controller |     |_____|
Co-Processor -----> |            |   Parallel Port 2/3 -> |            |
Primary ATA ------> |            |   Floppy disk -------> |            |
Secondary ATA ----> |____________|   Parallel Port 1----> |____________|

```

This graphic shows the typical assignment of interrupt lines. We see that most of the 15 lines have a fixed mapping, e.g. line 4 of the slave PIC is assigned to the mouse.

Each controller can be configured through two I/O ports, one “command” port and one “data” port. For the master controller these ports are `0x20` (command) and `0x21` (data). For the slave they are `0xa0` (command) and `0xa1` (data). For more information on how they can be configured see the [article on osdev.org].

[article on osdev.org]: https://wiki.osdev.org/8259_PIC

### Implementation

The initial configuration of the PICs is not usable, because it sends interrupt vector numbers in the range 0–15 to the CPU. These numbers are already occupied by CPU exceptions, for example number 8 corresponds to a double fault. To fix this overlapping issue, we need to remap the PIC interrupts to different numbers. The actual range doesn't matter as long as it does not overlap with the exceptions, but typically the range 32–47 is chosen, because these are the first free numbers after the 32 exception slots.

The configuration happens by writing special values to the command and data ports that we mentioned above. Fortunately there is already a crate called [`pic8259_simple`], so we don't need to write the initialization sequence ourselves. In case you are interested how it works, check out [its source code][pic crate source], it's fairly small and well documented.

[pic crate source]: https://docs.rs/crate/pic8259_simple/0.1.1/source/src/lib.rs

To add the crate as dependency, we add the following to our project:

[`pic8259_simple`]: https://docs.rs/pic8259_simple/0.1.1/pic8259_simple/

```toml
# in Cargo.toml

[dependencies]
pic8259_simple = "0.1.1"
```

```rust
// in lib.rs

extern crate pic8259_simple;
```