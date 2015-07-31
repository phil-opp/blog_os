---
layout: post
title: '[DRAFT] Rust OS Part 1: Booting'
related_posts: null
---
## Multiboot
Fortunately there is a bootloader standard: the [Multiboot Specification][multiboot].  So our kernel just needs to indicate that it supports Multiboot and every Multiboot-compliant bootloader can boot it. We will use the [GRUB 2] bootloader together with the Multiboot 2 specification ([PDF][Multiboot 2]).

To indicate our Multiboot 2 support to the bootloader, our kernel must contain a _Multiboot Header_, which has the following format:

Field         | Type            | Value
------------- | --------------- | ----------------------------------------
magic number  | u32             | 0xE85250D6
architecture  | u32             | 0 for i386, 4 for MIPS
header length | u32             | total header size, including tags
checksum      | u32             | -(magic + architecture + header length)
tags          | variable        |
end tag       | (u16, u16, u32) | (0, 0, 8)

Converted to a x86 assembly file it looks like this (Intel syntax):

```nasm
section .multiboot_header
header_start:
    dd 0xe85250d6                ; magic number (multiboot 2)
    dd 0                         ; architecture 0 (protected mode i386)
    dd header_end - header_start ; header length
    ; checksum
    dd 0x100000000 - (0xe85250d6 + 0 + (header_end - header_start))

    ; insert optional multiboot tags here

    ; required end tag
    dw 0    ; type
    dw 0    ; flags
    dd 8    ; size
header_end:
```
If you don't know x86 assembly, here is some quick guide:

- the header will be written to a section named `.multiboot_header` (we need this later)
- `header_start` and `header_end` are _labels_ that mark a memory location. We use them to calculate the header length easily
- `dd` stands for `define double` (32bit) and `dw` stands for `define word` (16bit)
- the additional `0x100000000` in the checksum calculation is a small hack[^fn-checksum_hack] to avoid a compiler warning

We can already _assemble_ this file (which I called `multiboot_header.asm`) using `nasm`. As it produces a flat binary by default, the resulting file just contains our 24 bytes (in little endian if you work on a x86 machine):

```
> nasm multiboot_header.asm
> hexdump -x multiboot_header
0000000    50d6    e852    0000    0000    0018    0000    af12    17ad
0000010    0000    0000    0008    0000
0000018
```


## Booting


[^fn-checksum_hack]: The formula from the table, `-(magic + architecture + header length)`, creates a negative value that doesn't fit into 32bit. By subtracting from `0x100000000` instead, we keep the value positive without changing its truncated value. Without the additional sign bit(s) the result fits into 32bit and the compiler is happy.

[multiboot]: https://en.wikipedia.org/wiki/Multiboot_Specification
[grub 2]: http://wiki.osdev.org/GRUB_2
[multiboot 2]: http://nongnu.askapache.com/grub/phcoder/multiboot.pdf
