---
layout: post
title: '[DRAFT] Rust OS Part 1: Booting'
related_posts: null
---
## Multiboot
Fortunately there is a bootloader standard: the [Multiboot Specification][multiboot].  So our kernel just needs to indicate that it supports Multiboot and every Multiboot-compliant bootloader can boot it. We will use the Multiboot 2 specification ([PDF][Multiboot 2]) together with the well-known [GRUB 2] bootloader.

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

We can already _assemble_ this file (which I called `multiboot_header.asm`) using `nasm`. It produces a flat binary by default, so the resulting file just contains our 24 bytes (in little endian if you work on a x86 machine):

```
> nasm multiboot_header.asm
> hexdump -x multiboot_header
0000000    50d6    e852    0000    0000    0018    0000    af12    17ad
0000010    0000    0000    0008    0000
0000018
```

[multiboot]: https://en.wikipedia.org/wiki/Multiboot_Specification
[multiboot 2]: http://nongnu.askapache.com/grub/phcoder/multiboot.pdf
[grub 2]: http://wiki.osdev.org/GRUB_2
[^fn-checksum_hack]: The formula from the table, `-(magic + architecture + header length)`, creates a negative value that doesn't fit into 32bit. By subtracting from `0x100000000` instead, we keep the value positive without changing its truncated value. Without the additional sign bit(s) the result fits into 32bit and the compiler is happy.

## The Boot Code
To boot our kernel, we must add some code that the bootloader can call. Let's create a file named `boot.asm`:

```nasm
global start

BITS 32
section .text
start:
    mov dword [0xb8000], 0x2f4b2f4f
    hlt
```
There are some new commands:

- `global` exports a label (makes it public). As `start` will be the entry point of our kernel, it needs to be public.
- `BITS 32` specifies that the following lines are 32-bit instructions. We don't need it in our multiboot header file, as it doesn't contain any runnable code.
- the `.text` section is the default section for executable code
- the `mov dword` instruction moves the 32bit constant `0x2f4f2f4b` to the memory at address `b8000` (it writes `OK` to the screen, an explanation follows in the [next post])
- `hlt` is the halt instruction and causes the CPU to stop

Through assembling, viewing and disassembling it we can see the CPU [Opcodes] in action:

```
> nasm boot.asm
> hexdump -x boot
0000000    05c7    8000    000b    2f4b    2f4f    00f4
000000b
> ndisasm -b 32 boot
00000000  C70500800B004B2F  mov dword [dword 0xb8000],0x2f4b2f4f
         -4F2F
0000000A  F4                hlt
```

[Opcodes]: https://en.wikipedia.org/wiki/Opcode

## Building the Executable
Now we create an [ELF] executable from these two files. We therefore need the object files of the two assembly files and a custom [linker script], that we call `linker.ld`:

```
ENTRY(start)

SECTIONS {
    . = 1M;

    .boot :
    {
        /* ensure that the multiboot header is at the beginning */
        *(.multiboot_header)
    }

    .text :
    {
        *(.text)
    }
}
```
Let's translate it:

- `start` is the entry point, the bootloader will jump to it after loading the kernel
- `. = 1M;` sets the load address of the first section to 1 MiB, which is a conventional place to load a kernel
- the executable will have two sections: `.boot` at the beginning and `.text` afterwards
- the `.text` output section contains all input sections named `.text`
- Sections named `.multiboot_header` are added to the first output section (`.boot`) to ensure they are at the beginning of the executable. This is necessary because GRUB expects to find the Multiboot header very early in the file.

So let's create the ELF object files and link them using our new linker script. It's important to pass the `-n` flag because otherwise the linker may page align the sections in the executable. If that happens, GRUB isn't able to find the Multiboot header because the `.boot` section isn't at the beginning anymore. We can use `objdump` to print the sections of the generated executable and verify that the `.boot` section has a low file offset.

```
> nasm -f elf64 multiboot_header.asm
> nasm -f elf64 boot.asm
> ld -n -o kernel.bin -T linker.ld multiboot_header.o boot.o
> objdump -h kernel.bin
kernel.bin:     file format elf64-x86-64

Sections:
Idx Name          Size      VMA               LMA               File off  Algn
  0 .boot         00000018  0000000000100000  0000000000100000  00000080  2**0
                  CONTENTS, ALLOC, LOAD, READONLY, DATA
  1 .text         0000000b  0000000000100020  0000000000100020  000000a0  2**4
                  CONTENTS, ALLOC, LOAD, READONLY, CODE
```

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
[linker script]: https://sourceware.org/binutils/docs/ld/Scripts.html

## Creating the ISO
The last step is to create a bootable ISO image with GRUB. We need to create the following directory structure and copy the `kernel.bin` to the right place:

```
isofiles
└── boot
    ├── grub
    │   └── grub.cfg
    └── kernel.bin

```
The `grub.cfg` specifies the file name of our kernel and it's Multiboot 2 compliance. It looks like this:

```
set timeout=0
set default=0

menuentry "my os" {
    multiboot2 /boot/kernel.bin
    boot
}
```
Now we can create a bootable image using the command:

```
grub-mkrescue -o os.iso isofiles
```

## Booting
Now it's time to boot our OS. We will use [QEMU]:

```
qemu-system-x86_64 -hda os.iso
```
![qemu output]({{ site.url }}/images/qemu-ok.png)

Notice the green `OK` in the upper left corner. Let's summarize what happens:

1. the BIOS loads the bootloader (GRUB) from the virtual hard drive (the ISO)
2. the bootloader reads the kernel executable and finds the Multiboot header
3. it copies the `.boot` and `.text` sections to memory (to addresses `0x100000` and `0x100020`)
4. it jumps to the entry point (`0x100020`, obtain it through `objdump -f`)
5. our kernel writes the green `OK` and stops the CPU

You can test it on real hardware, too. Just burn the ISO to a disk or USB stick and boot from it.

[QEMU]: https://en.wikipedia.org/wiki/QEMU

[next post]: #TODO
