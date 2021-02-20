+++
title = "UEFI Booting"
path = "booting/uefi"
date = 0000-01-01
template = "edition-3/page.html"

[extra]
hide_next_prev = true
icon = '''
<!-- icon source: https://de.wikipedia.org/wiki/Datei:Uefi_logo.svg -->
<svg baseProfile="tiny" xmlns="http://www.w3.org/2000/svg" viewBox = "0 0 367.92 424.8">
<path fill="#FFF" d="M183.505 7.5l12.515.016 59.87 34.233.632 13.683 23.938.38L339.524 89.6l16.386 30.31 5.136 192.808L349.92 329.3l-56.88 32.657-19.564-1.81-13.315 20.69-56.41 32.404-89.687-32.764L4.375 312.71 7.5 109.59z"/>
<path fill="#DC0000" d="M182.88 0l13.14 7.516-86.427 50.52S83.443 71.21 74.16 81.362c-11.362 12.428-7.917 30.125 2.16 42.48 24.693 30.28 88.66 54.367 141.12 34.56C239.666 150.01 339.524 89.6 339.524 89.6l28.397 16.243v213.12l-18 10.337V207.36l-56.88 32.66v121.937l-32.88 18.88V311.04l20.28-12.24v-51.543l-20.28 11.646s-2.37-32.09 1.92-42.902c4.1-10.31 15.74-21.72 25.2-18.72 6.95 2.21 5.76 24.95 5.76 24.95s42.95-24.85 56.88-32.86c2.25-36.34-9.13-59-43.92-55.44-15.87 1.63-28.37 10.02-38.88 17.28-11.14 7.7-20.4 16.555-28.8 26.64-15.89 19.1-33.02 45.26-35.28 76.32-1.77 24.357.71 159.07.71 159.07L183.6 424.8 0 318.96V105.84L182.88 0zM115.2 167.04c-13.318-10.95-29.718-21.208-47.52-25.2-11.942-2.678-23.93-1.128-32.4 3.6-22.328 12.466-28.844 45.437-26.64 77.76 3.508 51.445 22.065 86.146 48.96 113.04 17.977 17.977 47.576 39.66 74.16 41.76 27.702 2.187 36.335-16.023 42.48-36.72-20.956-14.324-44.265-26.296-65.52-40.32-3.91 2.99-3.572 6.328-9.36 6.48-5.15.135-10.955-4.727-14.4-9.36-6.09-8.19-8.026-21.054-8.64-30.96 33.78 18.062 66.363 37.317 100.08 55.44 3.688-67.27-23.104-124.2-61.2-155.52zM280.46 55.813l-85.795 52.732s-22.85 14.813-38.136 13.134c-4.99-.55-13.31-4.77-13.68-8.64-.7-7.16 25.2-21.02 25.2-21.02l87.84-50.27L280.46 55.8zM109.44 241.2c-11.23-5.81-21.966-12.114-32.4-18.72 1.032-7.922 2.438-15.645 12.24-13.68 11.49 2.303 19.817 20.686 20.16 32.4z"/>
</svg>
'''
+++

This post is an addendum to our main [**Booting**] post. It explains how to create a basic UEFI bootloader from scratch.

[**Booting**]: @/edition-3/posts/02-booting/index.md

<!-- more -->

This blog is openly developed on [GitHub]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom].

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments

<!-- toc -->

## Minimal UEFI App

```rust
use core::ffi::c_void;

#[no_mangle]
pub extern "efiapi" fn efi_main(
    image: *mut c_void,
    system_table: *const c_void,
) -> usize {
    loop {}
}
```

We don't need to create a custom target because Rust has a built-in target for the UEFI environment named `x86_64-unknown-uefi`. We can use `rustc TODO` to print this target as JSON:

```jsonc
// TODO
```

We see that the target sets the entry point to a function named `efi_main`. This is the reason that we chose this name for our entry function above. The target also defines that PE executables should be created.

To compile our project, we need to use cargo's `build-std` and `build-std-features` arguments because Rust does not ship a precompiled version of `core` crate for the UEFI target. For more details, see our [_Minimal Kernel_] post.

The full build command looks like this:

```bash
cargo build --target x86_64-unknown-uefi -Z build-std=core -Z build-std-features=TODO
```

This results in a `.efi` file in our `target/TODO` folder.

## Bootable Disk Image

To make our minimal UEFI app bootable, we need to create a new [GPT] disk image with a [EFI system partition]. On that partition, we need to put our `.efi` file under `TODO`. Then the UEFI firmware should automatically detect and load it when we boot from the corresponding disk.

To create this disk image, we create a new `disk_image` executable:

TODO

TODO: use the `gpt` and `fat32` crates to create the partitions

The reason for using a FAT32 partition is that this is the only partition type that the UEFI standard requires. In practice, most UEFI firmware implementations also support the NTFS filesystem, but we can't rely on that since this is not required by the standard.

Now we can run our `disk_image` executable to create the bootable disk image:

TODO

This results in a `.fat` and a `.img` file next to our `.efi` executable. These files can be launched in QEMU and on real hardware as described in the main [_Booting_] post. However, we don't see anything on the screen yet since we only `loop {}` in our `efi_main`:

TODO screenshot

Let's fix this by using the `uefi` crate.

## The `uefi` Crate

In order to print something to the screen, we need to call some functions provided by the UEFI firmware. These functions can be invoked through the `system_table` argument passed to our `efi_main` function. This table provides [function pointers] for all kinds of functionality, including access to the screen, disk, or network.

Since the system table has a standardized format that is identical on all systems, it makes sense to create an abstraction for it. This is what the `uefi` crate does. It provides a [`SystemTable`] type that abstracts the UEFI system table functions as normal Rust methods. It is not complete, but the most important functions are all available.

To use the crate, we first add it as a dependency in our `Cargo.toml`:

```toml
# TODO
```

Now we can change the types of the `image` and `system_table` arguments in our `efi_main` declaration:

```rust
// TODO
```

Since the Rust compiler is not able to typecheck the function signature of the entry point function, we could accidentally use the wrong signature here. To prevent this (and the resulting undefined behavior), the `uefi` crate provides an `entry` macro to enforce the correct signature. To use it, we change our `main.rs` like this:

```rust
// TODO
```

Now we can safely use the types provided by the `uefi` crate.

### Printing to Screen

The UEFI standard supports multiple interfaces for printing to the screen. The most simple one is the text-based TODO. To use it, ... TODO.

The text-based output is only available before exiting UEFI boot services. TODO explain

The UEFI standard also supports a pixel-based framebuffer for screen output through the GOP protocol. This framebuffer also stays available after exiting boot services, so it makes sense to set it up before switching to the kernel. The protocol can be set up like this:

TODO

See the [TODO] post for how to draw and render text using this framebuffer.

### Memory Allocation

### Physical Memory Map

### APIC Base

## Loading the Kernel

We already saw how to set up a framebuffer for screen output and query the physical memory map and the APIC base register address. This is already all the system information that a basic kernel needs from the bootloader.

The next step is to load the kernel executable. This involves loading the kernel from disk into memory, allocating a stack for it, and setting up a new page table hierarchy to properly map it to virtual memory.

### Loading it from Disk

One approach for including our kernel could be to place it in the FAT partition created by our `disk_image` crate. Then we could use the TODO protocol of the `uefi` crate to load it from disk into memory.

To keep things simple, we will use a different appoach here. Instead of loading the kernel separately, we place its bytes as a `static` variable inside our bootloader executable. This way, the UEFI firmware directly loads it into memory when launching the bootloader. To implement this, we can use the [`include_bytes`] macro of Rust's `core` library:

```rust
// TODO
```

### Parsing the Kernel

Now that we have our kernel executable in memory, we need to parse it. In the following, we assume that the kernel uses the ELF executable format, which is popular in the Linux world. This is also the excutable format that the kernel created in this blog series uses.

The ELF format is structured like this:

TODO

The various headers are useful in different situations. For loading the executable into memory, the _program header_ is most relevant. It looks like this:

TODO

TODO: mention readelf/objdump/etc for looking at program header

There are already a number of ELF parsing crates in the Rust ecosystem, so we don't need to create our own. In the following, we will use the [`xmas_elf`] crate, but other crates might work equally well.

TODO: load program segements and print them

TODO: .bss section -> mem_size might be larger than file_size

### Page Table Mappings

TODO:

- create new page table
- map each segment
    - special-case: mem_size > file_size

### Create a Stack

## Switching to Kernel

## Challenges

### Boot Information

- Physical Memory

### Integration in Build System

### Common Interface with BIOS

### Configurability
