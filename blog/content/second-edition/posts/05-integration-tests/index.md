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

In the previous post we added support for unit tests. The goal of unit tests is to test small components in isolation to ensure that all of them work as intended. They are run on the host machine and thus shouldn't rely on architecture specific functionality.

To test the interaction of the components, both with each other and the system environment, we can write _integration tests_. Compared to unit tests, ìntegration tests are more complex, because they need to run in a realistic environment. What this means depends on the application type. For example, for webserver applications it often means to set up a database instance. For an operating system kernel like ours, it means that we run the tests on the target hardware.

Running on the target architecture allows us to test all hardware specific code such as the VGA buffer or the effects of [page table] modifications. It also allows us to verify that our kernel boots without problems and that no [CPU exception] occurs. To achieve these goals reliably, test instances need to be independent. For example, if one tests misconfigures the system by loading an invalid page table, it should not influence the result of other tests.

[page table]: https://en.wikipedia.org/wiki/Page_table
[CPU exception]: https://wiki.osdev.org/Exceptions

In this post we will implement a very basic test framework that runs integration tests inside instances of the [QEMU] virtual machine. It is not as realistic as running them on real hardware, but it requires no addional hardware and makes the implementation simpler. Also, most hardware properties we use at this early stage are supported pretty well in QEMU, so it should be good enough for now.

[QEMU]: https://www.qemu.org/

## The Serial Port

The first problem we need to solve is how to retrieve the test results on the host system. Tests run inside QEMU, so we need a way to send them from inside the virtual machine to the host. The easiest way to achieve this is the [serial port], an old interface standard which is no longer found in modern computers. It is easy to program and QEMU can redirect the bytes sent over serial to the host's standard output or a file. This allows us to send "ok" or "failed" from our kernel to the host system.

[serial port]: https://en.wikipedia.org/wiki/Serial_port

### Implementation

TODO

- serial crate -> "Hello host" -> "hprintln!"

## Shutting Down QEMU

TODO

- qemu argument and port write

## Test Organization

TODO

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
