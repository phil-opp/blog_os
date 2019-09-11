+++
title = "Hardware Interrupts"
weight = 7
path = "hardware-interrupts"
date = 2018-10-22

+++

In this post we set up the programmable interrupt controller to correctly forward hardware interrupts to the CPU. To handle these interrupts we add new entries to our interrupt descriptor table, just like we did for our exception handlers. We will learn how to get periodic timer interrupts and how to get input from the keyboard.

<!-- more -->

This blog is openly developed on [GitHub]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom].  The complete source code for this post can be found in the [`post-07`][post branch] branch.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-07

<!-- toc -->

## Overview

Interrupts provide a way to notify the CPU from attached hardware devices. So instead of letting the kernel periodically check the keyboard for new characters (a process called [_polling_]), the keyboard can notify the kernel of each keypress. This is much more efficient because the kernel only needs to act when something happened. It also allows faster reaction times, since the kernel can react immediately and not only at the next poll.

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

The [Intel 8259] is a programmable interrupt controller (PIC) introduced in 1976. It has long been replaced by the newer [APIC], but its interface is still supported on current systems for backwards compatibility reasons. The 8259 PIC is significantly easier to set up than the APIC, so we will use it to introduce ourselves to interrupts before we switch to the APIC in a later post.

[APIC]: https://en.wikipedia.org/wiki/Intel_APIC_Architecture

The 8259 has 8 interrupt lines and several lines for communicating with the CPU. The typical systems back then were equipped with two instances of the 8259 PIC, one primary and one secondary PIC connected to one of the interrupt lines of the primary:

[Intel 8259]: https://en.wikipedia.org/wiki/Intel_8259

```
                     ____________                          ____________
Real Time Clock --> |            |   Timer -------------> |            |
ACPI -------------> |            |   Keyboard-----------> |            |      _____
Available --------> | Secondary  |----------------------> | Primary    |     |     |
Available --------> | Interrupt  |   Serial Port 2 -----> | Interrupt  |---> | CPU |
Mouse ------------> | Controller |   Serial Port 1 -----> | Controller |     |_____|
Co-Processor -----> |            |   Parallel Port 2/3 -> |            |
Primary ATA ------> |            |   Floppy disk -------> |            |
Secondary ATA ----> |____________|   Parallel Port 1----> |____________|

```

This graphic shows the typical assignment of interrupt lines. We see that most of the 15 lines have a fixed mapping, e.g. line 4 of the secondary PIC is assigned to the mouse.

Each controller can be configured through two [I/O ports], one “command” port and one “data” port. For the primary controller these ports are `0x20` (command) and `0x21` (data). For the secondary controller they are `0xa0` (command) and `0xa1` (data). For more information on how the PICs can be configured see the [article on osdev.org].

[I/O ports]: ./second-edition/posts/04-testing/index.md#i-o-ports
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

The main abstraction provided by the crate is the [`ChainedPics`] struct that represents the primary/secondary PIC layout we saw above. It is designed to be used in the following way:

[`ChainedPics`]: https://docs.rs/pic8259_simple/0.1.1/pic8259_simple/struct.ChainedPics.html

```rust
// in src/interrupts.rs

use pic8259_simple::ChainedPics;
use spin;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });
```

We're setting the offsets for the pics to the range 32–47 as we noted above. By wrapping the `ChainedPics` struct in a `Mutex` we are able to get safe mutable access (through the [`lock` method][spin mutex lock]), which we need in the next step. The `ChainedPics::new` function is unsafe because wrong offsets could cause undefined behavior.

[spin mutex lock]: https://docs.rs/spin/0.4.8/spin/struct.Mutex.html#method.lock

We can now initialize the 8259 PIC in our `init` function:

```rust
// in src/lib.rs

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() }; // new
}
```

We use the [`initialize`] function to perform the PIC initialization. Like the `ChainedPics::new` function, this function is also unsafe because it can cause undefined behavior if the PIC is misconfigured.

[`initialize`]: https://docs.rs/pic8259_simple/0.1.1/pic8259_simple/struct.ChainedPics.html#method.initialize

If all goes well we should continue to see the "It did not crash" message when executing `cargo xrun`.

## Enabling Interrupts

Until now nothing happened because interrupts are still disabled in the CPU configuration. This means that the CPU does not listen to the interrupt controller at all, so no interrupts can reach the CPU. Let's change that:

```rust
// in src/lib.rs

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();     // new
}
```

The `interrupts::enable` function of the `x86_64` crate executes the special `sti` instruction (“set interrupts”) to enable external interrupts. When we try `cargo xrun` now, we see that a double fault occurs:

![QEMU printing `EXCEPTION: DOUBLE FAULT` because of hardware timer](qemu-hardware-timer-double-fault.png)

The reason for this double fault is that the hardware timer (the [Intel 8253] to be exact) is enabled by default, so we start receiving timer interrupts as soon as we enable interrupts. Since we didn't define a handler function for it yet, our double fault handler is invoked.

[Intel 8253]: https://en.wikipedia.org/wiki/Intel_8253

## Handling Timer Interrupts

As we see from the graphic [above](#the-8259-pic), the timer uses line 0 of the primary PIC. This means that it arrives at the CPU as interrupt 32 (0 + offset 32). Instead of hardcoding index 32, we store it in an `InterruptIndex` enum:

```rust
// in src/interrupts.rs

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}
```

The enum is a [C-like enum] so that we can directly specify the index for each variant. The `repr(u8)` attribute specifies that each variant is represented as an `u8`. We will add more variants for other interrupts in the future.

[C-like enum]: https://doc.rust-lang.org/reference/items/enumerations.html#custom-discriminant-values-for-field-less-enumerations

Now we can add a handler function for the timer interrupt:

```rust
// in src/interrupts.rs

use crate::print;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        idt[InterruptIndex::Timer.as_usize()]
            .set_handler_fn(timer_interrupt_handler); // new

        idt
    };
}

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame)
{
    print!(".");
}
```

Our `timer_interrupt_handler` has the same signature as our exception handlers, because the CPU reacts identically to exceptions and external interrupts (the only difference is that some exceptions push an error code). The [`InterruptDescriptorTable`] struct implements the [`IndexMut`] trait, so we can access individual entries through array indexing syntax.

[`InterruptDescriptorTable`]: https://docs.rs/x86_64/0.7.5/x86_64/structures/idt/struct.InterruptDescriptorTable.html
[`IndexMut`]: https://doc.rust-lang.org/core/ops/trait.IndexMut.html

In our timer interrupt handler, we print a dot to the screen. As the timer interrupt happens periodically, we would expect to see a dot appearing on each timer tick. However, when we run it we see that only a single dot is printed:

![QEMU printing only a single dot for hardware timer](qemu-single-dot-printed.png)

### End of Interrupt

The reason is that the PIC expects an explicit “end of interrupt” (EOI) signal from our interrupt handler. This signal tells the controller that the interrupt was processed and that the system is ready to receive the next interrupt. So the PIC thinks we're still busy processing the first timer interrupt and waits patiently for the EOI signal before sending the next one.

To send the EOI, we use our static `PICS` struct again:

```rust
// in src/interrupts.rs

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame)
{
    print!(".");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}
```

The `notify_end_of_interrupt` figures out whether the primary or secondary PIC sent the interrupt and then uses the `command` and `data` ports to send an EOI signal to respective controllers. If the secondary PIC sent the interrupt both PICs need to be notified because the secondary PIC is connected to an input line of the primary PIC.

We need to be careful to use the correct interrupt vector number, otherwise we could accidentally delete an important unsent interrupt or cause our system to hang. This is the reason that the function is unsafe.

When we now execute `cargo xrun` we see dots periodically appearing on the screen:

![QEMU printing consecutive dots showing the hardware timer](qemu-hardware-timer-dots.gif)

### Configuring the Timer

The hardware timer that we use is called the _Progammable Interval Timer_ or PIT for short. Like the name says, it is possible to configure the interval between two interrupts. We won't go into details here because we will switch to the [APIC timer] soon, but the OSDev wiki has an extensive article about the [configuring the PIT].

[APIC timer]: https://wiki.osdev.org/APIC_timer
[configuring the PIT]: https://wiki.osdev.org/Programmable_Interval_Timer

## Deadlocks

We now have a form of concurrency in our kernel: The timer interrupts occur asynchronously, so they can interrupt our `_start` function at any time. Fortunately Rust's ownership system prevents many types of concurrency related bugs at compile time. One notable exception are deadlocks. Deadlocks occur if a thread tries to acquire a lock that will never become free. Thus the thread hangs indefinitely.

We can already provoke a deadlock in our kernel. Remember, our `println` macro calls the `vga_buffer::_print` function, which [locks a global `WRITER`][vga spinlock] using a spinlock:

[vga spinlock]: ./second-edition/posts/03-vga-text-buffer/index.md#spinlocks

```rust
// in src/vga_buffer.rs

[…]

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}
```

It locks the `WRITER`, calls `write_fmt` on it, and implicitly unlocks it at the end of the function. Now imagine that an interrupt occurs while the `WRITER` is locked and the interrupt handler tries to print something too:

Timestep | _start | interrupt_handler
---------|------|------------------
0 | calls `println!`      | &nbsp;
1 | `print` locks `WRITER` | &nbsp;
2 | | **interrupt occurs**, handler begins to run
3 | | calls `println!` |
4 | | `print` tries to lock `WRITER` (already locked)
5 | | `print` tries to lock `WRITER` (already locked)
… | | …
_never_ | _unlock `WRITER`_ |

The `WRITER` is locked, so the interrupt handler waits until it becomes free. But this never happens, because the `_start` function only continues to run after the interrupt handler returns. Thus the complete system hangs.

### Provoking a Deadlock

We can easily provoke such a deadlock in our kernel by printing something in the loop at the end of our `_start` function:

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    […]
    loop {
        use blog_os::print;
        print!("-");        // new
    }
}
```

When we run it in QEMU we get output of the form:

![QEMU output with many rows of hyphens and no dots](./qemu-deadlock.png)

We see that only a limited number of hyphens is printed, until the first timer interrupt occurs. Then the system hangs because the timer interrupt handler deadlocks when it tries to print a dot. This is the reason that we see no dots in the above output.

The actual number of hyphens varies between runs because the timer interrupt occurs asynchronously. This non-determinism is what makes concurrency related bugs so difficult to debug.

### Fixing the Deadlock

To avoid this deadlock, we can disable interrupts as long as the `Mutex` is locked:

```rust
// in src/vga_buffer.rs

/// Prints the given formatted string to the VGA text buffer
/// through the global `WRITER` instance.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;   // new

    interrupts::without_interrupts(|| {     // new
        WRITER.lock().write_fmt(args).unwrap();
    });
}
```

The [`without_interrupts`] function takes a [closure] and executes it in an interrupt-free environment. We use it to ensure that no interrupt can occur as long as the `Mutex` is locked. When we run our kernel now we see that it keeps running without hanging. (We still don't notice any dots, but this is because they're scrolling by too fast. Try to slow down the printing, e.g. by putting a `for _ in 0..10000 {}` inside the loop.)

[`without_interrupts`]: https://docs.rs/x86_64/0.7.5/x86_64/instructions/interrupts/fn.without_interrupts.html
[closure]: https://doc.rust-lang.org/book/second-edition/ch13-01-closures.html

We can apply the same change to our serial printing function to ensure that no deadlocks occur with it either:

```rust
// in src/serial.rs

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;       // new

    interrupts::without_interrupts(|| {         // new
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}
```

Note that disabling interrupts shouldn't be a general solution. The problem is that it increases the worst case interrupt latency, i.e. the time until the system reacts to an interrupt. Therefore interrupts should be only disabled for a very short time.

## Fixing a Race Condition

If you run `cargo xtest` you might see the `test_println_output` test failing:

```
> cargo xtest --lib
[…]
Running 4 tests
test_breakpoint_exception...[ok]
test_println... [ok]
test_println_many... [ok]
test_println_output... [failed]

Error: panicked at 'assertion failed: `(left == right)`
  left: `'.'`,
 right: `'S'`', src/vga_buffer.rs:205:9
```

The reason is a _race condition_ between the test and our timer handler. Remember, the test looks like this:

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_output() {
    serial_print!("test_println_output... ");

    let s = "Some test string that fits on a single line";
    println!("{}", s);
    for (i, c) in s.chars().enumerate() {
        let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
        assert_eq!(char::from(screen_char.ascii_character), c);
    }

    serial_println!("[ok]");
}
```

The test prints a string to the VGA buffer and then checks the output by manually iterating over the `buffer_chars` array. The race condition occurs because the timer interrupt handler might run between the `println` and the reading of the screen characters. Note that this isn't a dangerous _data race_, which Rust completely prevents at compile time. See the [_Rustonomicon_][nomicon-races] for details.

[nomicon-races]: https://doc.rust-lang.org/nomicon/races.html

To fix this, we need to keep the `WRITER` locked for the complete duration of the test, so that the timer handler can't write a `.` to the screen in between. The fixed test looks like this:

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    serial_print!("test_println_output... ");

    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });

    serial_println!("[ok]");
}
```

We performed the following changes:

- We keep the writer locked for the complete test by using the `lock()` method explicitly. Instead of `println`, we use the [`writeln`] marco that allows printing to an already locked writer.
- To avoid another deadlock, we disable interrupts for the tests duration. Otherwise the test might get interrupted while the writer is still locked.
- Since the timer interrupt handler can still run before the test, we print an additional newline `\n` before printing the string `s`. This way, we avoid test failure when the timer handler already printed some `.` characters to the current line.

[`writeln`]: https://doc.rust-lang.org/core/macro.writeln.html

With the above changes, `cargo xtest` now deterministically succeeds again.

This was a very harmless race condition that only caused a test failure. As you can imagine, other race conditions can be much more difficult to debug due to their non-deterministic nature. Luckily, Rust prevents us from data races, which are the most serious class of race conditions since they can cause all kinds of undefined behavior, including system crashes and silent memory corruptions.

## The `hlt` Instruction

Until now we used a simple empty loop statement at the end of our `_start` and `panic` functions. This causes the CPU to spin endlessly and thus works as expected. But it is also very inefficient, because the CPU continues to run at full speed even though there's no work to do. You can see this problem in your task manager when you run your kernel: The QEMU process needs close to 100% CPU the whole time.

What we really want to do is to halt the CPU until the next interrupt arrives. This allows the CPU to enter a sleep state in which it consumes much less energy. The [`hlt` instruction] does exactly that. Let's use this instruction to create an energy efficient endless loop:

[`hlt` instruction]: https://en.wikipedia.org/wiki/HLT_(x86_instruction)

```rust
// in src/lib.rs

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
```

The `instructions::hlt` function is just a [thin wrapper] around the assembly instruction. It is safe because there's no way it can compromise memory safety.

[thin wrapper]: https://github.com/rust-osdev/x86_64/blob/5e8e218381c5205f5777cb50da3ecac5d7e3b1ab/src/instructions/mod.rs#L16-L22

We can now use this `hlt_loop` instead of the endless loops in our `_start` and `panic` functions:

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    […]

    println!("It did not crash!");
    blog_os::hlt_loop();            // new
}


#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    blog_os::hlt_loop();            // new
}

```

Let's update our `lib.rs` as well:

```rust
// in src/lib.rs

/// Entry point for `cargo xtest`
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init();
    test_main();
    hlt_loop();         // new
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();         // new
}
```

We can also use `hlt_loop` in our double fault exception handler:

```rust
// in src/interrupts.rs

use crate::hlt_loop; // new

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) {
    println!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
    hlt_loop(); // new
}
```

When we run our kernel now in QEMU, we see a much lower CPU usage.

## Keyboard Input

Now that we are able to handle interrupts from external devices we are finally able to add support for keyboard input. This will allow us to interact with our kernel for the first time.

<aside class="post_aside">

Note that we only describe how to handle [PS/2] keyboards here, not USB keyboards. However the mainboard emulates USB keyboards as PS/2 devices to support older software, so we can safely ignore USB keyboards until we have USB support in our kernel.

</aside>

[PS/2]: https://en.wikipedia.org/wiki/PS/2_port

Like the hardware timer, the keyboard controller is already enabled by default. So when you press a key the keyboard controller sends an interrupt to the PIC, which forwards it to the CPU. The CPU looks for a handler function in the IDT, but the corresponding entry is empty. Therefore a double fault occurs.

So let's add a handler function for the keyboard interrupt. It's quite similar to how we defined the handler for the timer interrupt, it just uses a different interrupt number:

```rust
// in src/interrupts.rs

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // new
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        // new
        idt[InterruptIndex::Keyboard.as_usize()]
            .set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame)
{
    print!("k");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

As we see from the graphic [above](#the-8259-pic), the keyboard uses line 1 of the primary PIC. This means that it arrives at the CPU as interrupt 33 (1 + offset 32). We add this index as a new `Keyboard` variant to the `InterruptIndex` enum. We don't need to specify the value explicitly, since it defaults to the previous value plus one, which is also 33. In the interrupt handler, we print a `k` and send the end of interrupt signal to the interrupt controller.

We now see that a `k` appears on the screen when we press a key. However, this only works for the first key we press, even if we continue to press keys no more `k`s appear on the screen. This is because the keyboard controller won't send another interrupt until we have read the so-called _scancode_ of the pressed key.

### Reading the Scancodes

To find out _which_ key was pressed, we need to query the keyboard controller. We do this by reading from the data port of the PS/2 controller, which is the [I/O port] with number `0x60`:

[I/O port]: ./second-edition/posts/04-testing/index.md#i-o-ports

```rust
// in src/interrupts.rs

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame)
{
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    print!("{}", scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

We use the [`Port`] type of the `x86_64` crate to read a byte from the keyboard's data port. This byte is called the [_scancode_] and is a number that represents the key press/release. We don't do anything with the scancode yet, we just print it to the screen:

[`Port`]: https://docs.rs/x86_64/0.7.5/x86_64/instructions/port/struct.Port.html
[_scancode_]: https://en.wikipedia.org/wiki/Scancode

![QEMU printing scancodes to the screen when keys are pressed](qemu-printing-scancodes.gif)

The above image shows me slowly typing "123". We see that adjacent keys have adjacent scancodes and that pressing a key causes a different scancode than releasing it. But how do we translate the scancodes to the actual key actions exactly?

### Interpreting the Scancodes
There are three different standards for the mapping between scancodes and keys, the so-called _scancode sets_. All three go back to the keyboards of early IBM computers: the [IBM XT], the [IBM 3270 PC], and the [IBM AT]. Later computers fortunately did not continue the trend of defining new scancode sets, but rather emulated the existing sets and extended them. Today most keyboards can be configured to emulate any of the three sets.

[IBM XT]: https://en.wikipedia.org/wiki/IBM_Personal_Computer_XT
[IBM 3270 PC]: https://en.wikipedia.org/wiki/IBM_3270_PC
[IBM AT]: https://en.wikipedia.org/wiki/IBM_Personal_Computer/AT

By default, PS/2 keyboards emulate scancode set 1 ("XT"). In this set, the lower 7 bits of a scancode byte define the key, and the most significant bit defines whether it's a press ("0") or a release ("1"). Keys that were not present on the original [IBM XT] keyboard, such as the enter key on the keypad, generate two scancodes in succession: a `0xe0` escape byte and then a byte representing the key. For a list of all set 1 scancodes and their corresponding keys, check out the [OSDev Wiki][scancode set 1].

[scancode set 1]: https://wiki.osdev.org/Keyboard#Scan_Code_Set_1

To translate the scancodes to keys, we can use a match statement:

```rust
// in src/interrupts.rs

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame)
{
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    // new
    let key = match scancode {
        0x02 => Some('1'),
        0x03 => Some('2'),
        0x04 => Some('3'),
        0x05 => Some('4'),
        0x06 => Some('5'),
        0x07 => Some('6'),
        0x08 => Some('7'),
        0x09 => Some('8'),
        0x0a => Some('9'),
        0x0b => Some('0'),
        _ => None,
    };
    if let Some(key) = key {
        print!("{}", key);
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

The above code translates keypresses of the number keys 0-9 and ignores all other keys. It uses a [match] statement to assign a character or `None` to each scancode. It then uses [`if let`] to destructure the optional `key`. By using the same variable name `key` in the pattern, we [shadow] the previous declaration, which is a common pattern for destructuring `Option` types in Rust.

[match]: https://doc.rust-lang.org/book/ch06-02-match.html
[`if let`]: https://doc.rust-lang.org/book/ch18-01-all-the-places-for-patterns.html#conditional-if-let-expressions
[shadow]: https://doc.rust-lang.org/book/ch03-01-variables-and-mutability.html#shadowing

Now we can write numbers:

![QEMU printing numbers to the screen](qemu-printing-numbers.gif)

Translating the other keys works in the same way. Fortunately there is a crate named [`pc-keyboard`] for translating scancodes of scancode sets 1 and 2, so we don't have to implement this ourselves. To use the crate, we add it to our `Cargo.toml` and import it in our `lib.rs`:

[`pc-keyboard`]: https://docs.rs/pc-keyboard/0.3.1/pc_keyboard/

```toml
# in Cargo.toml

[dependencies]
pc-keyboard = "0.3.1"
```

Now we can use this crate to rewrite our `keyboard_interrupt_handler`:

```rust
// in/src/interrupts.rs

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame)
{
    use x86_64::instructions::port::Port;
    use pc_keyboard::{Keyboard, ScancodeSet1, DecodedKey, layouts};
    use spin::Mutex;

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(layouts::Us104Key, ScancodeSet1));
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

We use the `lazy_static` macro to create a static [`Keyboard`] object protected by a Mutex. On each interrupt, we lock the Mutex, read the scancode from the keyboard controller and pass it to the [`add_byte`] method, which translates the scancode into an `Option<KeyEvent>`. The [`KeyEvent`] contains which key caused the event and whether it was a press or release event.

[`Keyboard`]: https://docs.rs/pc-keyboard/0.3.1/pc_keyboard/struct.Keyboard.html
[`add_byte`]: https://docs.rs/pc-keyboard/0.3.1/pc_keyboard/struct.Keyboard.html#method.add_byte
[`KeyEvent`]: https://docs.rs/pc-keyboard/0.3.1/pc_keyboard/struct.KeyEvent.html

To interpret this key event, we pass it to the [`process_keyevent`] method, which translates the key event to a character if possible. For example, translates a press event of the `A` key to either a lowercase `a` character or an uppercase `A` character, depending on whether the shift key was pressed.

[`process_keyevent`]: https://docs.rs/pc-keyboard/0.3.1/pc_keyboard/struct.Keyboard.html#method.process_keyevent

With this modified interrupt handler we can now write text:

![Typing "Hello World" in QEMU](qemu-typing.gif)

### Configuring the Keyboard

It's possible to configure some aspects of a PS/2 keyboard, for example which scancode set it should use. We won't cover it here because this post is already long enough, but the OSDev Wiki has an overview of possible [configuration commands].

[configuration commands]: https://wiki.osdev.org/PS/2_Keyboard#Commands

## Summary

This post explained how to enable and handle external interrupts. We learned about the 8259 PIC and its primary/secondary layout, the remapping of the interrupt numbers, and the "end of interrupt" signal. We implemented handlers for the hardware timer and the keyboard and learned about the `hlt` instruction, which halts the CPU until the next interrupt.

Now we are able to interact with our kernel and have some fundamental building blocks for creating a small shell or simple games.

## What's next?

Timer interrupts are essential for an operating system, because they provide a way to periodically interrupt the running process and regain control in the kernel. The kernel can then switch to a different process and create the illusion that multiple processes run in parallel.

But before we can create processes or threads, we need a way to allocate memory for them. The next posts will explore memory management to provide this fundamental building block.
