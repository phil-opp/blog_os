+++
title = "Bare Bones"
+++

In this first chapter, we explain how to create an operating system for the `x86_64` architecture step for step. Starting from scratch, we first create a minimal Rust executable that doesn't depend on the standard library. We then turn it into a bootable OS kernel by combining it with a bootloader. The resulting disk image can then be launched in the [QEMU](https://www.qemu.org/) emulator or booted on a real machine.
