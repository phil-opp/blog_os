+++
title = "A Minimal Rust Kernel"
order = 4
path = "minimal-rust-kernel"
date  = 0000-01-01
template = "second-edition/page.html"
+++

In this post we create a minimal 64-bit Rust kernel. We built upon the [freestanding Rust binary] from the previous post to create a bootable disk image, that prints something to the screen.

[freestanding Rust binary]: ./second-edition/posts/03-freestanding-rust-binary/index.md

<!-- more -->

TODO github, issues, comments, etc

## The Boot Process
When you turn on a computer, it begins executing firmware code that is stored in motherboard [ROM]. This code performs a [power-on self-test], detects available RAM, and pre-initializes the CPU and hardware. Afterwards it looks for a bootable disk and starts booting the operating system kernel.

[ROM]: https://en.wikipedia.org/wiki/Read-only_memory
[power-on self-test]: https://en.wikipedia.org/wiki/Power-on_self-test

On x86, there are two firmware standards: the “Basic Input/Output System“ (**[BIOS]**) and the newer “Unified Extensible Firmware Interface” (**[UEFI]**). The BIOS standard is old and outdated, but simple and well-supported on any x86 machine since the 1980s. UEFI, in contrast, is more modern and has much more features, but is more complex to set up (at least in my opinion).

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[UEFI]: https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

Currently, we only provide BIOS support, but support for UEFI is planned, too. If you'd like to help us with this, check out the [Github issue](https://github.com/phil-opp/blog_os/issues/349).

### BIOS Boot
Almost all x86 systems have support for BIOS booting, even newer UEFI-based machines (they include an emulated BIOS). This is great, because you can use the same boot logic across all machines from the last centuries. But this wide compatibility is at the same time the biggest disadvantage of BIOS booting, because it means that the CPU is put into a 16-bit compability mode called [real mode] before booting so that that arcane bootloaders from the 1980s would still work.

But let's start from the beginning:

When you turn on a computer, it loads the BIOS from some special flash memory located on the motherboard. The BIOS runs self test and initialization routines of the hardware, then it looks for bootable disks. If it finds one, the control is transferred to its _bootloader_, which is a 512-byte portion of executable code stored at the disk's beginning. Most bootloaders are larger than 512 bytes, so bootloaders are commonly split into a small first stage, which fits into 512 bytes, and a second stage, which is subsequently loaded by the first stage.

The bootloader has to determine the location of the kernel image on the disk and load it into memory. It also needs to switch the CPU from the 16-bit [real mode] first to the 32-bit [protected mode], and then to the 64-bit [long mode], where 64-bit registers and the complete main memory are available. Its third job is to query certain information (such as a memory map) from the BIOS and pass it to the OS kernel.

[real mode]: https://en.wikipedia.org/wiki/Real_mode
[protected mode]: https://en.wikipedia.org/wiki/Protected_mode
[long mode]: https://en.wikipedia.org/wiki/Long_mode
[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation

Writing a bootloader is a bit cumbersome as it requires assembly language and a lot of non insightful steps like “write this magic value to this processor register”. Therefore we don't cover bootloader creation in this post and instead provide a tool named [bootimage] that automatically appends a bootloader to your kernel.

[bootimage]: https://github.com/phil-opp/bootimage

If you are interested in building your own bootloader, check out our “[Booting]” posts, where we explain in detail how a bootloader is built.

[Booting]: TODO

### The Multiboot Standard

### UEFI

## A Minimal Kernel
Now that we know how a computer boots, it's time to create our own minimal kernel. Our goal is to create a bootable disk image that prints a green “OK” to the screen when booted. For that we build upon the [freestanding Rust binary] we created in the previous post.

We already have our `_start` entry point, which will be called by the boot loader. So let's output something to screen from it.

### Printing to Screen
The easiest way to print text to the screen at this stage is the [VGA text buffer]. It is a special memory area mapped to the VGA hardware that contains the contents displayed on screen. It normally consists of 50 lines that each contain 80 character cells. Each character cell displays an ASCII character with some foreground and background colors. The screen output looks like this:

[VGA text buffer]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode

![screen output for common ASCII characters](https://upload.wikimedia.org/wikipedia/commons/6/6d/Codepage-737.png)

We will discuss the exact layout of the VGA buffer in the next post, where we write a first small driver for it. For printing “OK”, we just need to know that the buffer is located at address `0xb8000` and that each character cell consists of an ASCII byte and a color byte.

So let's extend our `main.rs` to write `OK` to the screen:

```rust
#[no_mangle]
pub fn _start(boot_info: &'static mut BootInfo) -> ! {
	let vga_buffer = 0xb8000 as *const u8 as *mut u8;
    unsafe {
        *vga_buffer.offset(0) = b'O';
        *vga_buffer.offset(1) = 0xa; // foreground color green
        *vga_buffer.offset(0) = b'K';
        *vga_buffer.offset(3) = 0xa; // foreground color green
    }

	loop {}
}
```

First, we cast the integer `0xb8000` into a [raw pointer]. Then we use the [`offset`] method to write the first four bytes individually. We write the ASCII character `b'O'` (the `b` prefix creates an single-byte ASCII character instead of a four-byte Unicode character), then we write the color `0xa` (which translates to “green foreground, black background”). We repeat the same for the second character.

[raw pointer]: https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

Note that there's a big [`unsafe`] block around all memory writes. The reason is that the Rust compiler can't prove that the raw pointers we create are valid. They could point anywhere and lead to data corruption. By putting them into an `unsafe` block we're basically telling the compiler that we are absolutely sure that the operations are valid. Note that an `unsafe` block does not turn off Rust's safety checks. It only allows you to do [four additional things].

[`unsafe`]: https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html
[four additional things]: https://doc.rust-lang.org/stable/book/second-edition/ch19-01-unsafe-rust.html#unsafe-superpowers

I want to emphasize that **this is not the way we want to do things in Rust!** It's very easy to mess up when working with raw pointers inside unsafe blocks, for example, we could easily write behind the buffer's end if we're not careful.

So we want to minimize the use of `unsafe` as much as possible. Rust gives us the ability to do this by creating safe abstractions. For example, we could create a VGA buffer type that encapsulates all unsafety and ensures that it is _impossible_ to do anything wrong from the outside. This way, we would only need minimal amounts of `unsafe` and can be sure that we don't violate [memory safety].

[memory safety]: https://en.wikipedia.org/wiki/Memory_safety

We will create such a safe VGA buffer abstraction in the next post. For the remainder of post, we stick to our unsafe version to keep things simple.

Now that we have an executable that does something perceptible, it is time to turn it into a bootable disk image. However, in order to be able do that, we need to cross-compile our kernel to our target system.

### Target Specification
Until now, we compiled our kernel for the host system, that means the system you're currently running on. This could be a Windows machine with an ARM processor. Or a Mac with a 32-bit x86 processor. Or any other of the [many targets that Rust supports][platform-support]. Independent of the host system, we want to compile an executable for a bare-metal x86_64 system, which is our target system.

[platform-support]: https://forge.rust-lang.org/platform-support.html


TODO rewrite

We require some special configuration parameters for our target system (e.g. no underlying OS), so none of the [existing target triples][platform-support] fits. (A target triple describes the CPU architecture, the vendor, the operating system, and sometimes additionally the calling convention; “unknown” means that there is no reasonable value). Luckily Rust allows us to define our own target in a JSON file. For example, a JSON file that describes the `x86_64-unknown-linux-gnu` target looks like this:

```json
TODO
```

Most fields are required by LLVM to generate code for that platform. For example, the `data-layout` field defines the size of various integer, floating point, and pointer types. Then there are fields that Rust uses for conditional compilation, such as max_atomic_width. The third kind of fields are the most interesting: They define how the crate should be built. For example, features?

We also target `x86_64` systems with our kernel, so our target specification will look very similar to the `x86_64-unknown-linux-gnu` specification. Let's start by creating a `x86_64-unknown-blog_os.json` file (choose any name you like) with the common content:

```json

```

We add the following entries, where we changed the values:

```json

```

The reason for the change…

Finally, we add some additional entries:

```json

```

The xxx does yyy…

Our target specification file now looks like this:

```json

```

We can now build the kernel for our new target by passing the name of the JSON file (without extension) as `--target`. There's is currently an open bug with custom targets, so you also need to set the `RUST_TARGET_PATH` environment variable to the current directory, otherwise Rust might not be able to find your target. The full command is:

```
> RUST_TARGET_PATH=(pwd) cargo build --target x86_64-unknown-blog_os
```

### Creating a Bootimage
Now that we have an executable that does something perceptible, it is time to turn it into a bootable disk image. As we learned in the [section about booting], we need a bootloader for that, which initializes the CPU and loads our kernel.

[section about booting]: #the-boot-process

To make things easy, we created a tool named `bootimage` that automatically downloads and builds our bootloader, and combines it with the kernel executable to create a bootable disk image. To install it, execute `cargo install bootimage` in your terminal.

After installing, creating a bootimage is as easy as executing `bootimage --target x86_64-unknown-blog_os`.

You should now see a file named `bootimage.bin` in your crate root directory. This file is a bootable disk image, so can boot it in a virtual machine or copy it to an USB drive to boot it on real hardware. (Note that this is not a CD image (they have a different format), so burning to disk doesn't work).

## Booting it!

- qemu
- bochs? virtualbox?
- makefile? cargo-make?

## Summary & What's next?
