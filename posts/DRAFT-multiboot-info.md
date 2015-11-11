---
layout: post
title: 'The Multiboot Information Structure'
---

When a Multiboot compliant bootloader loads a kernel, it passes a pointer to a boot information structure in the `ebx` register. We can use it to get information about available memory and loaded kernel sections.

TODO

## The Structure
The Multiboot information structure looks like this:

Field            | Type
---------------- | -----------
total size       | u32
reserved         | u32
tags             | variable
end tag = (0, 8) | (u32, u32)

There are many different types of tags, but they all have the same beginning:

Field         | Type
------------- | -----------------
type          | u32
size          | u32
other fields  | variable

All tags are 8-byte aligned. The last tag must be the _end tag_, which is a tag of type `0` and size `8`.

## A Rust module

TODO

## Tags

We are interested in two tags, the _Elf-symbols_ tag and the _memory map_ tag. For a full list of possible tags see section 3.4 in the Multiboot 2 specification ([PDF][Multiboot 2]).

[Multiboot 2]: http://nongnu.askapache.com/grub/phcoder/multiboot.pdf

### The Elf-Symbols Tag
The Elf-symbols tag contains a list of all sections of the loaded [ELF] kernel. It has the following format:

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format

Field                       | Type
--------------------------- | -----------------
type = 9                    | u32
size                        | u32
number of entries           | u16
entry size                  | u16
string table                | u16
reserved                    | u16
section headers             | variable

The section headers are just copied from the ELF file, so we need to look at the ELF specification to find the corresponding structure definition. Our kernel is a 64-bit ELF file, so we need to look at the ELF-64 specification ([PDF][ELF specification]). According to section 4 and figure 3, a section header has the following format:

[ELF specification]: http://www.uclibc.org/docs/elf-64-gen.pdf

Field                       | Type             | Value
--------------------------- | ---------------- | -----------
name                        | u32              | string table index
type                        | u32              | `0` (unused), `1` (section of program), `3` (string table), `8` (uninitialized section), etc.
flags                       | u64              | `0x1` (writable), `0x2` (loaded), `0x4` (executable), etc.
address                     | u64              | virtual start address of section (0 if not loaded)
file offset                 | u64              | offset (in bytes) of section contents in the file
size                        | u64              | size of the section in bytes
link                        | u32              | associated section (only for some section types)
info                        | u32              | extra information (only for some section types)
address align               | u64              | required alignment of section (power of 2)
entry size                  | u64              | contains the entry size for table sections (e.g. string table)

### The Memory Map Tag

TODO

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
