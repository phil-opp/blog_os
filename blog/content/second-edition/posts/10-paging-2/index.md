+++
title = "Paging 2"
order = 10
path = "paging-2"
date = 0000-01-01
template = "second-edition/page.html"
+++

This post TODO

<!-- more -->

This blog is openly developed on [Github]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom].

[Github]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments

## Introduction

In the [previous post] we learned about the principles of paging and how the 4-level page tables on the x86_64 architecture work. One thing that the post did not mention: **Our kernel already runs on paging**. The bootloader that we added in the ["A minimal Rust Kernel"] post already set up a 4-level paging hierarchy that maps every page of our kernel to a physical frame. The reason why the bootloader does this is that paging is manditory in 64-bit mode on x86_64.

[previous post]: ./second-edition/posts/09-paging/index.md
["A minimal Rust kernel"]: ./second-edition/posts/02-minimal-rust-kernel/index.md#creating-a-bootimage

The bootloader also sets the correct access permissions for each page, which means that only the pages containing code are executable and only data pages are writable. You can try this by accessing some memory outside our kernel:

```rust
let ptr = 0xdeadbeaf as *mut u32;
unsafe { *ptr = 42; }
```

You will see that this results in an page fault exception. (We don't have page fault handler, so you will see that the double fault handler is invoked.)

In case you are wondering how we could access the physical address `0xb8000` in order to print to the [VGA text buffer]: The bootloader identity mapped this frame, which means that it set up a page at the virtual address `0xb8000` that points to the physical frame with the same address.

[VGA text buffer]: ./second-edition/posts/03-vga-text-buffer/index.md

The question is: How do we access the page tables that our kernel runs to create new page mappings?

## Accessing Page Tables

Accessing the page tables from our kernel is not as easy as it may seem. To understand the problem let's take a look at the example 4-level page table hierarchy of the previous post again:

![An example 4-level page hierarchy with each page table shown in physical memory](../paging/x86_64-page-table-translation.svg)

The important thing here is that each page entry stores the _physical_ address of the next table. This avoids the need to run a translation for these addresses too, which would be bad for performance and could easily cause endless translation loops.

The problem for us is that we can't directly access physical addresses from our kernel, since our kernel also runs on top of virtual addresses. For example when we access address 4KiB, we access the _virtual_ address 4KiB, not the _physical_ address 4KiB where the level 4 page table lives. When we want to acccess the physical address 4KiB, we can only do so through some virtual address that maps to it.

So in order access page table frames, we need to map some virtual pages to them. There are different ways to create these mappings that all allow us to access arbitrary page table frames:


- A simple solution is to **identity map all page tables** like the VGA text buffer:

  ![A virtual and a physical address space with various virtual pages mapped to the physical frame with the same address](identity-mapped-page-tables.svg)
  
  In this example we see various identity-mapped page table frames. This way the physical addresses in the page tables are also valid virtual addresses so that we can easily access the page tables of all levels starting from the CR3 register.
  
  However, it clutters the virtual address space and makes it more difficult to find continuous memory regions of larger sizes. For example, imagine that we want to create a virtual memory region of size 1000 KiB in the above graphic, e.g. for [memory-mapping a file]. We can't start the region at 26 KiB because it would collide with the already mapped page at 1004 MiB. So we have to look further until we find a large enough unmapped area, for example at 1008 KiB. This is a similar fragmentation problem as with [segmentation].

  [memory-mapping a file]: https://en.wikipedia.org/wiki/Memory-mapped_file
  [segmentation]: ./second-edition/posts/09-paging/index.md#fragmentation
  
  Equally, it makes it much more difficult to create new page tables, because we need to find physical frames whose corresponding pages aren't already in use. For example, let's assume that we reserved the 1000 KiB memory region starting at 1008 KiB for our memory-mapped file. Now we can't use any frame with a _physical_ address between 1000 KiB and 2008 KiB anymore, because we can't identity map it.

- Alternatively, we could **map the page tables frames only temporarily** when we need to access them. To be able to create the temporary mappings, we could identity map some level 1 table:

  ![A virtual and a physical address space with an identity mapped level 1 table, which maps its 0th entry to the level 2 table frame, therey mapping that frame to page with address 0](temporarily-mapped-page-tables.svg)

  The level 1 table in this graphic controls the first 2 MiB of the virtual address space. This is because it is reachable by starting at the CR3 register and following the 0th entry in the level 4, level 3, and level 2 page tables. The entry with index 8 maps the virtual page at address 32 KiB to the physical frame at address 32 KiB, thereby identity mapping the level 1 table it is contained in. The graphic shows this identity-mapping by the horizontal arrow at 32 KiB.

  By writing the identity-mapped level 1 table our kernel can up to 511 temporary mappings. In the above example, the kernel temporarily mapped the physical frame at 32 KiB to the virtual page at 0 KiB, indicated by the dashed arrow. Now the kernel can access the level 2 page table by writing to the page starting at 0 KiB.
  
  The process for accessing an arbitrary page table frame would be:

  - Search for a free entry in the identity mapped level 1 table.
    - Store the physical address of the target frame in that entry.
  - Access the target frame through the virtual page that maps to the entry.
  - Set the entry back to unused.

  This approach keeps the virtual address space clean, since it reuses the same 512 virtual pages for creating the mappings. The drawback is that it is a bit cumbersome, especially since we would need to temporarily map up to three frames in order to create a single new mapping in the 4-level page table.

- While both of the above approaches work, there is a third technique called **recursive page tables** that combines their advantages: It keeps all page table frames mapped like with the identity-mapping, so that no temporary mappings are needed, and also keeps the mapped pages together to avoid fragmentation of the virtual address space. Recursive page tables are described in detail in the following section, because this is the technique that we will use for our implementation.

### Recursive Page Tables

The idea behind this approach sounds simple: _Map some entry of the level 4 page table to the frame of the very same table_, similar to how the level 1 table in the previous example mapped itself. By doing this in the level 4 table, we effectively reserve a part of the virtual address space and map all current and future page table frames to that space. Thus, the single entry makes every table of every level accessible through a calculatable address.

Let's go through an example to understand how this all works:

![An example 4-level page hierarchy with each page table shown in physical memory. Entry 511 of the level 4 page is mapped to frame 4KiB, the frame of the level 4 table itself.](recursive-page-table.svg)

The only difference to the [example at the beginning of this post] is the additional entry at index 511 in the level 4 table, which is mapped to physical frame 4 KiB, the frame of the level 4 table itself.

[example at the beginning of this post]: #accessing-page-tables

By letting the CPU follow this entry on a translation, it doesn't reach a level 3 table, but the same level 4 table again. This is similar to a recursive function that calls itself, therefore this table is called a _recursive page table_. The important thing is that the CPU assumes that every entry in the level 4 table points to a level 3 table, so it now treats the level 4 table as a level 3 table. This works because tables of all levels have the exact same layout on x86_64.

By following the recursive entry one or multiple times before we start the actual translation, we can effectively shorten the number of levels that the CPU traverses. For example, if we follow the recursive entry once and then proceed to the level 3 table, the CPU thinks that the level 3 table is a level 2 table. Going further, it treats the level 2 table as a level 1 table, and the level 1 table as the mapped frame. This means that we can now read and write the level 1 page table because the CPU thinks that it is the mapped frame.

TODO graphic

Similarly, we can follow the recursive entry twice before starting the translation to reduce the number of traversed levels to two. Let's go through it step by step: First the CPU follows the recursive entry on the level 4 table and thinks that it reaches a level 3 table. Then it follows the recursive entry again and thinks that it reaches a level 2 table. But in reality, it is still on the level 4 table. When the CPU now follows another entry, it lands on a level 3 table, but thinks it is already on a level 1 table. So while the next entry points at a level 2 table, the CPU thinks that it points to the mapped frame, which allows us to read and write the level 2 table.

TODO graphic

Accessing the level 3 tables works in the same way. For accessing the level 3 table, we follow the recursive entry entry three times, tricking the CPU into thinking it is already on a level 1 table. Then we follow another entry and reach a level 3 table, which the CPU treats as a mapped frame. For accessing the level 4 table itself, we just follow the recursive entry four times until the CPU treats the level 4 table as mapped frame.

TODO graphic

#### Address Calculation

We saw that we can access tables of all levels by following the recursive entry once or multiple times before the actual translation. But how do we do this?

Remember, the indexes into the various table levels are derived directly from the virtual address:

TODO graphic

To follow the recursive entry once before doing the translation we move each of the address one entry to the right:

TODO graphic



## A Physical Memory Map

## Allocating Stacks

## Summary

## What's next?

---
TODO update post date