---
layout: post
title: 'A minimal x86 kernel'
redirect_from: '/2015/08/18/multiboot-kernel/'
---
This post explains how to create a minimal x86 operating system kernel. In fact, it will just boot and print `OK` to the screen. The following blog posts we will extend it using the [Rust] programming language.

I tried to explain everything in detail and to keep the code as simple as possible. If you have any questions, suggestions or other issues, please leave a comment or [create an issue] on Github. The source code is available in a [repository][source code], too.

[Rust]: http://www.rust-lang.org/
[create an issue]: https://github.com/phil-opp/blog_os/issues
[source code]: https://github.com/phil-opp/blog_os/tree/multiboot_bootstrap/src/arch/x86_64

Note that this tutorial is written mainly for Linux. For some known problems on OS X see the comment section and [this issue][mac os issue]. If you want to use a virtual Linux machine, you can find instructions and a Vagrantfile in Ashley Willams's [x86-kernel repository].

[mac os issue]: https://github.com/phil-opp/blog_os/issues/55
[x86-kernel repository]: https://github.com/ashleygwilliams/x86-kernel

## Overview
When you turn on a computer, it loads the [BIOS] from some special flash memory. The BIOS runs self test and initialization routines of the hardware, then it looks for bootable devices. If it finds one, the control is transferred to its _bootloader_, which is a small portion of executable code stored at the device's beginning. The bootloader has to determine the location of the kernel image on the device and load it into memory. It also needs to switch the CPU to the so-called [protected mode] because x86 CPUs start in the very limited [real mode] by default (to be compatible to programs from 1978).

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[protected mode]: https://en.wikipedia.org/wiki/Protected_mode
[real mode]: http://wiki.osdev.org/Real_Mode

We won't write a bootloader because that would be a complex project on its own (if you really want to do it, check out [_Rolling Your Own Bootloader_]). Instead we will use one of the [many well-tested bootloaders][bootloader comparison] out there. But which one?

[_Rolling Your Own Bootloader_]: http://wiki.osdev.org/Rolling_Your_Own_Bootloader
[bootloader comparison]: https://en.wikipedia.org/wiki/Comparison_of_boot_loaders

## Multiboot
Fortunately there is a bootloader standard: the [Multiboot Specification][multiboot]. Our kernel just needs to indicate that it supports Multiboot and every Multiboot-compliant bootloader can boot it. We will use the Multiboot 2 specification ([PDF][Multiboot 2]) together with the well-known [GRUB 2] bootloader.

[multiboot]: https://en.wikipedia.org/wiki/Multiboot_Specification
[multiboot 2]: http://nongnu.askapache.com/grub/phcoder/multiboot.pdf
[grub 2]: http://wiki.osdev.org/GRUB_2

To indicate our Multiboot 2 support to the bootloader, our kernel must start with a _Multiboot Header_, which has the following format:

Field         | Type            | Value
------------- | --------------- | ----------------------------------------
magic number  | u32             | `0xE85250D6`
architecture  | u32             | `0` for i386, `4` for MIPS
header length | u32             | total header size, including tags
checksum      | u32             | `-(magic + architecture + header_length)`
tags          | variable        |
end tag       | (u16, u16, u32) | `(0, 0, 8)`

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
- `header_start` and `header_end` are _labels_ that mark a memory location, we use them to calculate the header length easily
- `dd` stands for `define double` (32bit) and `dw` stands for `define word` (16bit). They just output the specified 32bit/16bit constant.
- the additional `0x100000000` in the checksum calculation is a small hack[^fn-checksum_hack] to avoid a compiler warning

[^fn-checksum_hack]: The formula from the table, `-(magic + architecture + header_length)`, creates a negative value that doesn't fit into 32bit. By subtracting from `0x100000000` (= 2^(32)) instead, we keep the value positive without changing its truncated value. Without the additional sign bit(s) the result fits into 32bit and the compiler is happy :).

We can already _assemble_ this file (which I called `multiboot_header.asm`) using `nasm`. It produces a flat binary by default, so the resulting file just contains our 24 bytes (in little endian if you work on a x86 machine):

```
> nasm multiboot_header.asm
> hexdump -x multiboot_header
0000000    50d6    e852    0000    0000    0018    0000    af12    17ad
0000010    0000    0000    0008    0000
0000018
```

## The Boot Code
To boot our kernel, we must add some code that the bootloader can call. Let's create a file named `boot.asm`:

```nasm
global start

section .text
bits 32
start:
    ; print `OK` to screen
    mov dword [0xb8000], 0x2f4b2f4f
    hlt
```
There are some new commands:

- `global` exports a label (makes it public). As `start` will be the entry point of our kernel, it needs to be public.
- the `.text` section is the default section for executable code
- `bits 32` specifies that the following lines are 32-bit instructions. It's needed because the CPU is still in [Protected mode] when GRUB starts our kernel. When we switch to [Long mode] in the [next post] we can use `bits 64` (64-bit instructions).
- the `mov dword` instruction moves the 32bit constant `0x2f4f2f4b` to the memory at address `b8000` (it prints `OK` to the screen, an explanation follows in the next posts)
- `hlt` is the halt instruction and causes the CPU to stop

Through assembling, viewing and disassembling we can see the CPU [Opcodes] in action:

[Opcodes]: https://en.wikipedia.org/wiki/Opcode

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


## Building the Executable
To boot our executable later through GRUB, it should be an [ELF] executable. So we want `nasm` to create ELF [object files] instead of plain binaries. To do that, we simply pass the `‑f elf64` argument to it.

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
[object files]: http://wiki.osdev.org/Object_Files

To create the ELF _executable_, we need to [link] the object files together. We use a custom [linker script] named `linker.ld`:

[link]: https://en.wikipedia.org/wiki/Linker_(computing)
[linker script]: https://sourceware.org/binutils/docs/ld/Scripts.html

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
- `. = 1M;` sets the load address of the first section to 1 MiB, which is a conventional place to load a kernel[^Linker 1M]
- the executable will have two sections: `.boot` at the beginning and `.text` afterwards
- the `.text` output section contains all input sections named `.text`
- Sections named `.multiboot_header` are added to the first output section (`.boot`) to ensure they are at the beginning of the executable. This is necessary because GRUB expects to find the Multiboot header very early in the file.

[^Linker 1M]: We don't want to load the kernel to e.g. `0x0` because there are many special memory areas below the 1MB mark (for example the so-called VGA buffer at `0xb8000`, that we use to print `OK` to the screen).

So let's create the ELF object files and link them using our new linker script. It's important to pass the `-n` flag to the linker, which disables the automatic section alignment in the executable. Otherwise the linker may page align the `.boot` section in the executable file. If that happens, GRUB isn't able to find the Multiboot header because it isn't at the beginning anymore. We can use `objdump` to print the sections of the generated executable and verify that the `.boot` section has a low file offset.

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
_Note_: The `ld` and `objdump` commands are platform specific. If you're _not_ working on x86_64 architecture, you will need to [cross compile binutils]. Then use `x86_64‑elf‑ld` and `x86_64‑elf‑objdump` instead of `ld` and `objdump`.
[cross compile binutils]: /cross-compile-binutils.html

## Creating the ISO
The last step is to create a bootable ISO image with GRUB. We need to create the following directory structure and copy the `kernel.bin` to the right place:

```
isofiles
└── boot
    ├── grub
    │   └── grub.cfg
    └── kernel.bin

```
The `grub.cfg` specifies the file name of our kernel and its Multiboot 2 compliance. It looks like this:

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
_Note_: `grub-mkrescue` causes problems on some platforms. If it does not work for you, try the following steps:

- try to run it with `--verbose`
- make sure `xorriso` is installed (`xorriso` or `libisoburn` package)
- If you're using an EFI-system, `grub-mkrescue` tries to create an EFI image by default. You can either pass `-d /usr/lib/grub/i386-pc` to avoid EFI or install the `mtools` package to get a working EFI image
- on some system the command is named `grub2-mkrescue`

## Booting
Now it's time to boot our OS. We will use [QEMU]:

[QEMU]: https://en.wikipedia.org/wiki/QEMU

```
qemu-system-x86_64 -cdrom os.iso
```
![qemu output](/images/qemu-ok.png)

Notice the green `OK` in the upper left corner. If it does not work for you, take a look at the comment section.

Let's summarize what happens:

1. the BIOS loads the bootloader (GRUB) from the virtual hard drive (the ISO)
2. the bootloader reads the kernel executable and finds the Multiboot header
3. it copies the `.boot` and `.text` sections to memory (to addresses `0x100000` and `0x100020`)
4. it jumps to the entry point (`0x100020`, you can obtain it through `objdump -f`)
5. our kernel prints the green `OK` and stops the CPU

You can test it on real hardware, too. Just burn the ISO to a disk or USB stick and boot from it.

## Build Automation

Right now we need to execute 4 commands in the right order everytime we change a file. That's bad. So let's automate the build using a [Makefile][Makefile tutorial]. But first we should create some clean directory structure for our source files to separate the architecture specific files:

[Makefile tutorial]: http://mrbook.org/blog/tutorials/make/

```
…
├── Makefile
└── src
    └── arch
        └── x86_64
            ├── multiboot_header.asm
            ├── boot.asm
            ├── linker.ld
            └── grub.cfg
```
The Makefile looks like this (but indented with tabs instead of spaces):

```Makefile
arch ?= x86_64
kernel := build/kernel-$(arch).bin
iso := build/os-$(arch).iso

linker_script := src/arch/$(arch)/linker.ld
grub_cfg := src/arch/$(arch)/grub.cfg
assembly_source_files := $(wildcard src/arch/$(arch)/*.asm)
assembly_object_files := $(patsubst src/arch/$(arch)/%.asm, \
    build/arch/$(arch)/%.o, $(assembly_source_files))

.PHONY: all clean run iso

all: $(kernel)

clean:
    @rm -r build

run: $(iso)
    @qemu-system-x86_64 -cdrom $(iso)

iso: $(iso)

$(iso): $(kernel) $(grub_cfg)
    @mkdir -p build/isofiles/boot/grub
    @cp $(kernel) build/isofiles/boot/kernel.bin
    @cp $(grub_cfg) build/isofiles/boot/grub
    @grub-mkrescue -o $(iso) build/isofiles 2> /dev/null
    @rm -r build/isofiles

$(kernel): $(assembly_object_files) $(linker_script)
    @ld -n -T $(linker_script) -o $(kernel) $(assembly_object_files)

# compile assembly files
build/arch/$(arch)/%.o: src/arch/$(arch)/%.asm
    @mkdir -p $(shell dirname $@)
    @nasm -felf64 $< -o $@
```
Some comments (see the [Makefile tutorial] if you don't know `make`):

- the `$(wildcard src/arch/$(arch)/*.asm)` chooses all assembly files in the src/arch/$(arch)` directory, so you don't have to update the Makefile when you add a file
- the `patsubst` operation for `assembly_object_files` just translates `src/arch/$(arch)/XYZ.asm` to `build/arch/$(arch)/XYZ.o`
- the `$<` and `$@` in the assembly target are [automatic variables]
- if you're using [cross-compiled binutils][cross compile binutils] just replace `ld` with `x86_64‑elf‑ld`

[automatic variables]: https://www.gnu.org/software/make/manual/html_node/Automatic-Variables.html

Now we can invoke `make` and all updated assembly files are compiled and linked. The `make iso` command also creates the ISO image and `make run` will additionally start QEMU.

## What's next?

In the [next post] we will create a page table and do some CPU configuration to switch to the 64-bit [long mode].

[next post]: {{ page.next.url }}
[long mode]: https://en.wikipedia.org/wiki/Long_mode
