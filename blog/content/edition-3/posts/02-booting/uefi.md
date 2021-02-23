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

We start by creating a new `cargo` project with a `Cargo.toml` and a `src/main.rs`:

```toml
# in Cargo.toml

[package]
name = "uefi_app"
version = "0.1.0"
authors = ["Your Name <your-email@example.com>"]
edition = "2018"

[dependencies]
```

This `uefi_app` project is independent of the OS kernel created in the [_Booting_], so we use a separate directory.

[_Booting_]: @/edition-3/posts/02-booting/index.md

In the `src/main.rs`, we create a minimal `no_std` executable as shown in the [_Minimal Kernel_] post:

[_Minimal Kernel_]: @/edition-3/posts/01-minimal-kernel/index.md

```rust
// in src/main.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

The `#![no_std]` attribute disables the linking of the Rust standard library, which is not available on bare metal. The `#![no_main]` attribute, we disable the normal entry point function that based on the C runtime. The `#[panic_handler]` attribute specifies which function should be called when a panic occurs.

Next, we create an entry point function named `efi_main`:

```rust
// in src/main.rs

#![feature(abi_efiapi)]

use core::ffi::c_void;

#[no_mangle]
pub extern "efiapi" fn efi_main(
    image: *mut c_void,
    system_table: *const c_void,
) -> usize {
    loop {}
}
```

This function signature is standardized by the UEFI specification, which is available [in PDF form][uefi-pdf] on [uefi.org]. You can find the signature of the entry point function in section 4.1. Since UEFI also defines a specific [calling convention] (in section 2.3), we set the [`efiapi` calling convention] for our function. Since support for this calling function is still unstable in Rust, we need to add `#![feature(abi_efiapi)]` at the very top of our file.

[uefi-pdf]: https://uefi.org/sites/default/files/resources/UEFI%20Spec%202.8B%20May%202020.pdf
[uefi.org]: https://uefi.org/specifications
[calling convention]: https://en.wikipedia.org/wiki/Calling_convention
[`efiapi` calling convention]: https://github.com/rust-lang/rust/issues/65815

The function takes two arguments: an _image handle_ and a _system table_. The image handle is a firmware-allocated handle that identifies the UEFI image. The system table contains some input and output handles and provides access to various functions provided by the UEFI firmware. The function returns an `EFI_STATUS` integer to signal whether the function was successful. It is normally only returned by UEFI apps that are not bootloaders, e.g. UEFI drivers or apps that are launched manually from the UEFI shell. Bootloaders typically pass control to a OS kernel and never return.

### UEFI Target

For our minimal kernel, we needed to create a [custom target] because none of the [officially supported targets] was suitable. For our UEFI application we are more lucky: Rust has built-in support for a **`x86_64-unknown-uefi`** target, which we can use without problems.

[custom target]: @/edition-3/posts/01-minimal-kernel/index.md#kernel-target
[officially supported targets]: https://doc.rust-lang.org/rustc/platform-support.html

 If you're curious, you can query the JSON specification of the target with the following command:

```bash
rustc +nightly --print target-spec-json -Z unstable-options --target x86_64-unknown-uefi
```

This outputs looks something like the following:

```json
{
  "abi-return-struct-as-int": true,
  "allows-weak-linkage": false,
  "arch": "x86_64",
  "code-model": "large",
  "cpu": "x86-64",
  "data-layout": "e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128",
  "disable-redzone": true,
  "emit-debug-gdb-scripts": false,
  "exe-suffix": ".efi",
  "executables": true,
  "features": "-mmx,-sse,+soft-float",
  "is-builtin": true,
  "is-like-msvc": true,
  "is-like-windows": true,
  "linker": "rust-lld",
  "linker-flavor": "lld-link",
  "lld-flavor": "link",
  "llvm-target": "x86_64-unknown-windows",
  "max-atomic-width": 64,
  "os": "uefi",
  "panic-strategy": "abort",
  "pre-link-args": {
    "lld-link": [
      "/NOLOGO",
      "/NXCOMPAT",
      "/entry:efi_main",
      "/subsystem:efi_application"
    ],
    "msvc": [
      "/NOLOGO",
      "/NXCOMPAT",
      "/entry:efi_main",
      "/subsystem:efi_application"
    ]
  },
  "singlethread": true,
  "split-debuginfo": "packed",
  "stack-probes": {
    "kind": "call"
  },
  "target-pointer-width": "64"
}
```

From the output we can derive multiple properties of the target:

- The `exe-suffix` is `.efi`, which means that all executables compiled for this target have the suffix `.efi`.
- As for our [kernel target][custom target], both the redzone and SSE are disabled.
- The `is-like-windows` is an indicator that the target uses the conventions of Windows world, e.g. [PE] instead of [ELF] executables.
- The LLD linker is used, which means that we don't have to install any additional linker even when compiling on non-Windows systems.
- Like for all (most?) bare-metal targets, the `panic-strategy` is set to `abort` to disable unwinding.
- Various linker arguments are specified. For example, the `/entry` argument sets the name of the entry point function. This is the reason that we named our entry point function `efi_main` and applied the `#[no_mangle]` attribute above.

[PE]: https://en.wikipedia.org/wiki/Portable_Executable
[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format

If you're interested in understanding all these fields, check out the docs for Rust's internal [`Target`] and [`TargetOptions`] types. These are the types that the above JSON is converted to.

[`Target`]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_target/spec/struct.Target.html
[`TargetOptions`]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_target/spec/struct.TargetOptions.html

### Building

Even though the `x86_64-unknown-uefi` target is built-in, there are no precompiled versions of the `core` library available for it. This means that we need to use cargo's [`build-std` feature] as described in the [_Minimal Kernel_][minimal-kernel-build-std] post.

[`build-std` feature]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std
[minimal-kernel-build-std]: @/edition-3/posts/01-minimal-kernel/index.md#the-build-std-option

A nightly Rust compiler is required for building, so we need to set up a [rustup override] for the directory. We can do this either by running a [`rustup ovrride` command] or by adding a [`rust-toolchain` file].

[rustup override]: https://rust-lang.github.io/rustup/overrides.html
[`rustup override` command]: https://rust-lang.github.io/rustup/overrides.html#directory-overrides
[`rust-toolchain` file]: https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file

The full build command looks like this:

```bash
cargo build --target x86_64-unknown-uefi -Z build-std=core \
    -Z build-std-features=compiler-builtins-mem
```

This results in a `uefi_app.efi` file in our `x86_64-unknown-uefi/debug` folder. Congratulations! We just created our own minimal UEFI app.

## Bootable Disk Image

To make our minimal UEFI app bootable, we need to create a new [GPT] disk image with a [EFI system partition]. On that partition, we need to put our `.efi` file under `efi\boot\bootx64.efi`. Then the UEFI firmware should automatically detect and load it when we boot from the corresponding disk. See the section about the [UEFI boot process][uefi-boot-process] in the _Booting_ post for more details.

[GPT]: https://en.wikipedia.org/wiki/GUID_Partition_Table
[EFI system partition]: https://en.wikipedia.org/wiki/EFI_system_partition
[uefi-boot-process]:  @/edition-3/posts/02-booting/index.md#boot-process-1

To create this disk image, we create a new `disk_image` executable:

```bash
> cargo new --bin disk_image
```

This creates a new cargo project in a `disk_image` subdirectory. To share the `target` folder and `Cargo.lock` file with our `uefi_app` project, we set up a cargo workspace:

```toml
# in Cargo.toml

[workspace]
members = ["disk_image"]
```

### FAT Partition

The first step is to create an EFI system partition formatted with the [FAT] file system. The reason for using FAT is that this is the only file system that the UEFI standard requires. In practice, most UEFI firmware implementations also support the [NTFS] filesystem, but we can't rely on that since this is not required by the standard.

[FAT]: https://en.wikipedia.org/wiki/File_Allocation_Table
[NTFS]: https://en.wikipedia.org/wiki/NTFS

To create a new FAT file system, we use the [`fatfs`] crate:

[`fatfs`]: https://docs.rs/fatfs/0.3.5/fatfs/

```toml
# in disk_image/Cargo.toml

[dependencies]
fatfs = "0.3.5"
```

TODO

### GPT Disk Image


### Running

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
