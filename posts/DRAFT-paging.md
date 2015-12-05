---
layout: post
title: 'A Paging Module'
---

## Paging

## A Basic Paging Module
Let's create a basic `memory/paging/mod.rs` module:

```rust
pub const PAGE_SIZE: usize = 4096;
const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;

pub struct Page {
   number: usize,
}
```
We define constants for the page size and the number of entries per table. To make future function signatures more expressive, we can use the type aliases `PhysicalAddress` and `VirtualAddress`. The `Page` struct is similar to the `Frame` struct in the [previous post], but represents a virtual page instead of a physical frame.

[previous post]: {{ page.previous.url }}

### Page Table Entries
To model page table entries, we create a new `entry` submodule:

```rust
use memory::Frame; // needed later

pub struct Entry(u64);

impl Entry {
    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    pub fn set_unused(&mut self) {
        self.0 = 0;
    }
}
```
We define that an unused entry is completely 0. That allows us to distinguish unused entries from other non-present entries in the future. For example, we could define one of the available bits as the `swapped_out` bit for pages that are swapped to disk.

Next we will model the contained physical address and the various flags. Remember, entries have the following format:

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

To model the various flags, we will use the [bitflags] crate. Unfortunately the official version depends on the standard library as `no_std` is still unstable. But since it does not actually require any `std` functions, it's pretty easy to create a `no_std` version. You can find it here [here][bitflags fork]. To add it as a dependency, add the following to your `Cargo.toml`:

[bitflags]: https://github.com/rust-lang-nursery/bitflags
[bitflags fork]: https://github.com/phil-opp/bitflags/tree/no_std

```toml
[dependencies.bitflags]
git = "https://github.com/phil-opp/bitflags.git"
branch = "no_std"
```
Note that you need a `#[macro_use]` above the `extern crate` definition.

Now we can model the various flags:

```rust
bitflags! {
    flags EntryFlags: u64 {
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
To extract the flags from the entry we create a `TableEntryFlags::flags` method that uses [from_bits_truncate]:

[from_bits_truncate]: https://doc.rust-lang.org/bitflags/bitflags/macro.bitflags!.html#methods

```rust
pub fn flags(&self) -> EntryFlags {
    EntryFlags::from_bits_truncate(self.0)
}
```
This allows us to check for flags through the `contains()` function. For example, `flags().contains(PRESENT | WRITABLE)` returns true if the entry contains _both_ flags.

To extract the physical address, we add a `pointed_frame` method:

```rust
pub fn pointed_frame(&self) -> Option<Frame> {
    if self.flags().contains(PRESENT) {
        Some(Frame::containing_address(self.0 as usize & 0x000fffff_fffff000))
    } else {
        None
    }
}
```
If the entry is present, we mask bits 12–51 and return the corresponding frame. If the entry is not present, it does not point to a valid frame so we return `None`.

To modify entries, we add a `set` method that updates the flags and the pointed frame:

```rust
pub fn set(&mut self, frame: Frame, flags: EntryFlags) {
    assert!(frame.start_address() & !0x000fffff_fffff000 == 0);
    self.0 = (frame.start_address() as u64) | flags.bits();
}
```
The start address of a frame should be page aligned and smaller than 2^52 (since x86 uses 52bit physical addresses). Since an invalid address could mess up the entry, we add an assertion. To actually set the entry, we just need to `or` the start address and the flag bits.

The missing `start_address` function is pretty simple:

```rust
use memory::paging::PhysicalAddress;

impl Frame {
    fn start_address(&self) -> PhysicalAddress {
        self.number << 12
    }
}
```
Since we only need it in the entry submodule, we put it in a new `impl Frame` block in `entry.rs`.

## Page Tables
To model page tables, we create a basic `Table` struct in a new `table` submodule:

```rust
use memory::paging::entry::*;
use memory::paging::ENTRY_COUNT;

pub struct Table {
    entries: [Entry; ENTRY_COUNT],
}
```
It's just an array of 512 page table entries.

To make the `Table` indexable itself, we can implement the `Index` and `IndexMut` traits:

```rust
use core::ops::{Index, IndexMut};

impl Index<usize> for Table {
    type Output = Entry;

    fn index(&self, index: usize) -> &Entry {
        &self.entries[index]
    }
}

impl IndexMut<usize> for Table {
    fn index_mut(&mut self, index: usize) -> &mut Entry {
        &mut self.entries[index]
    }
}
```
Now it's possible to get the 42th entry through `some_table[42]`. Of course we could replace `usize` with `u32` or even `u16` here but it would cause more numerical conversions (`x as u16`).

Let's add a method that sets all entries to unused. We will need it when we create new page tables in the future. The method looks like this:

```rust
pub fn zero(&mut self) {
    for entry in self.entries.iter_mut() {
        entry.set_unused();
    }
}
```

Now we can read page tables and retrieve the mapping information. We can also update them through the `IndexMut` trait and the `Entry::set` method. But how do we get references to the various page tables?

We could read the `CR3` register to get the physical address of the P4 table and read its entries to get the P3 addresses. The P3 entries then point to the P2 tables and so on. But this method only works for identity-mapped pages. But in the future we will create new page tables, which aren't in the identity-mapped area anymore. Since we can't access them through their physical address, we need a way to map them to virtual addresses.

## Mapping Page Tables
So how do we map the page tables itself? We don't have that problem for the current P4, P3, and P2 table since they are part of the identity-mapped area, but we need a way to access future tables, too.

One solution is to identity map all page table. That way we would not need to differentiate virtual and physical address and could easily access the tables. But it clutters the virtual address space and increases fragmentation. And it makes creating page tables much more complicated since we need a physical frame whose corresponding page isn't already used for something else.

An alternative solution is to map the page tables only temporary. So to read/write a page table, we would map it to some free virtual address. We could use a small pool of such virtual addresses and reuse them for various tables. This method occupies only few virtual addresses and is thus a good solution for 32-bit systems, which have small address spaces. But it makes things much more complicated since the temporary mapping requires updating other page tables, which need to be mapped, too.

We will use another solution, which uses a trick called _recursive mapping_.

### Recursive Mapping
The trick is to map the P4 table recursively: The last entry doesn't point to a P3 table, but to the P4 table itself. Through this entry, all page tables are mapped to an unique virtual address.

TODO image

To access for example the P4 table itself, we use the address that chooses the 511th P4 entry, the 511th P3 entry, the 511th P2 entry and the 511th P1 entry. Thus we choose the same P4 frame over and over again and finally end up on it, too. Through the offset (12 bits) we choose the desired entry.

To access a P3 table, we do the same but choose the real P4 index instead of the fourth loop. So if we like to access the 42th P3 table, we use the address that chooses the 511th entry in the P4, P3, and P2 table, but the 42th P1 entry.

When accessing a P2 table, we only loop two times and then choose entries that correspond to the P4 and P3 table of the desired P2 table. And accessing a P1 table just loops once and then uses the corresponding P4, P3, and P2 entries.

So we can access and modify page tables of all levels by just setting one P4 entry once. It may seem a bit strange at first, but is a very clean and simple solution once you wrapped your head around it.

The math checks out, too. If all page tables are used, there is 1 P4 table, 511 P3 tables (the last entry is used for the recursive mapping), `511*512` P2 tables, and `511*512*512` P1 tables. So there are `134217728` page tables altogether. Each page table occupies 4KiB, so we need `134217728 * 4KiB = 512GiB` to store them. That's exactly the amount of memory that can be accessed through one P4 entry since `4KiB per page * 512 P1 entries * 512 P2 entries * 512 P3 entries = 512GiB`.

### Implementation
To map the P4 table recursively, we just need to point the 511th entry to the table itself. Of course we could do it in Rust, but it would require some unsafe pointer fiddling. It's easier to just add some lines to our boot assembly:

```nasm
mov eax, p4_table
or eax, 0b11 ; present + writable
mov [p4_table + 511 * 8], eax
```
I put it right after the `setup_page_tables` label, but you can add it wherever you like.

### The special addresses
Now we can use special virtual addresses to access the page tables. The P4 table is available at `0xfffffffffffff000`. Let's add a P4 constant to the `table` submodule:

```rust
pub const P4: *mut Table = 0xffffffff_fffff000 as *mut _;
```

Let's switch to the octal system, since it makes more sense for the other special addresses. The P4 address from above is equivalent to `0o177777_777_777_777_777_0000` in octal. You can see that is has index `777` in all tables and offset `0000`. The `177777` bits on the left are the sign extension bits, which are copies of the 47th bit. They are required because x86 only uses 48bit virtual addresses.

The other tables can be accessed through the following addresses:

Table | Address                         | Indexes
----- | ------------------------------- | ----------------------------------
P4    | `0o177777_777_777_777_777_0000` | –
P3    | `0o177777_777_777_777_XXX_0000` | `XXX` is the P4 index
P2    | `0o177777_777_777_XXX_YYY_0000` | like above, and `YYY` is the P3 index
P1    | `0o177777_777_XXX_YYY_ZZZ_0000` | like above, and `ZZZ` is the P2 index

If we look closely, we can see that the P3 address is equal to `(P4 << 9) | XXX_0000`. And the P2 address is calculated through `(P3 << 9) | YYY_0000`. So to get the next address, we need to shift it 9 bits to the left and add the table index. As a formula:

```
next_table_address = (table_address << 9) | (index << 12)
```

So let's add it as a `Table` method:

```rust
fn next_table_address(&self, index: usize) -> Option<usize> {
    let entry_flags = self[index].flags();
    if entry_flags.contains(PRESENT) && !entry_flags.contains(HUGE_PAGE) {
        let table_address = self as *const _ as usize;
        Some((table_address << 9) | (index << 12))
    } else {
        None
    }
}
```
The next table address is only valid if the corresponding entry is present and does not create a huge page. Then we can do some pointer casting to get the table address and use the formula to calculate the next address.

If the index is out of bounds, the function will panic since Rust checks array bounds. The panic is desired here since a wrong index should not be possible and indicates a bug.

To convert the address into references, we add two functions:

```rust
pub fn next_table(&self, index: usize) -> Option<&Table> {
    self.next_table_address(index)
        .map(|t| unsafe { &*(t as *const _) })
}

pub fn next_table_mut(&mut self, index: usize) -> Option<&mut Table> {
    self.next_table_address(index)
        .map(|t| unsafe { &mut *(t as *mut _) })
}
```
We convert the address into raw pointers and then convert them into references in `unsafe` blocks. Now we can start at the `P4` constant and use these functions to access the lower tables. And we don't even need `unsafe` blocks to do it!

Right now, your alarm bells should be ringing. Thanks to Rust, everything we've done before in this post was completely safe. But we just introduced two unsafe blocks to convince Rust that there are valid tables at the specified addresses. Can we really be sure?

First, these addresses are only valid if the P4 table is mapped recursively. Since the paging module will be the only module that modifies page tables, we can introduce an invariant for the module:

> _The 511th entry of the active P4 table must always be mapped to the active P4 table itself._

So if we switch to another P4 table at some time, it needs to be identity mapped _before_ it becomes active. As long as we obey this invariant, we can safely use the special addresses. But even with this invariant, there is a big problem with the two methods:

_What happens if we call them on a P1 table?_

Well, they would calculate the address of the next table (which does not exist) and treat it as a page table. Either they construct an invalid address (if `XXX < 400`) or access the mapped page itself. That way, we could easily corrupt memory or cause CPU exceptions by accident. So these two functions are not _safe_ in Rust terms. Thus we need to make them `unsafe` functions unless we find some clever solution.

## Some Clever Solution
We can use Rust's type system to statically guarantee that the methods can only be called on P4, P3, and P2 tables. The idea is to add a `Level` parameter to the `Table` type and implement the `next_table` methods only for level 4, 3, and 2.

To model the levels we use a trait and empty enums:

```rust
pub trait TableLevel {}

pub enum Level4 {}
enum Level3 {}
enum Level2 {}
enum Level1 {}

impl TableLevel for Level4 {}
impl TableLevel for Level3 {}
impl TableLevel for Level2 {}
impl TableLevel for Level1 {}
```
An empty enum has size zero and disappears completely after compiling. Unlike an empty struct, it's not possible to instantiate an empty enum. Since we will use `TableLevel` and `Level4` in exported types, they need to be public as well.

To differentiate the P1 table from the other tables, we introduce a `HierachicalLevel` trait, which is a subtrait of `TableLevel`. But we implement it only for the levels 4, 3, and 2:

```rust
trait HierachicalLevel: TableLevel {}

impl HierachicalLevel for Level4 {}
impl HierachicalLevel for Level3 {}
impl HierachicalLevel for Level2 {}
```

Now we add the level parameter to the `Table` type:

```rust
pub struct Table<L: TableLevel> {
    entries: [Entry; ENTRY_COUNT],
    level: PhantomData<L>,
}
```
We need to use [PhantomData] here because unused type parameters are not allowed in Rust.

[PhantomData]: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-type-parameters

Since we changed the `Table` type, we need to update every use of it:

```rust
pub const P4: *mut Table<Level4> = 0xffffffff_fffff000 as *mut _;
...
impl<L> Table<L> where L: TableLevel
{
    pub fn zero(&mut self) {...}
}

impl<L> Table<L> where L: HierachicalLevel
{
    pub fn next_table(&self, index: usize) -> Option<&Table<???>> {...}

    pub fn next_table_mut(&mut self, index: usize) -> Option<&mut Table<???>> {...}

    fn next_table_address(&self, index: usize) -> Option<usize> {...}
}

impl<L> Index<usize> for Table<L> where L: TableLevel {...}

impl<L> IndexMut<usize> for Table<L> where L: TableLevel {...}
```
Now the `next_table` methods are only available for P4, P3, and P2 tables. But they have the incomplete return type `Table<???>` now. What should we fill in for the `???`?

For a P4 table we would like to return a `Table<Level3>`, for a P3 table a `Table<Level2>`, and for a P2 table a `Table<Level1>`. So we want to return a table of the _next level_. So let's add a associated `NextLevel` type to the `HierachicalLevel` trait:

```rust
trait HierachicalLevel: TableLevel {
    type NextLevel: TableLevel;
}

impl HierachicalLevel for Level4 {
    type NextLevel = Level3;
}

impl HierachicalLevel for Level3 {
    type NextLevel = Level2;
}

impl HierachicalLevel for Level2 {
    type NextLevel = Level1;
}
```

Now we can replace the `Table<???>` types with `Table<L::NextLevel>` types and our code works as intended. You can try it with a simple test function:

```rust
fn test() {
    let p4 = unsafe { &*P4 };
    p4.next_table(42)
      .and_then(|p3| p3.next_table(1337))
      .and_then(|p2| p2.next_table(0xdeadbeaf))
      .and_then(|p1| p1.next_table(0xcafebabe))
}
```
Most of the indexes are completely out of bounds, so it would panic if it's called. But we don't need to call it since it already fails at compile time:

```
error: no method named `next_table` found for type
  `&memory::paging::table::Table<memory::paging::table::Level1>`
  in the current scope
```
Now remember that this is bare metal kernel code. We just used type system magic to make low-level page table manipulations safer. Rust is just awesome!





# OLD

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


## Translating addresses
Now we can use the recursive mapping to translate virtual address manually. We will create a function that takes a virtual address and returns the corresponding physical address:

```rust
pub fn translate(virtual_address: usize) -> Option<PhysicalAddress> {
    let page = Page::containing_address(virtual_address);
    let offset = virtual_address % PAGE_SIZE;

    let frame_number = {
        let p4_entry = page.p4_table().entry(page.p4_index());
        assert!(!p4_entry.flags().contains(HUGE_PAGE));
        if !p4_entry.flags().contains(PRESENT) {
            return None;
        }

        let p3_entry = unsafe { page.p3_table() }.entry(page.p3_index());
        if !p3_entry.flags().contains(PRESENT) {
            return None;
        }
        if p3_entry.flags().contains(HUGE_PAGE) {
            // 1GiB page (address must be 1GiB aligned)
            let start_frame_number = p3_entry.pointed_frame().number;
            assert!(start_frame_number % (ENTRY_COUNT * ENTRY_COUNT) == 0);
            start_frame_number + page.p2_index() * ENTRY_COUNT + page.p1_index()
        } else {
            // 2MiB or 4KiB page
            let p2_entry = unsafe { page.p2_table() }.entry(page.p2_index());
            if !p2_entry.flags().contains(PRESENT) {
                return None;
            }
            if p2_entry.flags().contains(HUGE_PAGE) {
                // 2MiB page (address must be 2MiB aligned)
                let start_frame_number = p2_entry.pointed_frame().number;
                assert!(start_frame_number % ENTRY_COUNT == 0);
                start_frame_number + page.p1_index()
            } else {
                // standard 4KiB page
                let p1_entry = unsafe { page.p1_table() }.entry(page.p1_index());
                assert!(!p1_entry.flags().contains(HUGE_PAGE));
                if !p1_entry.flags().contains(PRESENT) {
                    return None;
                }
                p1_entry.pointed_frame().number
            }
        }
    };
    Some(frame_number * PAGE_SIZE + offset)
}
```
(It's just some naive code and feels quite repeative… I'm open for alternative solutions)

TODO

## Modifying Entries
To modify page table entries, we add a `set_entry` function to `Table`:

```rust
fn set_entry(&mut self, index: usize, value: TableEntry) {
    assert!(index < ENTRY_COUNT);
    let entry_address = self.0.start_address() + index * ENTRY_SIZE;
    unsafe { *(entry_address as *mut _) = value }
}
```

And to create new entries, we add some `TableEntry` constructors:

```rust
const fn unused() -> TableEntry {
    TableEntry(0)
}

fn new(frame: Frame, flags: TableEntryFlags) -> TableEntry {
    let frame_addr = (frame.number << 12) & 0x000fffff_fffff000;
    TableEntry((frame_addr as u64) | flags.bits())
}
```

## Mapping Pages
To map

```rust
pub fn map_to<A>(page: &Page, frame: Frame, flags: TableEntryFlags,
    allocator: &mut A) where A: FrameAllocator
{
    let p4_index = page.p4_index();
    let p3_index = page.p3_index();
    let p2_index = page.p2_index();
    let p1_index = page.p1_index();

    let mut p4 = page.p4_table();
    if !p4.entry(p4_index).flags().contains(PRESENT) {
        let frame = allocator.allocate_frame().expect("no frames available");
        p4.set_entry(p4_index, TableEntry::new(frame, PRESENT | WRITABLE));
        unsafe { page.p3_table() }.zero();
    }
    let mut p3 = unsafe { page.p3_table() };
    if !p3.entry(p3_index).flags().contains(PRESENT) {
        let frame = allocator.allocate_frame().expect("no frames available");
        p3.set_entry(p3_index, TableEntry::new(frame, PRESENT | WRITABLE));
        unsafe { page.p2_table() }.zero();
    }
    let mut p2 = unsafe { page.p2_table() };
    if !p2.entry(p2_index).flags().contains(PRESENT) {
        let frame = allocator.allocate_frame().expect("no frames available");
        p2.set_entry(p2_index, TableEntry::new(frame, PRESENT | WRITABLE));
        unsafe { page.p1_table() }.zero();
    }
    let mut p1 = unsafe { page.p1_table() };
    assert!(!p1.entry(p1_index).flags().contains(PRESENT));
    p1.set_entry(p1_index, TableEntry::new(frame, flags));
}
```

## Unmapping Pages

## Switching Page Tables
