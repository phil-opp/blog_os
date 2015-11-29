---
layout: post
title: 'A Paging Module'
---

## Paging

## Modeling Page Tables
Let's begin a `memory/paging/mod.rs` module to model page tables:

```rust
pub const PAGE_SIZE: usize = 4096;
const ENTRY_SIZE: usize = 8;
const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;

pub struct Page {
   number: usize,
}

struct Table(Page);

#[derive(Debug, Clone, Copy)]
struct TableEntry(u64);
```
We define constants for the page size, the size of an entry in a page table, and the number of entries per table. To make future function signatures more expressive, we can use the type aliases `PhysicalAddress` and `VirtualAddress`. The `Page` struct is similar to the `Frame` struct in the [previous post], but represents a virtual page instead of a physical frame.

[previous post]: {{ page.previous.url }}

The `Table` struct represents a P4, P3, P2, or P1 table. It's a newtype wrapper around the `Page` that contains the table. And the `TableEntry` type represents an 8 byte large page table entry.

To get the i-th entry of a `Table`, we add a `entry()` method:

```rust
fn entry(&self, index: usize) -> TableEntry {
    assert!(index < ENTRY_COUNT);
    let entry_address = self.0.start_address() + index * ENTRY_SIZE;
    unsafe { *(entry_address as *const _) }
}
```
The `start_address` function is covered below. We're doing manual pointer arithmetic in this function and need an `unsafe` block to convince Rust that there's a valid `TableEntry` at the given address. For this to be safe, we need to make sure that we only construct valid `Table` structs in the future.

TODO formulierung for this to be safe

### Sign Extension
The `Page::start_address` method doesn't exist yet. But it should be a simple `page.number * PAGE_SIZE`, right? Well, if the x86_64 architecture had true 64bit addresses, yes. But in reality the addresses are just 48bit long and the other bits are just _sign extension_, i.e. a copy of the most significant bit. That means that the address calculated by `page.number * PAGE_SIZE` is wrong if the 47th bit is used. Some examples:

```
invalid address: 0x0000_800000000000
        sign extension | 48bit address
valid sign extension: 0xffff_800000000000
```
TODO graphic

So the address space is split into two halves: the _higher half_ containing addresses with sign extension and the _lower half_ containing addresses without. And our `Page::start_address` method needs to respect this:

```rust
pub fn start_address(&self) -> VirtualAddress {
    if self.number >= 0x800000000 {
        // sign extension necessary
        (self.number << 12) | 0xffff_000000000000
    } else {
        self.number << 12
    }
}
```
The `0x800000000` is the start address of the higher half without the last four 0s (because it's a page _number_).

### Table entries
Now we can get a `TableEntry` through the `entry` function. Now we need to extract the relevant information.

Remember, a page table entry looks like this:

Bit(s)                | Name | Meaning
--------------------- | ------ | ----------------------------------
0 | present | the page is currently in memory
1 | writable | it's allowed to write to this page
2 | user accessible | if not set, only kernel mode code can access this page
3 | write through caching | writes go directly to memory
4 | disable cache | no cache is used for this page
5 | accessed | the CPU sets this bit when this page is used
6 | dirty | the CPU sets this bit when a write to this page occurs
7 | huge page/null | must be 0 in P1 and P4, creates a 1GiB page in P3, creates a 2MiB page in P2
8 | global | page isn't flushed from caches on address space switch (PGE bit of CR4 register must be set)
9-11 | available | can be used freely by the OS
12-51 | physical address | the page aligned 52bit physical address of the frame or the next page table
52-62 | available | can be used freely by the OS
63 | no execute | forbid executing code on this page (the NXE bit in the EFER register must be set)

To extract the physical address we add a `TableEntry::pointed_frame` method:

```rust
fn pointed_frame(&self) -> Frame {
    Frame { number: ((self.0 & 0x000fffff_fffff000) >> 12) as usize }
}
```
First we mask bits 12-51 and then convert the physical address to the corresponding frame number (through `>> 12`). We don't need to respect any sign extension here since it only exists for virtual addresses.

To model the various flags, we will use the [bitflags] crate. Unfortunately the official version depends on the standard library as `no_std` is still unstable. But since it does not actually require any `std` functions, it's pretty easy to create a `no_std` version. You can find it here [here][bitflags fork]. To add it as a dependency, add the following to your `Cargo.toml`:

[bitflags]: /TODO
[bitflags fork]: /TODO

```toml
[dependencies.bitflags]
git = "https://github.com/phil-opp/bitflags.git"
branch = "no_std"
```
Note that you need a `#[macro_use]` above the `extern crate` definition.

Now we can model the various flags:

```rust
bitflags! {
    flags TableEntryFlags: u64 {
        const PRESENT =         1 << 0,
        const WRITABLE =        1 << 1,
        const USER_ACCESSIBLE = 1 << 2,
        const WRITE_THROUGH =   1 << 3,
        const NO_CACHE =        1 << 4,
        const ACCESSED =        1 << 5,
        const DIRTY =           1 << 6,
        const HUGE_PAGE =       1 << 7,
        const GLOBAL =          1 << 8,
        const NO_EXECUTE =      1 << 63,
    }
}
```
To extract the flags we create a `TableEntryFlags::flags` method that uses [from_bits_truncate]:

[from_bits_truncate]: /TODO

```rust
fn flags(&self) -> TableEntryFlags {
    TableEntryFlags::from_bits_truncate(self.0)
}
```

Now we can read page tables and retrieve the mapping information. But since we can't access page tables through their physical address, we need to map them to some virtual address, too.

## Mapping Page Tables
So how do we map the page tables itself? We don't have that problem for the current P4, P3, and P2 table since they are part of the identity-mapped area, but we need a way to access future tables, too.

One solution could be to identity map all page table. That way we would not need to differentiate virtual and physical address and could easily access the tables. But it makes creating page tables more complicated since we need a physical frame whose corresponding page isn't already used for something else. And it clutters the virtual address space and may even cause heap fragmentation.

An alternative solution is to map the page tables only temporary. So to read/write a page table, we would map it to some free virtual address. We could use a small pool of such virtual addresses and reuse them for various tables. This method occupies only few virtual addresses and is thus a good solution for 32-bit systems, which have small address spaces. But it makes things much more complicated since the temporary mapping requires updating other page tables, which need to be mapped, too. So we need to make sure that the temporary addresses are always mapped, else it could cause an endless recursion.

We will use another solution, which uses a trick called _recursive mapping_.

### Recursive Mapping
The trick is to map the `P4` table recursively: The last entry doesn't point to a `P3` table, but to the `P4` table itself. Through this entry, all page tables are mapped to an unique virtual address. So we can access and modify page tables of all levels by just setting one `P4` entry once. It may seem a bit strange at first, but is a very clean and simple solution once you wrapped your head around it.

TODO image

To access for example the `P4` table itself, we use the address that chooses the 511th `P4` entry, the 511th `P3` entry, the 511th `P2` entry and the 511th `P1` entry. Thus we choose the same `P4` frame over and over again and finally end up on it, too. Through the offset (12 bits) we choose the desired entry.

To access a `P3` table, we do the same but choose the real `P4` index instead of the fourth loop. So if we like to access the 42th `P3` table, we use the address that chooses the 511th entry in the `P4`, `P3`, and `P2` table, but the 42th `P1` entry.

When accessing a `P2` table, we only loop two times and then choose entries that correspond to the `P4` and `P3` table of the desired `P2` table. And accessing a `P1` table just loops once and then uses the corresponding `P4`, `P3`, and `P2` entries.

The math checks out, too. If all page tables are used, there is 1 `P4` table, 511 `P3` tables (the last entry is used for the recursive mapping), `511*512` `P2` tables, and `511*512*512` `P1` tables. So there are `134217728` page tables altogether. Each page table occupies 4KiB, so we need `134217728 * 4KiB = 512GiB` to store them. That's exactly the amount of memory that can be accessed through one `P4` entry since `4KiB per page * 512 P1 entries * 512 P2 entries * 512 P3 entries = 512GiB`.

TODO: recursive map in assembly

## Translating addresses
Now we can use the recursive mapping to translate virtual address manually. We will create a function that takes a virtual address and returns the corresponding physical address.

TODO


To get the page tables and corresponding indexes for a page, we add some methods for `Page`:

```rust
fn p4_index(&self) -> usize {(self.number >> 27) & 0o777}
fn p3_index(&self) -> usize {(self.number >> 18) & 0o777}
fn p2_index(&self) -> usize {(self.number >> 9) & 0o777}
fn p1_index(&self) -> usize {(self.number >> 0) & 0o777}

const fn p4_table(&self) -> Table {
    Table(Page { number: 0o_777_777_777_777 } )
}

fn p3_table(&self) -> Table {
    Table(Page {
      number: 0o_777_777_777_000 | self.p4_index(),
    })
}

fn p2_table(&self) -> Table {
    Table(Page {
      number: 0o_777_777_000_000 | (self.p4_index() << 9) |
              self.p3_index(),
    })
}

fn p1_table(&self) -> Table {
    Table(Page {
      number: 0o_777_000_000_000 | (self.p4_index() << 18) |
              (self.p3_index() << 9) | self.p2_index(),
    })
}
```
We use the octal numbers since they make it easy to express the 9 bit table indexes.

The P4 table is the same for all addresses, so we can make the function `const`. The associated page has index 511 in all four pages, thus the four `777` blocks. The P3 table, however, is different for different P4 indexes. So the last block varies from `000` to `776`, dependent on the page's P4 index. The P2 table additionally depends on the P3 index and to get the P1 table we use the recursive mapping only once (thus only one `777` block).

TODO

## Switching Page Tables

## Mapping Pages

## Unmapping Pages
