---
layout: post
title: "[DRAFT] Rust OS Part 1: Booting"
related_posts:
---

Fortunately there is a bootloader standard: the [Multiboot
Specification][multiboot]. So our kernel just needs to indicate that it supports
Multiboot and every Multiboot-compliant bootloader can boot it. We will use the [GRUB 2] bootloader together with the [Multiboot 2] specification. So let's begin!

To indicate our Multiboot 2 support to the bootloader, our kernel must contain a *Multiboot Header*, which has the following format:

Field | Size in byte
------|-----
magic number | 4

Offset | Type | Field Name
-------|------|-----------
0      | u32  | magic
4      | u32  | architecture
8      | u32  | header_length
12     | u32  | checksum
16-XX  |      | tags

[multiboot]: https://en.wikipedia.org/wiki/Multiboot_Specification
[GRUB 2]: http://wiki.osdev.org/GRUB_2
[Multiboot 2]: http://nongnu.askapache.com/grub/phcoder/multiboot.pdf
