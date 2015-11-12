---
layout: post
title: 'Remapping the Kernel'
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
