# Blog OS

[![Build Status](https://travis-ci.org/phil-opp/blog_os.svg?branch=master)](https://travis-ci.org/phil-opp/blog_os) [![Join the chat at https://gitter.im/phil-opp/blog_os](https://badges.gitter.im/phil-opp/blog_os.svg)](https://gitter.im/phil-opp/blog_os?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

This repository contains the source code for the _Writing an OS in Rust_ series at [os.phil-opp.com](http://os.phil-opp.com).

## Bare Bones
- [A Minimal x86 Kernel](http://os.phil-opp.com/multiboot-kernel.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_1))
- [Entering Long Mode](http://os.phil-opp.com/entering-longmode.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_2))
- [Set Up Rust](http://os.phil-opp.com/set-up-rust.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_3))
- [Printing to Screen](http://os.phil-opp.com/printing-to-screen.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_4))

## Memory Management
- [Allocating Frames](http://os.phil-opp.com/allocating-frames.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_5))
- [Page Tables](http://os.phil-opp.com/modifying-page-tables.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_6))
- [Remap the Kernel](http://os.phil-opp.com/remap-the-kernel.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_7))
- [Kernel Heap](http://os.phil-opp.com/kernel-heap.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_8))

## Exceptions
- [Handling Exceptions](http://os.phil-opp.com/handling-exceptions.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_9))
- [Double Faults](http://os.phil-opp.com/double-faults.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/post_10))

## Additional Resources
- [Cross Compile Binutils](http://os.phil-opp.com/cross-compile-binutils.html)
- [Cross Compile libcore](http://os.phil-opp.com/cross-compile-libcore.html)
- [Set Up GDB](http://os.phil-opp.com/set-up-gdb.html)
- [Handling Exceptions using Naked Functions](http://os.phil-opp.com/handling-exceptions-with-naked-fns.html)
    - [Catching Exceptions](http://os.phil-opp.com/catching-exceptions.html)
          ([source code](https://github.com/phil-opp/blog_os/tree/catching_exceptions))
    - [Better Exception Messages](http://os.phil-opp.com/better-exception-messages.html)
          ([source code](https://github.com/phil-opp/blog_os/tree/better_exception_messages))
    - [Returning from Exceptions](http://os.phil-opp.com/returning-from-exceptions.html)
          ([source code](https://github.com/phil-opp/blog_os/tree/returning_from_exceptions))

## Building
You need to have `nasm`, `grub-mkrescue`, `mformat` (included in `mtools`), `xorriso`, `qemu`, a nightly Rust compiler, and [xargo] installed. Then you can run it using `make run`.

[xargo]: https://github.com/japaric/xargo

Please file an issue if you run into any problems.

## License
The source code is dual-licensed under MIT or the Apache License (Version 2.0). This excludes the `blog` directory.
