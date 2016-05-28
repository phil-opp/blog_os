# Blog OS
[![Build Status](https://travis-ci.org/phil-opp/blog_os.svg?branch=master)](https://travis-ci.org/phil-opp/blog_os)

This repository contains the source code for the _Writing an OS in Rust_ series at [os.phil-opp.com](http://os.phil-opp.com).

## Bare Bones
- [A Minimal x86 Kernel](http://os.phil-opp.com/multiboot-kernel.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/multiboot_bootstrap))
- [Entering Long Mode](http://os.phil-opp.com/entering-longmode.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/entering_longmode))
- [Set Up Rust](http://os.phil-opp.com/set-up-rust.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/set_up_rust))
- [Printing to Screen](http://os.phil-opp.com/printing-to-screen.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/printing_to_screen))

## Memory Management
- [Allocating Frames](http://os.phil-opp.com/allocating-frames.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/allocating_frames))
- [Page Tables](http://os.phil-opp.com/modifying-page-tables.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/page_tables))
- [Remap the Kernel](http://os.phil-opp.com/remap-the-kernel.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/remap_the_kernel))
- [Kernel Heap](http://os.phil-opp.com/kernel-heap.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/kernel_heap))

## Interrupts
- [Catching Exceptions](http://os.phil-opp.com/catching-exceptions.html)
      ([source code](https://github.com/phil-opp/blog_os/tree/catching_exceptions))

## Additional Resources
- [Cross Compile Binutils](http://os.phil-opp.com/cross-compile-binutils.html)
- [Cross Compile libcore](http://os.phil-opp.com/cross-compile-libcore.html)
- [Set Up GDB](http://os.phil-opp.com/set-up-gdb.html)

## Building
You need to have `nasm`, `grub-mkrescue`, `xorriso`, `qemu` and a nighly Rust compiler installed. Then you can run it using `make run`.

Please file an issue if you run into any problems.

## License
The source code is dual-licensed under MIT or the Apache License (Version 2.0). This excludes the `blog` directory.
