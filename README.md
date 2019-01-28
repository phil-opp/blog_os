# Blog OS

[![Build Status](https://travis-ci.org/phil-opp/blog_os.svg?branch=master)](https://travis-ci.org/phil-opp/blog_os) [![Join the chat at https://gitter.im/phil-opp/blog_os](https://badges.gitter.im/phil-opp/blog_os.svg)](https://gitter.im/phil-opp/blog_os?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

This repository contains the source code for the _Writing an OS in Rust_ series at [os.phil-opp.com](https://os.phil-opp.com).

## Building
You need a nightly Rust compiler and the `cargo-xbuild` and `bootimage` tools. You can install the tools by executing the following command:

```
cargo install cargo-xbuild bootimage
```

Afterwards you can invoke `bootimage build` to produce a bootable disk image. Please file an issue if you run into any problems.

To run the image in [QEMU], you can execute `bootimage run`. Note that you need to have QEMU installed.

[QEMU]: https://www.qemu.org/

## Posts

The goal of this project is to provide step-by-step tutorials in individual blog posts. We currently have the following set of posts:

### Bare Bones

- [A Freestanding Rust Binary](https://os.phil-opp.com/freestanding-rust-binary/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-01))
- [A Minimal Rust Kernel](https://os.phil-opp.com/minimal-rust-kernel/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-02))
- [VGA Text Mode](https://os.phil-opp.com/vga-text-mode/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-03))

### Testing

- [Unit Testing](https://os.phil-opp.com/unit-testing/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-04))
- [Integration Tests](https://os.phil-opp.com/integration-tests/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-05))

### Interrupts

- [CPU Exceptions](https://os.phil-opp.com/cpu-exceptions/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-06))
- [Double Faults](https://os.phil-opp.com/double-fault-exceptions/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-07))
- [Hardware Interrupts](https://os.phil-opp.com/hardware-interrupts/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-08))

### Memory Management

- [Introduction to Paging](https://os.phil-opp.com/paging-introduction/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-09))
- [Advanced Paging](https://os.phil-opp.com/advanced-paging/)
    ([source code](https://github.com/phil-opp/blog_os/tree/post-10))


## First Edition Posts

The current version of the blog is already the second edition. The first edition is outdated and no longer maintained, but might still be useful. The posts of the first edition are:

### Bare Bones
- [A Minimal x86 Kernel](https://os.phil-opp.com/multiboot-kernel.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_1))
- [Entering Long Mode](https://os.phil-opp.com/entering-longmode.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_2))
- [Set Up Rust](https://os.phil-opp.com/set-up-rust.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_3))
- [Printing to Screen](https://os.phil-opp.com/printing-to-screen.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_4))

### Memory Management
- [Allocating Frames](https://os.phil-opp.com/allocating-frames.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_5))
- [Page Tables](https://os.phil-opp.com/modifying-page-tables.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_6))
- [Remap the Kernel](https://os.phil-opp.com/remap-the-kernel.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_7))
- [Kernel Heap](https://os.phil-opp.com/kernel-heap.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_8))

### Exceptions
- [Handling Exceptions](https://os.phil-opp.com/handling-exceptions.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_9))
- [Double Faults](https://os.phil-opp.com/double-faults.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/first_edition_post_10))

### Additional Resources
- [Cross Compile Binutils](https://os.phil-opp.com/cross-compile-binutils.html)
- [Cross Compile libcore](https://os.phil-opp.com/cross-compile-libcore.html)
- [Set Up GDB](https://os.phil-opp.com/set-up-gdb.html)
- [Handling Exceptions using Naked Functions](https://os.phil-opp.com/handling-exceptions-with-naked-fns.html)
    - [Catching Exceptions](https://os.phil-opp.com/catching-exceptions.html)
          ([source code](https://github.com/phil-opp/blog_os/tree/catching_exceptions))
    - [Better Exception Messages](https://os.phil-opp.com/better-exception-messages.html)
          ([source code](https://github.com/phil-opp/blog_os/tree/better_exception_messages))
    - [Returning from Exceptions](https://os.phil-opp.com/returning-from-exceptions.html)
          ([source code](https://github.com/phil-opp/blog_os/tree/returning_from_exceptions))

## License
The source code is dual-licensed under MIT or the Apache License (Version 2.0). This excludes the `blog` directory.
