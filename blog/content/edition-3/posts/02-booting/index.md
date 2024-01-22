+++
title = "Booting"
weight = 2
path = "booting"
date = 0000-01-01
draft = true

[extra]
chapter = "Bare Bones"
icon = '''
<svg xmlns="http://www.w3.org/2000/svg" fill="currentColor" class="bi bi-power" viewBox="0 0 16 16">
  <path d="M7.5 1v7h1V1h-1z"/>
  <path d="M3 8.812a4.999 4.999 0 0 1 2.578-4.375l-.485-.874A6 6 0 1 0 11 3.616l-.501.865A5 5 0 1 1 3 8.812z"/>
</svg>
'''

extra_content = ["uefi/index.md"]
+++

In this post, we explore the boot process on both BIOS and UEFI-based systems.
We combine the [minimal kernel] created in the previous post with a bootloader to create a bootable disk image.
We then show how this image can be started in the [QEMU] emulator and run on real hardware.

[minimal kernel]: @/edition-3/posts/01-minimal-kernel/index.md
[QEMU]: https://www.qemu.org/

<!-- more -->

This blog is openly developed on [GitHub].
If you have any problems or questions, please open an issue there.
You can also leave comments [at the bottom].
The complete source code for this post can be found in the [`post-3.2`][post branch] branch.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-3.2

<!-- toc -->

## The Boot Process
When you turn on a computer, it begins executing firmware code that is stored in motherboard [ROM].
This code performs a [power-on self-test], detects available RAM, and pre-initializes the CPU and other hardware.
Afterwards it looks for a bootable disk and starts booting the operating system kernel.

[ROM]: https://en.wikipedia.org/wiki/Read-only_memory
[power-on self-test]: https://en.wikipedia.org/wiki/Power-on_self-test

On x86, there are two firmware standards: the “Basic Input/Output System“ (**[BIOS]**) and the newer “Unified Extensible Firmware Interface” (**[UEFI]**).
The BIOS standard is outdated and not standardized, but relatively simple and supported on almost any x86 machine since the 1980s.
UEFI, in contrast, is more modern and has much more features, but also more complex and only runs on fairly recent hardware (built since ~2012).

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[UEFI]: https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

### BIOS

Almost all x86 systems have support for BIOS booting, including most UEFI-based machines that support an emulated BIOS.
This is great, because you can use the same boot logic across all machines from the last centuries.
The drawback is that the standard is very old, for example the CPU is put into a 16-bit compatibility mode called [real mode] before booting so that archaic bootloaders from the 1980s would still work.
Also, BIOS-compatibility will be slowly removed on newer UEFI machines over the next years (see below).

#### Boot Process

When you turn on a BIOS-based computer, it first loads the BIOS firmware from some special flash memory located on the motherboard.
The BIOS runs self test and initialization routines of the hardware, then it looks for bootable disks.
For that it loads the first disk sector (512 bytes) of each disk into memory, which contains the [_master boot record_] (MBR) structure.
This structure has the following general format:

[_master boot record_]: https://en.wikipedia.org/wiki/Master_boot_record

| Offset | Field             | Size |
| ------ | ----------------- | ---- |
| 0      | bootstrap code    | 446  |
| 446    | partition entry 1 | 16   |
| 462    | partition entry 2 | 16   |
| 478    | partition entry 3 | 16   |
| 444    | partition entry 4 | 16   |
| 510    | boot signature    | 2    |

The bootstrap code is commonly called the _bootloader_ and responsible for loading and starting the operating system kernel.
The four partition entries describe the [disk partitions] such as the `C:` partition on Windows.
The boot signature field at the end of the structure specifies whether this disk is bootable or not.
If it is bootable, the signature field must be set to the [magic bytes] `0xaa55`.
It's worth noting that there are [many extensions][mbr-extensions] of the MBR format, which for example include a 5th partition entry or a disk signature.

[disk partitions]: https://en.wikipedia.org/wiki/Disk_partitioning
[magic bytes]: https://en.wikipedia.org/wiki/Magic_number_(programming)
[mbr-extensions]: https://en.wikipedia.org/wiki/Master_boot_record#Sector_layout

The BIOS itself only cares for the boot signature field.
If it finds a disk with a boot signature equal to `0xaa55`, it directly passes control to the bootloader code stored at the beginning of the disk.
This bootloader is then responsible for multiple things:

- **Loading the kernel from disk:** The bootloader has to determine the location of the kernel image on the disk and load it into memory.
- **Initializing the CPU:** As noted above, all `x86_64` CPUs start up in a 16-bit [real mode] to be compatible with older operating systems.
So in order to run current 64-bit operating systems, the bootloader needs to switch the CPU from the 16-bit [real mode] first to the 32-bit [protected mode], and then to the 64-bit [long mode], where all CPU registers and the complete main memory are available.
- **Querying system information:** The third job of the bootloader is to query certain information from the BIOS and pass it to the OS kernel.
This, for example, includes information about the available main memory and graphical output devices.
- **Setting up an execution environment:** Kernels are typically stored as normal executable files (e.g. in the [ELF] or [PE] format), which require some loading procedure.
This includes setting up a [call stack] and a [page table].

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
[PE]: https://en.wikipedia.org/wiki/Portable_Executable
[call stack]: https://en.wikipedia.org/wiki/Call_stack
[real mode]: https://en.wikipedia.org/wiki/Real_mode
[protected mode]: https://en.wikipedia.org/wiki/Protected_mode
[long mode]: https://en.wikipedia.org/wiki/Long_mode
[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation
[page table]: https://en.wikipedia.org/wiki/Page_table

Some bootloaders also include a basic user interface for [choosing between multiple installed OSs][multi-booting] or entering a recovery mode.
Since it is not possible to do all that within the available 446 bytes, most bootloaders are split into a small first stage, which is as small as possible, and a second stage, which is subsequently loaded by the first stage.

[multi-booting]: https://en.wikipedia.org/wiki/Multi-booting

Writing a BIOS bootloader is cumbersome as it requires assembly language and a lot of non insightful steps like _“write this magic value to this processor register”_.
Therefore we don't cover bootloader creation in this post and instead use the existing [`bootloader`] crate to make our kernel bootable.

(If you are interested in building your own BIOS bootloader, you can look through the [BIOS source code] of the `bootloader` crate on GitHub, which is mostly written in Rust and has only about 50 lines of assembly code.)

[BIOS source code]: https://github.com/rust-osdev/bootloader/tree/main/bios

#### The Future of BIOS

As noted above, most modern systems still support booting operating systems written for the legacy BIOS firmware for backwards-compatibility.
However, there are [plans to remove this support soon][end-bios-support].
Thus, it is strongly recommended to make operating system kernels compatible with the newer UEFI standard too.
Fortunately, it is possible to create a kernel that supports booting on both BIOS (for older systems) and UEFI (for modern systems).

[end-bios-support]: https://arstechnica.com/gadgets/2017/11/intel-to-kill-off-the-last-vestiges-of-the-ancient-pc-bios-by-2020/

### UEFI

The Unified Extensible Firmware Interface (UEFI) replaces the classical BIOS firmware on most modern computers.
The specification provides lots of useful features that make bootloader implementations much simpler:

- It supports initializing the CPU directly into 64-bit mode, instead of starting in a DOS-compatible 16-bit mode like the BIOS firmware.
- It understands disk partitions and executable files.
Thus it is able to fully load the bootloader from disk into memory (no 512-byte "first stage" is required anymore).
- A standardized [specification][uefi-specification] minimizes the differences between systems.
This isn't the case for the legacy BIOS firmware, so that bootloaders often have to try different methods because of hardware differences.
- The specification is independent of the CPU architecture, so that the same interface can be used to boot on `x86_64` and e.g. `ARM` CPUs.
- It natively supports network booting without requiring additional drivers.

[uefi-specification]: https://uefi.org/specifications

The UEFI standard also tries to make the boot process safer through a so-called _"secure boot"_ mechanism.
The idea is that the firmware only allows loading bootloaders that are signed by a trusted [digital signature].
Thus, malware should be prevented from compromising the early boot process.

[digital signature]: https://en.wikipedia.org/wiki/Digital_signature

#### Issues & Criticism

While most of the UEFI specification sounds like a good idea, there are also many issues with the standard.
The main issue for most people is the fear that the _secure boot_ mechanism could be used to lock users into a specific operating system (e.g. Windows) and thus prevent the installation of alternative operating systems.

Another point of criticism is that the large number of features make the UEFI firmware very complex, which increases the chance that there are some bugs in the firmware implementation itself.
This can lead to security problems because the firmware has complete control over the hardware.
For example, a vulnerability in the built-in network stack of an UEFI implementation can allow attackers to compromise the system and e.g. silently observe all I/O data.
The fact that most UEFI implementations are not open-source makes this issue even more problematic, since there is no way to audit the firmware code for potential bugs.

While there are open firmware projects such as [coreboot] that try to solve these problems, there is no way around the UEFI standard on most modern consumer computers.
So we have to live with these drawbacks for now if we want to build a widely compatible bootloader and operating system kernel.

[coreboot]: https://www.coreboot.org/

#### Boot Process

The UEFI boot process works in the following way:

- After powering on and self-testing all components, the UEFI firmware starts looking for special bootable disk partitions called [EFI system partitions].
These partitions must be formatted with the [FAT file system] and assigned a special ID that indicates them as EFI system partition.
The UEFI standard understands both the [MBR] and [GPT] partition table formats for this, at least theoretically.
In practice, some UEFI implementations seem to [directly switch to BIOS-style booting when an MBR partition table is used][mbr-csm], so it is recommended to only use the GPT format with UEFI.
- If the firmware finds an EFI system partition, it looks for an executable file named `efi\boot\bootx64.efi` (on x86_64 systems).
This executable must use the [Portable Executable (PE)] format, which is common in the Windows world.
- It then loads the executable from disk to memory, sets up the execution environment (CPU state, page tables, etc.) in a standardized way, and finally jumps to the entry point of the loaded executable.

[MBR]: https://en.wikipedia.org/wiki/Master_boot_record
[GPT]: https://en.wikipedia.org/wiki/GUID_Partition_Table
[mbr-csm]: https://bbs.archlinux.org/viewtopic.php?id=142637
[EFI system partitions]: https://en.wikipedia.org/wiki/EFI_system_partition
[FAT file system]: https://en.wikipedia.org/wiki/File_Allocation_Table
[Portable Executable (PE)]: https://en.wikipedia.org/wiki/Portable_Executable

From this point on, the loaded executable has control.
Typically, this executable is a bootloader that then loads the actual operating system kernel.
Theoretically, it would also be possible to let the UEFI firmware load the kernel directly without a bootloader in between, but this would make it more difficult to port the kernel to other architectures.

Bootloaders and kernels typically need additional information about the system, for example the amount of available memory.
For this reason, the UEFI firmware passes a pointer to a special _system table_ as an argument when invoking the bootloader entry point function.
Using this table, the bootloader can query various system information and even invoke special functions provided by the UEFI firmware, for example for accessing the hard disk.

#### How we will use UEFI

As it is probably clear at this point, the UEFI interface is very powerful and complex.
The wide range of functionality makes it even possible to write an operating system directly as an UEFI application, using the UEFI services provided by the system table instead of creating own drivers.
In practice, however, most operating systems use UEFI only for the bootloader since own drivers give you better performance and more control over the system.
We will also follow this path for our OS implementation.

To keep this post focused, we won't cover the creation of an UEFI bootloader here.
Instead, we will use the already mentioned [`bootloader`] crate, which allows loading our kernel on both UEFI and BIOS systems.
If you're interested in how to create an UEFI bootloader yourself, check out our extra post about [**UEFI Booting**].

[**UEFI Booting**]: @/edition-3/posts/02-booting/uefi/index.md

### The Multiboot Standard

To avoid that every operating system implements its own bootloader that is only compatible with a single OS, the [Free Software Foundation] created an open bootloader standard called [Multiboot] in 1995.
The standard defines an interface between the bootloader and operating system, so that any Multiboot compliant bootloader can load any Multiboot compliant operating system on both BIOS and UEFI systems.
The reference implementation is [GNU GRUB], which is the most popular bootloader for Linux systems.

[Free Software Foundation]: https://en.wikipedia.org/wiki/Free_Software_Foundation
[Multiboot]: https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html
[GNU GRUB]: https://en.wikipedia.org/wiki/GNU_GRUB

To make a kernel Multiboot compliant, one just needs to insert a so-called [Multiboot header] at the beginning of the kernel file.
This makes it very easy to boot an OS in GRUB.
However, GRUB and the Multiboot standard have some issues too:

[Multiboot header]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format

- The standard is designed to make the bootloader simple instead of the kernel.
For example, the kernel needs to be linked with an [adjusted default page size], because GRUB can't find the Multiboot header otherwise.
Another example is that the [boot information], which is passed to the kernel, contains lots of architecture-dependent structures instead of providing clean abstractions.
- The standard supports only the 32-bit protected mode on BIOS systems.
This means that you still have to do the CPU configuration to switch to the 64-bit long mode.
- For UEFI systems, the standard provides very little added value as it simply exposes the normal UEFI interface to kernels.
- Both GRUB and the Multiboot standard are only sparsely documented.
- GRUB needs to be installed on the host system to create a bootable disk image from the kernel file.
This makes development on Windows or Mac more difficult.

[adjusted default page size]: https://wiki.osdev.org/Multiboot#Multiboot_2
[boot information]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format

Because of these drawbacks we decided to not use GRUB or the Multiboot standard for this series.
However, we might add Multiboot support to our [`bootloader`] crate at some point, so that it becomes possible to load your kernel on a GRUB system too.
If you're interested in writing a Multiboot compliant kernel, check out the [first edition] of this blog series.

[first edition]: @/edition-1/_index.md

## Bootable Disk Image

We now know that most operating system kernels are loaded by bootloaders, which are small programs that initialize the hardware to reasonable defaults, load the kernel from disk, and provide it with some fundamental information about the underlying system.
In this section, we will learn how to combine the [minimal kernel] we created in the previous post with the `bootloader` crate in order to create a bootable disk image.

The [`bootloader`] crate supports both BIOS and UEFI booting on `x86_64` and creates a reasonable default execution environment for our kernel.
This way, we can focus on the actual kernel design in the following posts instead of spending a lot of time on system initialization.

### The `bootloader_api` Crate

In order to make our kernel compatible with the `bootloader` crate, we first need to add a dependency on the [`bootloader_api`] crate:

[`bootloader`]: https://docs.rs/bootloader/latest/bootloader/
[`bootloader_api`]: https://docs.rs/bootloader_api/latest/bootloader_api/

```toml,hl_lines=4
# in Cargo.toml

[dependencies]
bootloader_api = "0.11.2"
```

Now we need to replace our custom `_start` entry point function with [`bootloader_api::entry_point`] macro. This macro instructs the compiler to create a special `.bootloader-config` section with encoded configuration options in the resulting executable, which is later read by the bootloader implementation.

[`bootloader_api::entry_point`]: https://docs.rs/bootloader_api/latest/bootloader_api/macro.entry_point.html

We will take a closer look at the `entry_point` macro and the different configuration options later. For now, we just use the default setup:

```rust,hl_lines=3 6-8
// in main.rs

bootloader_api::entry_point!(kernel_main);

// ↓ this replaces the `_start` function ↓
fn kernel_main(_boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    loop {}
}
```

There are a few notable things:

- The `kernel_main` function is just a normal Rust function with an arbitrary name. No `#[no_mangle]` attribute is needed anymore since the `entry_point` macro handles this internally.
- Like before, our entry point function is [diverging], i.e. it must never return. We ensure this by looping endlessly.
- There is a new [`BootInfo`] argument, which the bootloader fills with various system information. We will use this argument later. For now, we prefix it with an underscore to avoid an "unused variable" warning.
- The `entry_point` macro verifies that the `kernel_main` function has the correct arguments and return type, otherwise a compile error will occur. This is important because undefined behavior might occur when the function signature does not match the bootloader's expectations.

[diverging]: https://doc.rust-lang.org/rust-by-example/fn/diverging.html
[`BootInfo`]: https://docs.rs/bootloader_api/latest/bootloader_api/info/struct.BootInfo.html

To verify that the `entry_point` macro worked as expected, we can use the `objdump` tool as [described in the previous post][objdump-prev]. First, we recompile using `cargo build --target x86_64-unknown-none`, then we inspect the section headers using `objdump` or `rust-objdump`:

[objdump-prev]: @/edition-3/posts/01-minimal-kernel/index.md#inspect-elf-file-using-objdump

```bash,hl_lines=8
❯ rust-objdump -h target/x86_64-unknown-none/debug/kernel

target/x86_64-unknown-none/debug/kernel:        file format elf64-x86-64

Sections:
Idx Name               Size     VMA              Type
  0                    00000000 0000000000000000
  1 .bootloader-config 0000007c 0000000000200120 DATA
  2 .text              00000075 00000000002011a0 TEXT
  3 .debug_abbrev      000001c8 0000000000000000 DEBUG
  4 .debug_info        00000b56 0000000000000000 DEBUG
  5 .debug_aranges     00000090 0000000000000000 DEBUG
  6 .debug_ranges      00000040 0000000000000000 DEBUG
  7 .debug_str         00000997 0000000000000000 DEBUG
  8 .debug_pubnames    0000014c 0000000000000000 DEBUG
  9 .debug_pubtypes    00000548 0000000000000000 DEBUG
 10 .debug_frame       000000b0 0000000000000000 DEBUG
 11 .debug_line        0000012c 0000000000000000 DEBUG
 12 .comment           00000013 0000000000000000
 13 .symtab            000000a8 0000000000000000
 14 .shstrtab          000000b8 0000000000000000
 15 .strtab            000000cd 0000000000000000
```

We see that there is indeed a new `.bootloader-config` section of size `0x7c` in our kernel executable.
This means that we can now look into how to create a bootable disk image from our kernel.

### Creating a Disk Image

Now that our kernel is compatible with the `bootloader` crate, we can turn it into a bootable disk image.
To do that, we need to create a disk image file with an [MBR] or [GPT] partition table and create a new [FAT][FAT file system] boot partition there.
Then we copy our compiled kernel and the compiled bootloader to this boot partition.

While we could perform these steps manually using platform-specific tools (e.g. [`mkfs`] on Linux), this would be cumbersome to use and difficult to set up.
Fortunately, the `bootloader` crate provides a cross-platform [`DiskImageBuilder`] type to construct BIOS and UEFI disk images.
We just need to pass path to our kernel executable and then call [`create_bios_image`] and/or [`create_uefi_image`] with our desired target path.

[`mkfs`]: https://www.man7.org/linux/man-pages/man8/mkfs.fat.8.html
[`DiskImageBuilder`]: https://docs.rs/bootloader/0.11.3/bootloader/struct.DiskImageBuilder.html
[`create_bios_image`]: https://docs.rs/bootloader/0.11.3/bootloader/struct.DiskImageBuilder.html#method.create_bios_image
[`create_uefi_image`]: https://docs.rs/bootloader/0.11.3/bootloader/struct.DiskImageBuilder.html#method.create_uefi_image

By using the `DiskImageBuilder` together with some advanced features of `cargo`, we can combine the kernel build and disk image creation steps.
Another advantage of this approach is that we don't need to pass the `--target x86_64-unknown-none` argument anymore.
In the next sections, we will implement following steps to achieve this:

- Create a [`cargo` workspace] with an empty root package.
- Add an [_artifact dependency_] to include the compiled kernel binary in the root package.
- Invoke the `bootloader::DiskImageBuilder` in the root package.

[`cargo` workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html
[_artifact dependency_]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#artifact-dependencies

Don't worry if that sounds a bit complex!
We will explain each of these steps in detail.

#### Creating a Workspace

Cargo provides a feature named [_workspaces_] to manage projects that consistent of multiple crates.
The idea is that the crates share a single `Cargo.lock` file (to pin dependencies) and a common `target` folder.
The different crates can depend on each other by specifying [`path` dependencies].

[_workspaces_]: https://doc.rust-lang.org/cargo/reference/workspaces.html
[`path` dependencies]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#specifying-path-dependencies

Creating a cargo workspace is easy. We first create a new subfolder named `kernel` and move our existing `Cargo.toml` file and `src` folder there.
We keep the `Cargo.lock` file and the `target` folder in the outer level, `cargo` will update them automatically.
The folder structure should look like this now:

```bash ,hl_lines=3-6
.
├── Cargo.lock
├── kernel
│   ├── Cargo.toml
│   └── src
│       └── main.rs
└── target
```

Next, we create a new `blog_os` crate at the root using `cargo init`:

```bash
❯ cargo init --name blog_os
```

You can of course choose any name you like for the crate.
The command creates a new `src/main.rs` at the root with a main function printing "Hello, world!".
It also creates a new `Cargo.toml` file at the root.
The directory structure now looks like this:

```bash,hl_lines=3 8-9
.
├── Cargo.lock
├── Cargo.toml
├── kernel
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── src
│   └── main.rs
└── target
```

The final step is to add the workspace configuration to the `Cargo.toml` at the root:

```toml ,hl_lines=8-9
# in top-level Cargo.toml

[package]
name = "blog_os"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["kernel"]

[dependencies]
```

That's it!
Now our `blog_os` and `kernel` crates live in the same workspace.
To ensure that everything works as intended, we can run `cargo tree` to list all the packages in the workspace:

```bash
❯ cargo tree --workspace
blog_os v0.1.0 (/.../os)

kernel v0.1.0 (/.../os/kernel)
└── bootloader_api v0.11.3
```

We see that both the `blog_os` and the `kernel` crates are listed, which means that `cargo` recognizes that they're both part of the same workspace.

<div class="note">

If you're getting a _"profiles for the non root package will be ignored"_ warning here, you probably still have a manual `panic = "abort"` override specified in your `kernel/Cargo.toml`.
This override is no longer needed since we compile our kernel for the `x86_64-unknown-none` target, which uses `panic = "abort"` by default.
So to fix this warning, just remove the `profile.dev` and `profile.release` tables from your `kernel/Cargo.toml` file.

</div>

We now have a simple cargo workspace and a new `blog_os` crate at the root.
But what do we need that new crate for?

#### Adding an Artifact Dependency

The reason that we added the new `blog_os` crate is that we want to do something with our _compiled_ kernel.
`Cargo` provides an useful feature for this, called [_artifact dependencies_].
The basic idea is that crates can depend on compiled artifacts (e.g. executables) of other crates.
This is especially useful for artifacts that need to be compiled for a specific target, such as our OS kernel.

[_artifact dependencies_]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#artifact-dependencies

Unfortunately, artifact dependencies are still an unstable feature and not available on stable Rust/Cargo releases yet.
This means that we need to use a [nightly Rust] release for now.

##### Nightly Rust

As the name implies, nightly releases are created every night from the latest `master` commit of the [`rust-lang/rust`] project.
While there is some risk of breakage on the nightly channel, it only occurs very seldomly thanks to extensive checks on the [Rust CI].
Most of the time, breakage only affects unstable features, which require an explicit opt-in.
So by limiting the number of used unstable features as much as possible, we can get a quite stable experience on the nightly channel.
In case something _does_ go wrong, [`rustup`] makes it easy to switch back to an earlier nightly until the issue is resolved.

[nightly Rust]: https://doc.rust-lang.org/book/appendix-07-nightly-rust.html
[`rust-lang/rust`]: https://github.com/rust-lang/rust
[Rust CI]: https://forge.rust-lang.org/infra/docs/rustc-ci.html

<div class = "note"><details>
<summary><em>What is <code>rustup</code></em>?</summary>

The [`rustup`] tool is the [officially recommended] way of installing Rust.
It supports having multiple versions of Rust installed simultaneously and makes upgrading Rust easy.
It also provides access to optional tools and components such as [`rustfmt`] or [`rust-analyzer`].
This guide requires `rustup`, so please install it if you haven't already.

[`rustup`]: https://rustup.rs/
[officially recommended]: https://www.rust-lang.org/learn/get-started
[`rustfmt`]: https://github.com/rust-lang/rustfmt/
[`rust-analyzer`]: https://github.com/rust-lang/rust-analyzer

</details></div>

##### Using Nightly Rust

To use nightly Rust for our project, we create a new [`rust-toolchain.toml`] file in the root directory of our project:

[`rust-toolchain.toml`]: https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file

```toml ,hl_lines=1-3
[toolchain]
channel = "nightly"
profile = "default"
targets = ["x86_64-unknown-none"]
```

The `channel` field specifies which [`toolchain`] to use.
In our case, we want to use the latest nightly compiler.
We could also specify a specific nightly here, e.g. `nightly-2023-04-30`, which can be useful when there is some breakage in the newest nightly.
In the `targets` list, we can specify additional targets that we want to compile to.
In our case, we specify the `x86_64-unknown-none` target that we use for our kernel.

[`toolchain`]: https://rust-lang.github.io/rustup/concepts/toolchains.html

Rustup automatically reads the `rust-toolchain.toml` file and sets up the requested Rust version when running a `cargo` or `rustc` command in this folder, or a subfolder.
We can try this by running `cargo --version`:

```bash
❯ cargo --version
info: syncing channel updates for 'nightly-x86_64-unknown-linux-gnu'
info: latest update on 2023-04-30, rust version 1.71.0-nightly (87b1f891e 2023-04-29)
info: downloading component 'cargo'
info: downloading component 'clippy'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rust-std' for 'x86_64-unknown-none'
info: downloading component 'rustc'
info: downloading component 'rustfmt'
info: installing component 'cargo'
info: installing component 'clippy'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rust-std' for 'x86_64-unknown-none'
info: installing component 'rustc'
info: installing component 'rustfmt'
cargo 1.71.0-nightly (9e586fbd8 2023-04-25)
```

We see that `rustup` automatically downloads and install the nightly version of all Rust components.
This is of course only done once, if the requested toolchain is not installed yet.
To list all installed toolchains, use `rustup toolchain list`.
Updating toolchains is possible through `rustup update`.

##### Enabling Artifact Dependencies

Now that we've installed a nightly version of Rust, we can opt-in to the unstable [_artifact dependency_] feature.
To do this, we create a new folder named `.cargo` in the root of our project.
Inside that folder, we create a new [`cargo` configuration file] named `config.toml`:

[`cargo` configuration file]: https://doc.rust-lang.org/cargo/reference/config.html

```toml ,hl_lines=3-4
# .cargo/config.toml

[unstable]
bindeps = true
```

##### Creating an Artifact Dependency

After switching to nightly Rust and enabling the unstable `bindeps` feature, we can finally add an artifact dependency on our compiled kernel.
For this, we update the `dependency` table of our `blog_os` crate:

```toml ,hl_lines=11-12
[package]
name = "blog_os"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["kernel"]

[dependencies]

[build-dependencies]
kernel = { path = "kernel", artifact = "bin", target = "x86_64-unknown-none" }
```

We will use the artifact in a cargo [_build script_], so we add it to the `build-dependencies` section instead of the normal `dependencies` section.
We specify that the `kernel` crate lives in the `kernel` subdirectory through the `path` key.
The `artifact = "bin"` key specifies that we're interested in the compiled kernel binary (this makes the dependency an artifact dependency).
Finally, we use the `target` key to specify that our kernel binary should be compiled for the `x86_64-unknown-none` target.

[_build script_]: https://doc.rust-lang.org/cargo/reference/build-scripts.html

Now `cargo` will automatically build our kernel before building our `blog_os` crate.
We can see this when building the `blog_os` crate using `cargo build`:

```
❯ cargo build
   Compiling bootloader_api v0.11.3
   Compiling kernel v0.1.0 (/.../os/kernel)
   Compiling blog_os v0.1.0 (/.../os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.51s
```

The `blog_os` crate should be built for our host system, so we don't specify a `--target` argument.
Cargo uses the same profile for compiling the `blog_os` and `kernel` crates, so `cargo build --release` will also build the `kernel` binary with optimizations enabled.

Now that we have set up an artifact dependency on our kernel, we can finally create the bootable disk image.

#### Using the `DiskImageBuilder`

The last step is to invoke the [`DiskImageBuilder`] of the `bootloader` crate, with our kernel executable as input.
We will do this through a cargo [_build script_], which enables us to implement custom build steps that are run on `cargo build`.

To set up a build script, we place a new file named `build.rs` in the root folder of our project (not in the `src` folder!).
Inside it, we create a simple main function:

```rust ,hl_lines=3-5
// build.rs

fn main() {
    panic!("not implemented yet")
}
```

When we run `cargo build` now, we see that the panic is hit:

```bash
❯ cargo build
   Compiling blog_os v0.1.0 (/.../os)
error: failed to run custom build command for `blog_os v0.1.0 (/.../os)`

Caused by:
  process didn't exit successfully: `/.../os/target/debug/build/blog_os-ff0a4f2814615867/build-script-build` (exit status: 101)
  --- stderr
  thread 'main' panicked at 'not implemented yet', build.rs:5:5
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

This panic shows us that cargo found the build script and automatically invoked it as part of `cargo build`.

Now we're ready to use the [`DiskImageBuilder`].
For that, we first add a build dependency on the `bootloader` crate to our `blog_os` crate:

```toml ,hl_lines=5
# in root Cargo.toml

[build-dependencies]
kernel = { path = "kernel", artifact = "bin", target = "x86_64-unknown-none" }
bootloader = "0.11.3"
```

The crate requires the `rust-src` and `llvm-tools` components of `rustup`, which are not installed by default.
To install them, we update our `rust-toolchain.toml` file:

```toml ,hl_lines=6
# rust-toolchain.toml

[toolchain]
channel = "nightly"
profile = "default"
targets = ["x86_64-unknown-none"]
components = ["rust-src", "llvm-tools-preview"]
```

If we run `cargo build` now, the bootloader should be built as a dependency.
The initial build will take a long time, but it should finish without errors.
Please open an issue in the [`rust-osdev/bootloader`] repository if you encounter any issues.

[`rust-osdev/bootloader`]: https://github.com/rust-osdev/bootloader

After adding the dependency, we can use the [`DiskImageBuilder`] in the `main` function of our build script:

```rust, hl_lines=3-4 7-9
// build.rs

use bootloader::DiskImageBuilder;
use std::{env, path::PathBuf};

fn main() {
    // set by cargo for the kernel artifact dependency
    let kernel_path = env::var("CARGO_BIN_FILE_KERNEL").unwrap();
    let disk_builder = DiskImageBuilder::new(PathBuf::from(kernel_path));
}
```

Cargo communicates the path of artifact dependencies through environment variables.
For our `kernel` dependency, the environment variable name is `CARGO_BIN_FILE_KERNEL`.
To read it, we use the [`std::env::var`] function.
If it's not present, we panic using [`unwrap`].
Then wrap convert it to a [`PathBuf`] and pass it to [`DiskImageBuilder::new`].

[`std::env::var`]: https://doc.rust-lang.org/std/env/fn.var.html
[`unwrap`]: https://doc.rust-lang.org/std/result/enum.Result.html#method.unwrap
[`PathBuf`]: https://doc.rust-lang.org/std/path/struct.PathBuf.html
[`DiskImageBuilder::new`]: https://docs.rs/bootloader/0.11.3/bootloader/struct.DiskImageBuilder.html#method.new

Next, we call the `create_uefi_image` and `create_bios_image` methods to create the UEFI and BIOS disk images:

```rust ,hl_lines=11-14 16-18
// build.rs

use bootloader::DiskImageBuilder;
use std::{env, path::PathBuf};

fn main() {
    // set by cargo for the kernel artifact dependency
    let kernel_path = env::var("CARGO_BIN_FILE_KERNEL").unwrap();
    let disk_builder = DiskImageBuilder::new(PathBuf::from(kernel_path));

    // specify output paths
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let uefi_path = out_dir.join("blog_os-uefi.img");
    let bios_path = out_dir.join("blog_os-bios.img");

    // create the disk images
    disk_builder.create_uefi_image(&uefi_path).unwrap();
    disk_builder.create_bios_image(&bios_path).unwrap();
}
```

To prevent collisions, cargo [requires build scripts] to place all their outputs in a specific directory.
Cargo specifies this directory through the `OUT_DIR` environment variable, which we read using [`std::env::var`] again.
After converting the directory path to a [`PathBuf`], we can use the [`join`] method to append file names to it (choose any names you like).
We then use the the `create_uefi_image` and `create_bios_image` methods to create bootable UEFI and BIOS disk images at these paths.

[requires build scripts]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script
[`join`]: https://doc.rust-lang.org/std/path/struct.PathBuf.html#method.join

We can now use use a simple `cargo build` to cross-compile our kernel, build the bootloader, and combine them to create a bootable disk image:

```
❯ cargo build
   Compiling bootloader_api v0.11.3
   Compiling blog_os v0.1.0 (/.../os)
   Compiling kernel v0.1.0 (/.../os/kernel)
    Finished dev [unoptimized + debuginfo] target(s) in 0.43s
```

Cargo will automatically detect when our kernel code is modified and recompile the dependent `blog_os` crate. Builds with optimizations work too, by running `cargo build --release`.

#### Where is it?

We just have one remaining issue:
We don't know in which directory we created the disk images.

So let's update our build script to make the values `uefi_path` and `bios_path` variables accessible.
The best way to do that is to instruct `cargo` to set an environment variable.
We can do this by printing a special [`cargo:rustc-env` string] in our build script:

```rust ,hl_lines=20-22
// build.rs

use bootloader::DiskImageBuilder;
use std::{env, path::PathBuf};

fn main() {
    // set by cargo for the kernel artifact dependency
    let kernel_path = env::var("CARGO_BIN_FILE_KERNEL").unwrap();
    let disk_builder = DiskImageBuilder::new(PathBuf::from(kernel_path));

    // specify output paths
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let uefi_path = out_dir.join("blog_os-uefi.img");
    let bios_path = out_dir.join("blog_os-bios.img");

    // create the disk images
    disk_builder.create_uefi_image(&uefi_path).unwrap();
    disk_builder.create_bios_image(&bios_path).unwrap();

    // pass the disk image paths via environment variables
    println!("cargo:rustc-env=UEFI_IMAGE={}", uefi_path.display());
    println!("cargo:rustc-env=BIOS_IMAGE={}", bios_path.display());
}
```

[`cargo:rustc-env` string]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-env

This sets two environment variables, `UEFI_IMAGE` and `BIOS_IMAGE`.
These variables are now available at build time in the `src/main.rs` of our `blog_os` crate.
This file still contains the default _"Hello, world!"_ output.
Let's change it to print the disk image paths:

```rust ,hl_lines=4-5
// src/main.rs

fn main() {
    println!("UEFI disk image at {}", env!("UEFI_IMAGE"));
    println!("BIOS disk image at {}", env!("BIOS_IMAGE"));
}
```

Since the environment variables are set at build time, we can use the special [`env!` macro] to fill them in.

[`env!` macro]: https://doc.rust-lang.org/std/macro.env.html

Now we can invoke `cargo run` to find out where our disk images are:

```
❯ cargo run
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running `target/debug/blog_os`
UEFI disk image at /.../os/target/debug/build/blog_os-a2f3397119bcf798/out/blog_os-uefi.img
BIOS disk image at /.../os/target/debug/build/blog_os-a2f3397119bcf798/out/blog_os-bios.img
```

We see that they live in some subdirectory in `target/debug/build`.
Note that cargo includes some internals hashes in this path and that it might change this path at any time.

Using this long path is a bit cumbersome, so let's copy the files to the `target/debug` or `target/release` directories directly:

```rust ,hl_lines=3 6-14
// src/main.rs

use std::{env, fs};

fn main() {
    let current_exe = env::current_exe().unwrap();
    let uefi_target = current_exe.with_file_name("uefi.img");
    let bios_target = current_exe.with_file_name("bios.img");

    fs::copy(env!("UEFI_IMAGE"), &uefi_target).unwrap();
    fs::copy(env!("BIOS_IMAGE"), &bios_target).unwrap();

    println!("UEFI disk image at {}", uefi_target.display());
    println!("BIOS disk image at {}", bios_target.display());
}
```

We exploit that the `main` function becomes an executable in `target` or `target/release` after compilation, so we can use the [`current_exe`] path to find the right directory.
Then we use the [`with_file_name`] method to create new file paths in the same directory.
As before, choose any name you like here.

[`current_exe`]: https://doc.rust-lang.org/std/env/fn.current_exe.html
[`with_file_name`]: https://doc.rust-lang.org/std/path/struct.PathBuf.html#method.with_file_name

To copy the disk images to their new path, we use the [`fs::copy`] function.
The last step is to print the new paths.
Now we have the disk images available at a shorter and stable path, which is easier to use:

[`fs::copy`]: https://doc.rust-lang.org/std/fs/fn.copy.html

```bash
❯ cargo run
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running `target/debug/blog_os`
UEFI disk image at /.../os/target/debug/uefi.img
BIOS disk image at /.../os/target/debug/bios.img
❯ cargo run --release
    Finished release [optimized] target(s) in 0.02s
     Running `target/release/blog_os`
UEFI disk image at /.../os/target/release/uefi.img
BIOS disk image at /.../os/target/release/bios.img
```

We see that the disk images are copied to `target/debug` for development builds and to `target/release` for optimized builds, just as we intended.

#### Making `rust-analyzer` happy

In case you're using [`rust-analyzer`], you might notice that it reports some errors in the `kernel/src/main.rs`.
The error might be one of these:

- _found duplicate lang item `panic_impl`_
- _language item required, but not found: `eh_personality`_

The reason for these errors is that `rust-analyzer` tries to build tests and benchmarks for all crates in the workspace.
This fails for our `kernel` crate because testing/benchmarking automatically includes the `std` crate, which conflicts with our `#[no_std]` implementation.

So to fix these errors, we need to specify that our `kernel` crate should not be tested or benchmarked.
We can do this by adding the following to our `kernel/Cargo.toml` file:

```toml ,hl_lines=8-11
# in kernel/Cargo.toml

[package]
name = "kernel"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "kernel"
test = false
bench = false

[dependencies]
bootloader_api = "0.11.0"
```

Now `rust-analyzer` should not report any errors anymore.

## Running our Kernel

After creating a bootable disk image for our kernel, we are finally able to run it.
Before we learn how to run it on real hardware, we start by running it inside the [QEMU] system emulator.
This has multiple advantages:

- We can't break anything: Our kernel has full hardware access, so that a bug might have serious consequences on real hardware.
- We don't need a separate computer: QEMU runs as a normal program on our development computer.
- The edit-test cycle is much faster: We don't need to copy the disk image to bootable usb stick on every kernel change.
- It's possible to debug our kernel via QEMU's debug tools and GDB.

We will still learn how to boot our kernel on real hardware later in this post, but for now we focus on QEMU.
For that you need to install QEMU on your machine as described on the [QEMU download page].

[QEMU download page]: https://www.qemu.org/download/

### Running in QEMU

After installing QEMU, you can run `qemu-system-x86_64 --version` in a terminal to verify that it is installed.
Then you can run the BIOS disk image of our kernel through the following command:

```
qemu-system-x86_64 -drive format=raw,file=target/debug/bios.img
```

As a result, you should see a window open that looks like this:

![QEMU printing several `INFO:` log messages](qemu-bios.png)

This output comes from the bootloader.
As we see, the last line is _"Jumping to kernel entry point at […]"_.
This is the point where the `_start` function of our kernel is called.
Since we currently only `loop {}` in that function nothing else happens, so it is expected that we don't see any additional output.

Running the UEFI disk image works in a similar way, but we need to pass some additional files to QEMU to emulate an UEFI firmware.
This is necessary because QEMU does not support emulating an UEFI firmware natively.
The files that we need are provided by the [Open Virtual Machine Firmware (OVMF)][OVMF] project, which is a sub-project of [TianoCore] and implements UEFI support for virtual machines.
Unfortunately, the project is only [sparsely documented][ovmf-whitepaper] and does not even have a clear homepage.

[OVMF]: https://github.com/tianocore/tianocore.github.io/wiki/OVMF
[TianoCore]: https://www.tianocore.org/
[ovmf-whitepaper]: https://www.linux-kvm.org/downloads/lersek/ovmf-whitepaper-c770f8c.txt

The easiest way to work with OVMF is to download pre-built images of the code.
We provide such images in the [`rust-osdev/ovmf-prebuilt`] repository, ~~which is updated daily from [Gerd Hoffman's RPM builds](https://www.kraxel.org/repos/)~~.
The compiled OVMF are provided as [GitHub releases][ovmf-prebuilt-releases].

[`rust-osdev/ovmf-prebuilt`]: https://github.com/rust-osdev/ovmf-prebuilt/
[ovmf-prebuilt-releases]: https://github.com/rust-osdev/ovmf-prebuilt/releases/latest

To run our UEFI disk image in QEMU, we need the `OVMF-pure-efi.fd` file (other files might work as well).
After downloading it, we can then run our UEFI disk image using the following command:

```
qemu-system-x86_64 -drive format=raw,file=target/debug/uefi.img  -bios OVMF-pure-efi.fd
```

If everything works, this command opens a window with the following content:


![QEMU printing several `INFO:` log messages](qemu-uefi.png)

The output is a bit different than with the BIOS disk image.
Among other things, it explicitly mentions that this is an UEFI boot right on top.

### QEMU Run Scripts

Remembering the QEMU run commands and invoking them manually is a bit cumbersome, so let's invoke the commands from our Rust code.
We implement this by creating a new `src/bin/qemu-bios.rs` file with the following contents:

```rust ,hl_lines=3-14
// src/bin/qemu-bios.rs

use std::{
    env,
    process::{self, Command},
};

fn main() {
    let mut qemu = Command::new("qemu-system-x86_64");
    qemu.arg("-drive");
    qemu.arg(format!("format=raw,file={}", env!("BIOS_IMAGE")));
    let exit_status = qemu.status().unwrap();
    process::exit(exit_status.code().unwrap_or(-1));
}
```

Like our `src/main.rs` file, the `qemu_bios.rs` is an executable that can use the outputs of our build script.
Instead of copying the disk images and printing their paths, we pass the original bios disk image path as input to a QEMU child process.
We create this child process using [`Command::new`], add the arguments via [`Command::arg`], and finally start it using [`Command::status`].
Once the command exits, we finish with the same exit code using [`std::process::exit`].

[`Command::new`]: https://doc.rust-lang.org/std/process/struct.Command.html#method.new
[`Command::arg`]: https://doc.rust-lang.org/std/process/struct.Command.html#method.arg
[`Command::status`]: https://doc.rust-lang.org/std/process/struct.Command.html#method.status
[`std::process::exit`]: https://doc.rust-lang.org/std/process/fn.exit.html

Now we can use `cargo run --bin qemu-bios` to build the kernel, convert it to a bootable disk image, and launch the BIOS disk image in QEMU.
Of course, cargo will only recompile the kernel and rerun the build script if necessary.

Our `src/main.rs` is still usable through `cargo run --bin blog_os`.
However, invoking `cargo run` without a `--bin` arguments will now error because cargo does not know which binary it should start in this case.
We can specify this by adding a new `default-run` key to our top-level `Cargo.toml`:

```toml ,hl_lines=7
# in Cargo.toml

[package]
name = "blog_os"
version = "0.1.0"
edition = "2021"
default-run = "blog_os"

# <...>
```

Now `cargo run` works again.
If you prefer, you can of course also set `default-run` to `qemu-bios` instead.

Let's make things complete by adding a `qemu-uefi` executable as well.
We need the `OVMF-pure-efi.fd`, which we could add as normal file path.
However, the [`ovmf-prebuilt`] crate provides an easier way:
It includes a prebuilt copy of the `OVMF` file and provides it through its `ovmf_pure_efi` function.
To use it, we add it as a dependency to our top-level `Cargo.toml`:

[`ovmf-prebuilt`]: https://docs.rs/ovmf-prebuilt/0.1.0-alpha.1/ovmf_prebuilt/

```toml ,hl_lines=6
# in Cargo.toml

# ...

[dependencies]
ovmf-prebuilt = "0.1.0-alpha"

[build-dependencies]
kernel = { path = "kernel", artifact = "bin", target = "x86_64-unknown-none" }
bootloader = "0.11.3"
```

Now we can create our `qemu-uefi` executable at `src/bin/qemu-uefi.rs`:

```rust ,hl_lines=3-15
// src/bin/qemu-uefi.rs

use std::{
    env,
    process::{self, Command},
};

fn main() {
    let mut qemu = Command::new("qemu-system-x86_64");
    qemu.arg("-drive");
    qemu.arg(format!("format=raw,file={}", env!("UEFI_IMAGE")));
    qemu.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
    let exit_status = qemu.status().unwrap();
    process::exit(exit_status.code().unwrap_or(-1));
}
```

It's very similar to our `qemu-bios` executable.
The only two differences are that it passes an additional `-bios` argument and that it uses the `UEFI_IMAGE` instead of the `BIOS_IMAGE`.
Using a quick `cargo run --bin qemu-uefi`, we can confirm that it works as intended.


### Screen Output

While we see some screen output from the bootloader, our kernel still does nothing.
Let's fix this by trying to output something to the screen from our kernel too.

Screen output works through a so-called [_framebuffer_].
A framebuffer is a memory region that contains the pixels that should be shown on the screen.
The graphics card automatically reads the contents of this region on every screen refresh and updates the shown pixels accordingly.

[_framebuffer_]: https://en.wikipedia.org/wiki/Framebuffer

Since the size, pixel format, and memory location of the framebuffer can vary between different systems, we need to find out these parameters first.
The easiest way to do this is to read it from the [boot information structure][`BootInfo`] that the bootloader passes as argument to our kernel entry point:

```rust ,hl_lines=3 7-13
// in kernel/src/main.rs

use bootloader_api::BootInfo;

// ...

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_ref() {
        let info = framebuffer.info();
        let buffer = framebuffer.buffer();
    }
    loop {}
}

// ...
```

Even though most systems support a framebuffer, some might not.
The [`BootInfo`] type reflects this by specifying its `framebuffer` field as an [`Option`].
Since screen output won't be essential for our kernel (there are other possible communication channels such as serial ports), we use an [`if let`] statement to run the framebuffer code only if a framebuffer is available.

[`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html
[`if let`]: https://doc.rust-lang.org/reference/expressions/if-expr.html#if-let-expressions

The [`FrameBuffer`] type provides two methods: The `info` method returns a [`FrameBufferInfo`] instance with all kinds of information about the framebuffer format, including the pixel type and the screen resolution.
The `buffer` method returns the actual framebuffer content in form of a mutable byte [slice].

[`FrameBuffer`]: https://docs.rs/bootloader/0.11.0/bootloader/boot_info/struct.FrameBuffer.html
[`FrameBufferInfo`]: https://docs.rs/bootloader/0.11.0/bootloader/boot_info/struct.FrameBufferInfo.html
[slice]: https://doc.rust-lang.org/std/primitive.slice.html

We will look into programming the framebuffer in detail in the next post.
For now, let's just try setting the whole screen to some color.
For this, we just set every pixel in the byte slice to some fixed value:

```rust ,hl_lines=5-7
// in kernel/src/main.rs

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        for byte in framebuffer.buffer_mut() {
            *byte = 0x90;
        }
    }
    loop {}
}
```

While it depends on the pixel color format how these values are interpreted, the result will likely be some shade of gray since we set the same value for every color channel (e.g. in the RGB color format).

To boot the new version in QEMU, we use `cargo run --bin qemu-bios` or `cargo run --bin qemu-uefi`.
We see that our guess that the whole screen would turn gray was right:

![QEMU showing a gray screen](qemu-gray.png)

We finally see some output from our little kernel!

### Booting on Real Hardware

To boot on real hardware, write either the `uefi.img` or the `bios.img` disk image to an USB thumb drive.
The actual steps to do this depend on your operating system (see below).
After writing the thumb drive, you can let your computer boot from it.
You can typically choose the boot device by pressing some specific key during the BIOS setup that happens directly after you turn on the computer.

In the following, we show some ways to write a disk image to a thumb drive.

<div class="warning">

**WARNING**: Be very with the following operations.
If you specify the wrong device as the `of=` parameter, you could end up erasing your system or other important data, so make sure that you choose the right target drive.

</div>

#### Unix-like

On any Unix-like host OS (including both Linux and macOS), you can use the `dd` command to write the disk image directly to a USB drive.
First run either `sudo fdisk -l` (on Linux) or `diskutil list` (on a Mac) to get info about where in `/dev` the file representing your device is located.
After that, open a terminal window and run either of the following commands:

##### Linux
```
# replace /dev/sdX with device filename as revealed by "sudo fdisk -l"
$ sudo dd if=target/release/uefi.img of=/dev/sdX
```

##### macOS
```
# replace /dev/diskX with device filename as revealed by "diskutil list"
$ sudo dd if=target/release/uefi.img of=/dev/diskX
```

#### Windows

On Windows, you can use the [Rufus] tool, which is developed as an open-source project [on GitHub][rufus-github].
After downloading it you can directly run it, there's no installation necessary.
In the interface, you select the USB stick you want to write to.

[Rufus]: https://rufus.ie/
[rufus-github]: https://github.com/pbatard/rufus

## Summary and Next Steps

In this post we learned about the [boot process](#the-boot-process) on x86 machines and about the [BIOS](#bios) and [UEFI](#uefi) firmware standards.
We used the `bootloader` and `bootloader_api` crates to convert our kernel to a [bootable disk image](#bootable-disk-image) and [started in QEMU](#running-in-qemu).
Through advanced cargo features such as [workspaces](#creating-a-workspace), [build scripts](#using-the-diskimagebuilder), and [artifact dependencies](#adding-an-artifact-dependency), we created a nice build system that can bring us directly from source code to a running QEMU instance using a single command.

We also started to look into frame buffers and [screen output](#screen-output).
In the [next post], we will continue with this and learn how to draw shapes and render text.

[next post]: @/edition-3/posts/03-screen-output/index.md
