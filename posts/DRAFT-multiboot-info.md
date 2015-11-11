---
layout: post
title: 'The Multiboot Information Structure'
---

When a multiboot compliant bootloader loads a kernel, it passes a pointer to a boot information struct in the `ebx` register. We can use it to get information about available memory and loaded kernel sections. So let's write a module for it!

## The Structure
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
