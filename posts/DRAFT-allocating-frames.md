---
layout: post
title: 'Allocating Frames'
---

TODO

## The Multiboot Information Structure
When a Multiboot compliant bootloader loads a kernel, it passes a pointer to a boot information structure in the `ebx` register. We can use it to get information about available memory and loaded kernel sections.

First, we need to pass this pointer to our kernel as an argument to `rust_main`. To find out how arguments are passed to functions, we can look at the [calling convention of Linux]:

[calling convention of Linux]: https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI

> The first six integer or pointer arguments are passed in registers RDI, RSI, RDX, RCX, R8, and R9

So to pass the pointer to our kernel, we need to move it to `rdi` before calling the kernel. Since we're not using the `rdi`/`edi` register in our bootstrap code right now, we can simply set the `edi` register right after booting (in `boot.asm`):

```nasm
start:
    mov esp, stack_top
    mov edi, ebx       ; Move Multiboot info pointer to edi
```
Now we can add the argument to our `rust_main`:

```rust
pub extern fn rust_main(multiboot_information_address: usize) { ... }
```

Now we can use the [multiboot2-elf64] crate to query get some information about mapped kernel sections and available memory. I just wrote it for this blog post since I could not find any other Multiboot 2 crate. It's really ugly and incomplete, but it does its job.

[multiboot2-elf64]: https://github.com/phil-opp/multiboot2-elf64

So let's add a dependency on the git repository in the `Cargo.toml`:

```toml
...
[dependencies.multiboot2]
git = "https://github.com/phil-opp/multiboot2-elf64"
```

Now we can add `extern crate multiboot2` and use it to print available memory areas.

### Available Memory
The boot information structure consists of various _tags_. The _memory map_ tag contains a list of all areas of available RAM. Special areas such as the VGA text buffer at `0xb8000` are not available. Note that some of the available memory is already used by our kernel and by the multiboot information structure itself.

To print available memory areas, we can use the `multiboot2` crate in our `rust_main` as follows:

```rust
let boot_info = unsafe{ multiboot2::load(multiboot_information_address) };

println!("memory areas:");
for area in boot_info.memory_map_tag().unwrap().memory_areas() {
    println!("    start: 0x{:x}, length: 0x{:x}", area.base_addr, area.length);
}
```

The `load` function is `unsafe` because it relies on a valid address. Since the memory tag is not required, the `memory_map_tag()` function returns an `Option`. The `memory_areas()` function returns the desired memory area iterator.

The output looks like this:

```
Hello World!
memory areas:
    start: 0x0, length: 0x9fc00
    start: 0x100000, length: 0x7ee0000
```
So we have one area from `0x0` to `0x9fc00`, which is a bit below the 1MiB mark. The second, bigger area starts at 1MiB and contains the rest of available memory. The area from `0x9fc00` to 1MiB is not available. For example the VGA text buffer at `0xb8000` is in that area. This is the reason for putting our kernel at 1MiB and not at e.g. `0x0`.

If you give QEMU more than 4GiB of memory by passing `-m 5G`, you get another unusable area below the 4GiB mark. This memory is normally mapped to some hardware devices. See the [OSDev Wiki][Memory_map] for more information.

[Memory_map]: http://wiki.osdev.org/Memory_Map_(x86)

### Kernel ELF Sections
To read and print the sections of our kernel ELF file, we can use the _Elf-sections_ tag:

```rust
println!("kernel sections:");
for section in boot_info.elf_sections_tag().unwrap().sections() {
    println!("    addr: 0x{:x}, size: 0x{:x}, flags: 0x{:x}",
        section.addr, section.size, section.flags);
}
```
This should print out the start address and size of all kernel sections. If the section is writable, the `0x1` is set in `flags`. The `0x4` bit marks an executable section and the `0x2` indicates that the section was loaded in memory. For example, the `.text` section is executable but not writable and the `.data` section just the opposite.

But when we execute it, tons of really small sections are printed. We can use the `objdump -h build/kernel-x86_64.bin` command to list the sections with name. There seem to be over 200 sections and many of them start with `.text.*` or `.data.rel.ro.local.*`. The Rust compiler puts each function in an own `.text` subsection. To merge these subsections, we can update our linker script:

```
SECTIONS {
    . = 1M;

    .boot :
    {
        KEEP(*(.multiboot_header))
    }

    .text :
    {
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data.rel.ro : {
        *(.data.rel.ro.local*) *(.data.rel.ro .data.rel.ro.*)
    }
}
```

These lines are taken from the default linker script of `ld`, which can be obtained through `ld ‑verbose`. Now there are only 12 sections left and we get a much more useful output:

![qemu output](/images/qemu-memory-areas-and-kernel-sections.png)

## Start and End of Kernel
We can now use the ELF section tag to calculate the start and end address of our loaded kernel:

TODO

## A frame allocator
When we create a paging module in the next post, we will need to map virtual pages to free physical frames. So we will need some kind of allocator that keeps track of physical frames and gives us a free one when needed. We can use the memory tag to write such a frame allocator.

The allocator struct looks like this:

```rust
struct AreaFrameAllocator {
    first_used_frame: Frame,
    last_used_frame: Frame,
    current_area: Option<MemoryArea>,
    areas: MemoryAreaIter,
}
```
TODO

To allocate a frame we try to find one in the current area and update the first/last used bounds. If we can't find one, we look for the new area with the minimal start address, that still contains free frames. If the current area is `None`, there are no free frames left.

TODO

### Unit Tests
TODO

## Remapping the Kernel Sections
We can use the ELF section tag to write a skeleton that remaps the kernel correctly:

```rust
for section in multiboot.elf_tag().sections() {
    for page in start_page..end_page {
        // TODO identity_map(page, section.writable(), section.executable())
    }
}
```
TODO
