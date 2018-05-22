+++
title = "Integration Tests"
order = 5
path = "integration-tests"
date  = 2018-05-18
template = "second-edition/page.html"
+++

In this post we complete the testing picture by implementing a basic integration test framework, which allows us to run tests on the target system. The idea is to run tests inside QEMU and report the results back to the host through the serial port.

<!-- more -->

This blog is openly developed on [Github]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom].

[Github]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments

## Overview

In the previous post we added support for unit tests. The goal of unit tests is to test small components in isolation to ensure that all of them work as intended. The tests are run on the host machine and thus shouldn't rely on architecture specific functionality.

To test the interaction of the components, both with each other and the system environment, we can write _integration tests_. Compared to unit tests, ìntegration tests are more complex, because they need to run in a realistic environment. What this means depends on the application type. For example, for webserver applications it often means to set up a database instance. For an operating system kernel like ours, it means that we run the tests on the target hardware without an underlying operating system.

Running on the target architecture allows us to test all hardware specific code such as the VGA buffer or the effects of [page table] modifications. It also allows us to verify that our kernel boots without problems and that no [CPU exception] occurs.

[page table]: https://en.wikipedia.org/wiki/Page_table
[CPU exception]: https://wiki.osdev.org/Exceptions

In this post we will implement a very basic test framework that runs integration tests inside instances of the [QEMU] virtual machine. It is not as realistic as running them on real hardware, but it is much simpler and should be suffient as long as we only use standard hardware that is well supported in QEMU.

[QEMU]: https://www.qemu.org/

## The Serial Port

The naive way of doing an integration test would be to add some assertions in the code, launch QEMU, and manually check if a panic occured or not. This is very cumbersome and not practical if we have hundreds of integration tests. So we want an automated solution that runs all tests and fails if not all of them pass.

Such an automated test framework needs to know whether a test succeeded or failed. It can't look at the screen output of QEMU, so we need a different way of retrieving the test results on the host system. A simple way to achieve this is by using the [serial port], an old interface standard which is no longer found in modern computers. It is easy to program and QEMU can redirect the bytes sent over serial to the host's standard output or a file.

[serial port]: https://en.wikipedia.org/wiki/Serial_port

The chips implementing a serial interface are called [UARTs]. There are [lots of UART models] on x86, but fortunately the only differences between them are some advanced features we don't need. The common UARTs today are all compatible to the [16550 UART], so we will use that model for our testing framework.

[UARTs]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter
[lots of UART models]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter#UART_models
[16550 UART]: https://en.wikipedia.org/wiki/16550_UART

### Port I/O
 TODO

### Implementation

There is already the [`uart_16550`] crate, so we don't need to write our own driver for it. To add it as a dependency, we update our `Cargo.toml` and `main.rs`:

[`uart_16550`]: https://docs.rs/uart_16550

```toml
# in Cargo.toml

[dependencies]
uart_16550 = "0.1.0"
```

```rust
// in src/main.rs

extern crate uart_16550;
```

The `uart_16550` crate contains a `SerialPort` struct that represents the UART registers, but we still need to construct an instance of it ourselves. For that we create a new `serial` module with the following content:

```rust
// in src/main.rs

mod serial
```

```rust
// in src/serial.rs

use uart_16550::SerialPort;
use spin::Mutex;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = SerialPort::new(0x3F8);
        serial_port.init();
        Mutex::new(serial_port)
    };
}
```

Like with the [VGA text buffer], we use a spinlock and `lazy_static` to create a `static`. However, this time we use `lazy_static` to ensure that the `init` method is called before first use. We're using the port address `0x3F8`, which is standard for the first serial interface.

[VGA text buffer]: ./second-edition/posts/03-vga-text-buffer/index.md#spinlocks

To make the serial port easily usable, we add `serial_print!` and `serial_println!` macros:

```rust
pub fn print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    let _ = COM1.lock().write_fmt(args);
}

/// Prints to the host through the serial interface.
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
macro_rules! serial_println {
    () => (serial_print!("\n"));
    ($fmt:expr) => (serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (serial_print!(concat!($fmt, "\n"), $($arg)*));
}
```

The `SerialPort` type already implements the [`fmt::Write`] trait, so we don't need to provide an implementation.

[`fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

Now we can print to the serial interface in our `main.rs`:

```rust
// in src/main.rs

#[macro_use]
mod serial;

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!"); // prints to vga buffer
    hprintln!("Hello Host{}", "!"); // prints to serial

    loop {}
}
```

Note that we need to add the `#[macro_use]` attribute to the `mod serial` declaration, because otherwise the `hprintln` macro is not imported.

### QEMU Arguments

To see the serial output in QEMU, we can use the `-serial` argument to redirect the output to stdout:

```
> qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-blog_os/debug/bootimage-blog_os.bin \
    -serial mon:stdio
warning: TCG doesn't support requested feature: CPUID.01H:ECX.vmx [bit 5]
Hello Host!
```

If you chose a different name than `blog_os`, you need to update the paths of course. Note that you can no longer exit QEMU through `Ctrl+c`. As an alternative you can use `Ctrl+a` and then `x`.

As an alternative to this long command, we can pass the argument to `bootimage run`, with an additional `--` to separate the build arguments (passed to cargo) from the run arguments (passed to QEMU).

```
bootimage run -- -serial mon:stdio
```

Instead of standard output, QEMU supports [many more target devices][QEMU -serial]. For redirecting the output to a file, the argument is:

[QEMU -serial]: https://qemu.weilnetz.de/doc/qemu-doc.html#Debug_002fExpert-options

```
-serial file:output-file.txt
```

## Shutting Down QEMU

Right now we have an endless loop at the end of our `_start` function and need to close QEMU manually. This does not work for automated tests. We could try to kill QEMU automatically from the host, for example after some special output was sent over serial, but this would be a bit hacky and difficult to get right. The cleaner solution would be to implement a way to shutdown our OS. Unfortunatly this is relatively complex, because it requires implementing support for either the [APM] or [ACPI] power management standard.

[APM]: https://wiki.osdev.org/APM
[ACPI]: https://wiki.osdev.org/ACPI

Luckily, there is an escape hatch: QEMU supports a special `isa-debug-exit` device, which provides an easy way to exit QEMU from the guest system. To enable it, we add the following argument to our QEMU command:

```
-device isa-debug-exit,iobase=0xf4,iosize=0x04
```

The `iobase` specifies on which port address the device should live and the `iosize` specifies the port size (`0x04` means four bytes). Now the guest can write a value to the `0xf4` port and QEMU will exit with [exit status] `(passed_value << 1) | 1`.

[exit status]: https://en.wikipedia.org/wiki/Exit_status

To write to the I/O port, we use the [`x86_64`] crate:

[`x86_64`]: https://docs.rs/x86_64

```toml
# in Cargo.toml

[dependencies]
x86_64 = "0.2.0"
```

```rust
// in src/main.rs

extern crate x86_64;

pub unsafe fn exit_qemu() {
    use x86_64::instructions::port::Port;

    let mut port = Port::<u32>::new(0xf4);
    port.write(0);
}
```

We mark the function as `unsafe` because it relies on the fact that a special QEMU device is attached to the I/O port with address `0xf4`. For the port type we choose `u32` because the `iosize` is 4 bytes. As value we write a zero, which causes QEMU to exit with exit status `(0 << 1) | 1 = 1`.

Note that we could also use the exit status instead of the serial interface for sending the test results, for example `1` for success and `2` for failure. However, this wouldn't allow us to send panic messages like the serial interface does and would also prevent us from replacing `exit_qemu` with a proper shutdown someday. Therefore we continue to use the serial interface and just always write a `0` to the port.

We can now test the QEMU shutdown by calling `exit_qemu` from our `_start` function:

```rust
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!"); // prints to vga buffer
    println!("Hello Host{}", "!"); // prints to serial

    unsafe { exit_qemu(); }

    loop {}
}
```

You should see that QEMU immediately closes after booting.

## Hiding QEMU

We are now able to launch a QEMU instance that writes its output to the serial port and automatically exits itself when it's done. So we no longer need the VGA buffer output or the graphical representation that still pops up. We can disable it by passing the `-display none` parameter to QEMU. The full command looks like this:

```
qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-blog_os/debug/bootimage-blog_os.bin \
    -serial mon:stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -display none
```

Or, with `bootimage run`:

```
bootimage run -- \
    -serial mon:stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -display none
```

Now QEMU runs completely in the background and no window is opened anymore. This is not only less annoying, but also allows our test framework to run in environments without a graphical user interface, such as [Travis CI].

[Travis CI]: https://travis-ci.com/

## Test Organization

TODO

To achieve these goals reliably, test instances need to be independent. For example, if one tests misconfigures the system by loading an invalid page table, it should not influence the result of other tests.

 This allows us to send "ok" or "failed" from our kernel to the host system.


- split into lib.rs and main.rs
- add tests as src/bin/test-*

## Bootimage Test

TODO

- uses cargo metadata to find test-* binaries
- compiles and executes them, redirects output to file
- checks file for `ok`
- prints results

## Summary

TODO

TODO update date

## What's next?
In the next post, we will explore _CPU exceptions_. These exceptions are thrown by the CPU when something illegal happens, such as a division by zero or an access to an unmapped memory page (a so-called “page fault”). Being able to catch and examine these exceptions is very important for debugging future errors. Exception handling is also very similar to the handling of hardware interrupts, which is required for keyboard support.
