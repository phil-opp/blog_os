+++
title = "Kernel Heap"
weight = 8
path = "kernel-heap"
aliases = ["kernel-heap.html"]
date  = 2016-04-11
updated = "2017-11-19"
template = "first-edition/page.html"
+++

In the previous posts we created a [frame allocator] and a [page table module]. Now we are ready to create a kernel heap and a memory allocator. Thus, we will unlock `Box`, `Vec`, `BTreeMap`, and the rest of the [alloc] crate.

[frame allocator]: @/first-edition/posts/05-allocating-frames/index.md
[page table module]: @/first-edition/posts/06-page-tables/index.md
[alloc]: https://doc.rust-lang.org/nightly/alloc/index.html

<!-- more -->

As always, you can find the complete source code on [GitHub]. Please file [issues] for any problems, questions, or improvement suggestions. There is also a comment section at the end of this page.

[GitHub]: https://github.com/phil-opp/blog_os/tree/first_edition_post_8
[issues]: https://github.com/phil-opp/blog_os/issues

## Introduction
The _heap_ is the memory area for long-lived allocations. The programmer can access it by using types like [Box][Box rustbyexample] or [Vec]. Behind the scenes, the compiler manages that memory by inserting calls to some memory allocator. By default, Rust links to the [jemalloc] allocator (for binaries) or the system allocator (for libraries). However, both rely on [system calls] such as [sbrk] and are thus unusable in our kernel. So we need to create and link our own allocator.

[Box rustbyexample]: https://doc.rust-lang.org/rust-by-example/std/box.html
[Vec]: https://doc.rust-lang.org/book/vectors.html
[jemalloc]: http://jemalloc.net/
[system calls]: https://en.wikipedia.org/wiki/System_call
[sbrk]: https://en.wikipedia.org/wiki/Sbrk

A good allocator is fast and reliable. It also effectively utilizes the available memory and keeps [fragmentation] low. Furthermore, it works well for concurrent applications and scales to any number of processors. It even optimizes the memory layout with respect to the CPU caches to improve [cache locality] and avoid [false sharing].

[cache locality]: https://www.geeksforgeeks.org/locality-of-reference-and-cache-operation-in-cache-memory/
[fragmentation]: https://en.wikipedia.org/wiki/Fragmentation_(computing)
[false sharing]: https://mechanical-sympathy.blogspot.de/2011/07/false-sharing.html

These requirements make good allocators pretty complex. For example, [jemalloc] has over 30.000 lines of code. This complexity is out of scope for our kernel, so we will create a much simpler allocator. Nevertheless, it should suffice for the foreseeable future, since we'll allocate only when it's absolutely necessary.

## The Allocator Interface

The allocator interface in Rust is defined through the [`Alloc` trait], which looks like this:

[`Alloc` trait]: https://doc.rust-lang.org/1.20.0/alloc/allocator/trait.Alloc.html

```rust
pub unsafe trait Alloc {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr>;
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout);
    […] // about 13 methods with default implementations
}
```

The `alloc` method should allocate a memory block with the size and alignment given through `Layout` parameter. The `deallocate` method should free such memory blocks again. Both methods are `unsafe`, as is the trait itself. This has different reasons:

- Implementing the `Alloc` trait is unsafe, because the implementation must satisfy a set of contracts. Among other things, pointers returned by `alloc` must point to valid memory and adhere to the `Layout` requirements.
- Calling `alloc` is unsafe because the caller must ensure that the passed layout does not have size zero. I think this is because of compatibility reasons with existing C-allocators, where zero-sized allocations are undefined behavior.
- Calling `dealloc` is unsafe because the caller must guarantee that the passed parameters adhere to the contract. For example, `ptr` must denote a valid memory block allocated via this allocator.

To set the system allocator, the `global_allocator` attribute can be added to a `static` that implements `Alloc` for a shared reference of itself. For example:

```rust
#[global_allocator]
static MY_ALLOCATOR: MyAllocator = MyAllocator {...};

impl<'a> Alloc for &'a MyAllocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {...}
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {...}
}
```

Note that `Alloc` needs to be implemented for `&MyAllocator`, not for `MyAllocator`. The reason is that the `alloc` and `dealloc` methods require mutable `self` references, but there's no way to get such a reference safely from a `static`. By requiring implementations for `&MyAllocator`, the global allocator interface avoids this problem and pushes the burden of synchronization onto the user.

## Including the alloc crate
The `Alloc` trait is part of the `alloc` crate, which like `core` is a subset of Rust's standard library. Apart from the trait, the crate also contains the standard types that require allocations such as `Box`, `Vec` and `Arc`. We can include it through a simple `extern crate`:

```rust
// in src/lib.rs
#![feature(alloc)] // the alloc crate is still unstable

[...]

#[macro_use]
extern crate alloc;
```

We don't need to add anything to our Cargo.toml, since the `alloc` crate is part of the standard library and shipped with the Rust compiler. The `alloc` crate provides the [format!] and [vec!] macros, so we use `#[macro_use]` to import them.

[format!]: https://doc.rust-lang.org/1.10.0/collections/macro.format!.html
[vec!]: https://doc.rust-lang.org/1.10.0/collections/macro.vec!.html

When we try to compile our crate now, the following error occurs:

```
error[E0463]: can't find crate for `alloc`
  --> src/lib.rs:10:1
   |
16 | extern crate alloc;
   | ^^^^^^^^^^^^^^^^^^^ can't find crate
```

The problem is that [`xargo`] only cross compiles `libcore` by default. To also cross compile the `alloc` crate, we need to create a file named `Xargo.toml` in our project root (right next to the `Cargo.toml`) with the following content:

[`xargo`]: https://github.com/japaric/xargo

```toml
[target.x86_64-blog_os.dependencies]
alloc = {}
```

This instructs `xargo` that we also need `alloc`. It still doesn't compile, since we need to define a global allocator in order to use the `alloc` crate:

```
error: no #[default_lib_allocator] found but one is required; is libstd not linked?
```

## A Bump Allocator

For our first allocator, we start simple. We create a `memory::heap_allocator` module containing a so-called _bump allocator_:

```rust
// in src/memory/mod.rs

mod heap_allocator;

// in src/memory/heap_allocator.rs

use alloc::heap::{Alloc, AllocErr, Layout};

/// A simple allocator that allocates memory linearly and ignores freed memory.
#[derive(Debug)]
pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
}

impl BumpAllocator {
    pub const fn new(heap_start: usize, heap_end: usize) -> Self {
        Self { heap_start, heap_end, next: heap_start }
    }
}

unsafe impl Alloc for BumpAllocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        let alloc_start = align_up(self.next, layout.align());
        let alloc_end = alloc_start.saturating_add(layout.size());

        if alloc_end <= self.heap_end {
            self.next = alloc_end;
            Ok(alloc_start as *mut u8)
        } else {
            Err(AllocErr::Exhausted{ request: layout })
        }
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        // do nothing, leak memory
    }
}
```

We also need to add `#![feature(allocator_api)]` to our `lib.rs`, since the allocator API is still unstable.

The `heap_start` and `heap_end` fields contain the start and end address of our kernel heap. The `next` field contains the next free address and is increased after every allocation. To `allocate` a memory block we align the `next` address using the `align_up` function (described below). Then we add up the desired `size` and make sure that we don't exceed the end of the heap. We use a saturating add so that the `alloc_end` cannot overflow, which could lead to an invalid allocation. If everything goes well, we update the `next` address and return a pointer to the start address of the allocation. Else, we return `None`.

### Alignment
In order to simplify alignment, we add `align_down` and `align_up` functions:

``` rust
/// Align downwards. Returns the greatest x with alignment `align`
/// so that x <= addr. The alignment must be a power of 2.
pub fn align_down(addr: usize, align: usize) -> usize {
    if align.is_power_of_two() {
        addr & !(align - 1)
    } else if align == 0 {
        addr
    } else {
        panic!("`align` must be a power of 2");
    }
}

/// Align upwards. Returns the smallest x with alignment `align`
/// so that x >= addr. The alignment must be a power of 2.
pub fn align_up(addr: usize, align: usize) -> usize {
    align_down(addr + align - 1, align)
}
```

Let's start with `align_down`: If the alignment is a valid power of two (i.e. in `{1,2,4,8,…}`), we use some bitwise operations to return the aligned address. It works because every power of two has exactly one bit set in its binary representation. For example, the numbers `{1,2,4,8,…}` are `{1,10,100,1000,…}` in binary. By subtracting 1 we get `{0,01,011,0111,…}`. These binary numbers have a `1` at exactly the positions that need to be zeroed in `addr`. For example, the last 3 bits need to be zeroed for a alignment of 8.

To align `addr`, we create a [bitmask] from `align-1`. We want a `0` at the position of each `1`, so we invert it using `!`. After that, the binary numbers look like this: `{…11111,…11110,…11100,…11000,…}`. Finally, we zero the correct bits using a binary `AND`.

[bitmask]: https://en.wikipedia.org/wiki/Mask_(computing)

Aligning upwards is simple now. We just increase `addr` by `align-1` and call `align_down`. We add `align-1` instead of `align` because we would otherwise waste `align` bytes for already aligned addresses.

### Reusing Freed Memory
The heap memory is limited, so we should reuse freed memory for new allocations. This sounds simple, but is not so easy in practice since allocations can live arbitrarily long (and can be freed in an arbitrary order). This means that we need some kind of data structure to keep track of which memory areas are free and which are in use. This data structure should be very optimized since it causes overheads in both space (i.e. it needs backing memory) and time (i.e. accessing and organizing it needs CPU cycles).

Our bump allocator only keeps track of the next free memory address, which doesn't suffice to keep track of freed memory areas. So our only choice is to ignore deallocations and leak the corresponding memory. Thus our allocator quickly runs out of memory in a real system, but it suffices for simple testing. Later in this post, we will introduce a better allocator that does not leak freed memory.

### Using it as System Allocator

Above we saw that we can use a static allocator as system allocator through the `global_allocator` attribute:

```rust
#[global_allocator]
static ALLOCATOR: MyAllocator = MyAllocator {...};
```

This requires an implementation of `Alloc` for `&MyAllocator`, i.e. a shared reference. If we try to add such an implementation for our bump allocator (`unsafe impl<'a> Alloc for &'a BumpAllocator`), we have a problem: Our `alloc` method requires updating the `next` field, which is not possible for a shared reference.

One solution could be to put the bump allocator behind a Mutex and wrap it into a new type, for which we can implement `Alloc` for a shared reference:

```rust
struct LockedBumpAllocator(Mutex<BumpAllocator>);

impl<'a> Alloc for &'a LockedBumpAllocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        self.0.lock().alloc(layout)
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        self.0.lock().dealloc(ptr, layout)
    }
}
```

However, there is a more interesting solution for our bump allocator that avoids locking altogether. The idea is to exploit that we only need to update a single `usize` field byusing an `AtomicUsize` type. This type uses special synchronized hardware instructions to ensure data race freedom without requiring locks.

#### A lock-free Bump Allocator
A lock-free implementation looks like this:

```rust
use core::sync::atomic::{AtomicUsize, Ordering};

/// A simple allocator that allocates memory linearly and ignores freed memory.
#[derive(Debug)]
pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: AtomicUsize,
}

impl BumpAllocator {
    pub const fn new(heap_start: usize, heap_end: usize) -> Self {
        // NOTE: requires adding #![feature(const_atomic_usize_new)] to lib.rs
        Self { heap_start, heap_end, next: AtomicUsize::new(heap_start) }
    }
}

unsafe impl<'a> Alloc for &'a BumpAllocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        loop {
            // load current state of the `next` field
            let current_next = self.next.load(Ordering::Relaxed);
            let alloc_start = align_up(current_next, layout.align());
            let alloc_end = alloc_start.saturating_add(layout.size());

            if alloc_end <= self.heap_end {
                // update the `next` pointer if it still has the value `current_next`
                let next_now = self.next.compare_and_swap(current_next, alloc_end,
                    Ordering::Relaxed);
                if next_now == current_next {
                    // next address was successfully updated, allocation succeeded
                    return Ok(alloc_start as *mut u8);
                }
            } else {
                return Err(AllocErr::Exhausted{ request: layout })
            }
        }
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        // do nothing, leak memory
    }
}
```

The implementation is a bit more complicated now. First, there is now a `loop` around the whole method body, since we might need multiple tries until we succeed (e.g. if multiple threads try to allocate at the same time). Also, the loads operation is an explicit method call now, i.e. `self.next.load(Ordering::Relaxed)` instead of just `self.next`. The ordering parameter makes it possible to restrict the automatic instruction reordering performed by both the compiler and the CPU itself. For example, it is used when implementing locks to ensure that no write to the locked variable happens before the lock is acquired. We don't have such requirements, so we use the less restrictive `Relaxed` ordering.

The heart of this lock-free method is the `compare_and_swap` call that updates the `next` address:

```rust
...
let next_now = self.next.compare_and_swap(current_next, alloc_end,
    Ordering::Relaxed);
if next_now == current_next {
    // next address was successfully updated, allocation succeeded
    return Ok(alloc_start as *mut u8);
}
...
```

Compare-and-swap is a special CPU instruction that updates a variable with a given value if it still contains the value we expect. If it doesn't, it means that another thread updated the value simultaneously, so we need to try again. The important feature is that this happens in a single uninteruptible operation (thus the name `atomic`), so no partial updates or intermediate states are possible.

In detail, `compare_and_swap` works by comparing `next` with the first argument and, in case they're equal, updates `next` with the second parameter (the previous value is returned). To find out whether a switch happened, we check the returned previous value of `next`. If it is equal to the first parameter, the values were swapped. Otherwise, we try again in the next loop iteration.

#### Setting the Global Allocator

Now we can define a static bump allocator, that we can set as system allocator:

```rust
pub const HEAP_START: usize = 0o_000_001_000_000_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

#[global_allocator]
static HEAP_ALLOCATOR: BumpAllocator = BumpAllocator::new(HEAP_START,
    HEAP_START + HEAP_SIZE);
```

We use `0o_000_001_000_000_0000` as heap start address, which is the address starting at the second `P3` entry. It doesn't really matter which address we choose here as long as it's unused. We use a heap size of 100 KiB, which should be large enough for the near future.

Putting the above in the `memory::heap_allocator` module would make most sense, but unfortunately there is currently a [weird bug][global allocator bug] in the global allocator implementation that requires putting the global allocator in the root module. I hope it's fixed soon, but until then we need to put the above lines in `src/lib.rs`. For that, we need to make the `memory::heap_allocator` module public and add an import for `BumpAllocator`. We also need to add the `#![feature(global_allocator)]` at the top of our `lib.rs`, since the `global_allocator` attribute is still unstable.

[global allocator bug]: https://github.com/rust-lang/rust/issues/44113

That's it! We have successfully created and linked a custom system allocator. Now we're ready to test it.

### Testing

We should be able to allocate memory on the heap now. Let's try it in our `rust_main`:

```rust
// in rust_main in src/lib.rs

use alloc::boxed::Box;
let heap_test = Box::new(42);
```

When we run it, a triple fault occurs and causes permanent rebooting. Let's try debug it using QEMU and objdump as described [in the previous post][qemu debugging]:

[qemu debugging]: @/first-edition/posts/07-remap-the-kernel/index.md#debugging

```
> qemu-system-x86_64 -d int -no-reboot -cdrom build/os-x86_64.iso
…
check_exception old: 0xffffffff new 0xe
     0: v=0e e=0002 i=0 cpl=0 IP=0008:0000000000102860 pc=0000000000102860
        SP=0010:0000000000116af0 CR2=0000000040000000
…
```
Aha! It's a [page fault] \(`v=0e`) and was caused by the code at `0x102860`. The code tried to write (`e=0002`) to address `0x40000000`. This address is `0o_000_001_000_000_0000` in octal, which is the `HEAP_START` address defined above. Of course it page-faults: We have forgotten to map the heap memory to some physical memory.

[page fault]: https://wiki.osdev.org/Exceptions#Page_Fault

### Some Refactoring
In order to map the heap cleanly, we do a bit of refactoring first. We move all memory initialization from our `rust_main` to a new `memory::init` function. Now our `rust_main` looks like this:

```rust
// in src/lib.rs

#[no_mangle]
pub extern "C" fn rust_main(multiboot_information_address: usize) {
    // ATTENTION: we have a very small stack and no guard page
    vga_buffer::clear_screen();
    println!("Hello World{}", "!");

    let boot_info = unsafe {
        multiboot2::load(multiboot_information_address)
    };
    enable_nxe_bit();
    enable_write_protect_bit();

    // set up guard page and map the heap pages
    memory::init(boot_info);

    use alloc::boxed::Box;
    let heap_test = Box::new(42);

    println!("It did not crash!");

    loop {}
}
```

The `memory::init` function looks like this:

```rust
// in src/memory/mod.rs

use multiboot2::BootInformation;

pub fn init(boot_info: &BootInformation) {
    let memory_map_tag = boot_info.memory_map_tag().expect(
        "Memory map tag required");
    let elf_sections_tag = boot_info.elf_sections_tag().expect(
        "Elf sections tag required");

    let kernel_start = elf_sections_tag.sections()
        .filter(|s| s.is_allocated()).map(|s| s.addr).min().unwrap();
    let kernel_end = elf_sections_tag.sections()
        .filter(|s| s.is_allocated()).map(|s| s.addr + s.size).max()
        .unwrap();

    println!("kernel start: {:#x}, kernel end: {:#x}",
             kernel_start,
             kernel_end);
    println!("multiboot start: {:#x}, multiboot end: {:#x}",
             boot_info.start_address(),
             boot_info.end_address());

    let mut frame_allocator = AreaFrameAllocator::new(
        kernel_start as usize, kernel_end as usize,
        boot_info.start_address(), boot_info.end_address(),
        memory_map_tag.memory_areas());

    paging::remap_the_kernel(&mut frame_allocator, boot_info);
}
```

We've just moved the code to a new function. However, we've sneaked some improvements in:

- An additional `.filter(|s| s.is_allocated())` in the calculation of `kernel_start` and `kernel_end`. This ignores all sections that aren't loaded to memory (such as debug sections). Thus, the kernel end address is no longer artificially increased by such sections.
- We use the `start_address()` and `end_address()` methods of `boot_info` instead of calculating the adresses manually.
- We use the alternate `{:#x}` form when printing kernel/multiboot addresses. Before, we used `0x{:x}`, which leads to the same result. For a complete list of these “alternate” formatting forms, check out the [std::fmt documentation].

[std::fmt documentation]: https://doc.rust-lang.org/nightly/std/fmt/index.html#sign0

### Safety
It is important that the `memory::init` function is called only once, because it creates a new frame allocator based on kernel and multiboot start/end. When we call it a second time, a new frame allocator is created that reassigns the same frames, even if they are already in use.

In the second call it would use an identical frame allocator to remap the kernel. The `remap_the_kernel` function would request a frame from the frame allocator to create a new page table. But the returned frame is already in use, since we used it to create our current page table in the first call. In order to initialize the new table, the function zeroes it. This is the point where everything breaks, since we zero our current page table. The CPU is unable to read the next instruction  and throws a page fault.

So we need to ensure that `memory::init` can be only called once. We could mark it as `unsafe`, which would bring it in line with Rust's memory safety rules. However, that would just push the unsafety to the caller. The caller can still accidentally call the function twice, the only difference is that the mistake needs to happen inside `unsafe` blocks.

A better solution is to insert a check at the function's beginning, that panics if the function is called a second time. This approach has a small runtime cost, but we only call it once, so it's negligible. And we avoid two `unsafe` blocks (one at the calling site and one at the function itself), which is always good.

In order to make such checks easy, I created a small crate named [once]. To add it, we run `cargo add once` and add the following to our `src/lib.rs`:

[once]: https://crates.io/crates/once

```rust
// in src/lib.rs

#[macro_use]
extern crate once;
```

The crate provides an [assert_has_not_been_called!] macro (sorry for the long name :D). We can use it to fix the safety problem easily:

[assert_has_not_been_called!]: https://docs.rs/once/0.3.2/once/macro.assert_has_not_been_called!.html

``` rust
// in src/memory/mod.rs

pub fn init(boot_info: &BootInformation) {
    assert_has_not_been_called!("memory::init must be called only once");

    let memory_map_tag = ...
    ...
}
```
That's it. Now our `memory::init` function can only be called once. The macro works by creating a static [AtomicBool] named `CALLED`, which is initialized to `false`. When the macro is invoked, it checks the value of `CALLED` and sets it to `true`. If the value was already `true` before, the macro panics.

[AtomicBool]: https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html

### Mapping the Heap
Now we're ready to map the heap pages. In order to do it, we need access to the `ActivePageTable` or `Mapper` instance (see the [page table] and [kernel remapping] posts). For that we return it from the `paging::remap_the_kernel` function:

[page table]: @/first-edition/posts/06-page-tables/index.md
[kernel remapping]: @/first-edition/posts/07-remap-the-kernel/index.md

```rust
// in src/memory/paging/mod.rs

pub fn remap_the_kernel<A>(allocator: &mut A, boot_info: &BootInformation)
    -> ActivePageTable // new
    where A: FrameAllocator
{
    ...
    println!("guard page at {:#x}", old_p4_page.start_address());

    active_table // new
}
```

Now we have full page table access in the `memory::init` function. This allows us to map the heap pages to physical frames:

```rust
// in src/memory/mod.rs

pub fn init(boot_info: &BootInformation) {
    ...

    let mut frame_allocator = ...;

    // below is the new part

    let mut active_table = paging::remap_the_kernel(&mut frame_allocator,
        boot_info);

    use self::paging::Page;
    use {HEAP_START, HEAP_SIZE};

    let heap_start_page = Page::containing_address(HEAP_START);
    let heap_end_page = Page::containing_address(HEAP_START + HEAP_SIZE-1);

    for page in Page::range_inclusive(heap_start_page, heap_end_page) {
        active_table.map(page, paging::WRITABLE, &mut frame_allocator);
    }
}
```

The `Page::range_inclusive` function is just a copy of the `Frame::range_inclusive` function:

```rust
// in src/memory/paging/mod.rs

#[derive(…, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page {...}

impl Page {
    ...
    pub fn range_inclusive(start: Page, end: Page) -> PageIter {
        PageIter {
            start: start,
            end: end,
        }
    }
}

pub struct PageIter {
    start: Page,
    end: Page,
}

impl Iterator for PageIter {
    type Item = Page;

    fn next(&mut self) -> Option<Page> {
        if self.start <= self.end {
            let page = self.start;
            self.start.number += 1;
            Some(page)
        } else {
            None
        }
    }
}
```

Now we map the whole heap to physical pages. This needs some time and might introduce a noticeable delay when we increase the heap size in the future. Another drawback is that we consume a large amount of physical frames even though we might not need the whole heap space. We will fix these problems in a future post by mapping the pages lazily.

### It works!

Now `Box` and `Vec` should work. For example:

```rust
// in rust_main in src/lib.rs

use alloc::boxed::Box;
let mut heap_test = Box::new(42);
*heap_test -= 15;
let heap_test2 = Box::new("hello");
println!("{:?} {:?}", heap_test, heap_test2);

let mut vec_test = vec![1,2,3,4,5,6,7];
vec_test[3] = 42;
for i in &vec_test {
    print!("{} ", i);
}
```

We can also use all other types of the `alloc` crate, including:

- the reference counted pointers [Rc] and [Arc]
- the owned string type [String] and the [format!] macro
- [Linked List]
- the growable ring buffer [VecDeque]
- [BinaryHeap]
- [BTreeMap] and [BTreeSet]

[Rc]: https://doc.rust-lang.org/1.10.0/alloc/rc/
[Arc]: https://doc.rust-lang.org/1.10.0/alloc/arc/
[String]: https://doc.rust-lang.org/1.10.0/collections/string/struct.String.html
[Linked List]: https://doc.rust-lang.org/1.10.0/collections/linked_list/struct.LinkedList.html
[VecDeque]: https://doc.rust-lang.org/1.10.0/collections/vec_deque/struct.VecDeque.html
[BinaryHeap]: https://doc.rust-lang.org/1.10.0/collections/binary_heap/struct.BinaryHeap.html
[BTreeMap]: https://doc.rust-lang.org/1.10.0/collections/btree_map/struct.BTreeMap.html
[BTreeSet]: https://doc.rust-lang.org/1.10.0/collections/btree_set/struct.BTreeSet.html

## A better Allocator
Right now, we leak every freed memory block. Thus, we run out of memory quickly, for example, by creating a new `String` in each iteration of a loop:

```rust
// in rust_main in src/lib.rs

for i in 0..10000 {
    format!("Some String");
}
```

To fix this, we need to create an allocator that keeps track of freed memory blocks and reuses them if possible. This introduces some challenges:

- We need to keep track of a possibly unlimited number of freed blocks. For example, an application could allocate `n` one-byte sized blocks and free every second block, which creates `n/2` freed blocks. We can't rely on any upper bound of freed block since `n` could be arbitrarily large.
- We can't use any of the collections from above, since they rely on allocations themselves. (It might be possible as soon as [RFC #1398] is [implemented][#32838], which allows user-defined allocators for specific collection instances.)
- We need to merge adjacent freed blocks if possible. Otherwise, the freed memory is no longer usable for large allocations. We will discuss this point in more detail below.
- Our allocator should search the set of freed blocks quickly and keep fragmentation low.

[RFC #1398]: https://github.com/rust-lang/rfcs/blob/master/text/1398-kinds-of-allocators.md
[#32838]: https://github.com/rust-lang/rust/issues/32838

### Creating a List of freed Blocks

Where do we store the information about an unlimited number of freed blocks? We can't use any fixed size data structure since it could always be too small for some allocation sequences. So we need some kind of dynamically growing set.

One possible solution could be to use an array-like data structure that starts at some unused virtual address. If the array becomes full, we increase its size and map new physical frames as backing storage. This approach would require a large part of the virtual address space since the array could grow significantly. We would need to create a custom implementation of a growable array and manipulate the page tables when deallocating. It would also consume a possibly large number of physical frames as backing storage.

We will choose another solution with different tradoffs. It's not clearly “better” than the approach above and has significant disadvantages itself. However, it has one big advantage: It does not need any additional physical or virtual memory at all. This makes it less complex since we don't need to manipulate any page tables. The idea is the following:

A freed memory block is not used anymore and no one needs the stored information. It is still mapped to a virtual address and backed by a physical page. So we just store the information about the freed block _in the block itself_.  We keep a pointer to the first block and store a pointer to the next block in each block. Thus, we create a single linked list:

![Linked List Allocator](overview.svg)

In the following, we call a freed block a _hole_. Each hole stores its size and a pointer to the next hole. If a hole is larger than needed, we leave the remaining memory unused. By storing a pointer to the first hole, we are able to traverse the complete list.

#### Initialization
When the heap is created, all of its memory is unused. Thus, it forms a single large hole:

![Heap Initialization](initialization.svg)

The optional pointer to the next hole is set to `None`.

#### Allocation
In order to allocate a block of memory, we need to find a hole that satisfies the size and alignment requirements. If the found hole is larger than required, we split it into two smaller holes. For example, when we allocate a 24 byte block right after initialization, we split the single hole into a hole of size 24 and a hole with the remaining size:

![split hole](split-hole.svg)

Then we use the new 24 byte hole to perform the allocation:

![24 bytes allocated](allocate.svg)

To find a suitable hole, we can use several search strategies:

- **best fit**: Search the whole list and choose the _smallest_ hole that satisfies the requirements.
- **worst fit**: Search the whole list and choose the _largest_ hole that satisfies the requirements.
- **first fit**: Search the list from the beginning and choose the _first_ hole that satisfies the requirements.

Each strategy has its advantages and disadvantages. Best fit uses the smallest hole possible and leaves larger holes for large allocations. But splitting the smallest hole might create a tiny hole, which is too small for most allocations. In contrast, the worst fit strategy always chooses the largest hole. Thus, it does not create tiny holes, but it consumes the large block, which might be required for large allocations.

For our use case, the best fit strategy is better than worst fit. The reason is that we have a minimal hole size of 16 bytes, since each hole needs to be able to store a size (8 bytes) and a pointer to the next hole (8 bytes). Thus, even the best fit strategy leads to holes of usable size. Furthermore, we will need to allocate very large blocks occasionally (e.g. for [DMA] buffers).

[DMA]: https://en.wikipedia.org/wiki/Direct_memory_access

However, both best fit and worst fit have a significant problem: They need to scan the whole list for each allocation in order to find the optimal block. This leads to long allocation times if the list is long. The first fit strategy does not have this problem, as it returns as soon as it finds a suitable hole. It is fairly fast for small allocations and might only need to scan the whole list for large allocations.

#### Deallocation
To deallocate a block of memory, we can just insert its corresponding hole somewhere into the list. However, we need to merge adjacent holes. Otherwise, we are unable to reuse the freed memory for larger allocations. For example:

![deallocate memory, which leads to adjacent holes](deallocate.svg)

In order to use these adjacent holes for a large allocation, we need to merge them to a single large hole first:

![merge adjacent holes and allocate large block](merge-holes-and-allocate.svg)

The easiest way to ensure that adjacent holes are always merged, is to keep the hole list sorted by address. Thus, we only need to check the predecessor and the successor in the list when we free a memory block. If they are adjacent to the freed block, we merge the corresponding holes. Else, we insert the freed block as a new hole at the correct position.

### Implementation
The detailed implementation would go beyond the scope of this post, since it contains several hidden difficulties. For example:

- Several merge cases: Merge with the previous hole, merge with the next hole, merge with both holes.
- We need to satisfy the alignment requirements, which requires additional splitting logic.
- The minimal hole size of 16 bytes: We must not create smaller holes when splitting a hole.

I created the [linked_list_allocator] crate to handle all of these cases. It consists of a [Heap struct] that provides an `allocate_first_fit` and a `deallocate` method. It also contains a [LockedHeap] type that wraps `Heap` into spinlock so that it's usable as a static system allocator. If you are interested in the implementation details, check out the [source code][linked_list_allocator source].

[linked_list_allocator]: https://docs.rs/crate/linked_list_allocator/0.4.1
[Heap struct]: https://docs.rs/linked_list_allocator/0.4.1/linked_list_allocator/struct.Heap.html
[LockedHeap]: https://docs.rs/linked_list_allocator/0.4.1/linked_list_allocator/struct.LockedHeap.html
[linked_list_allocator source]: https://github.com/phil-opp/linked-list-allocator

We need to add the extern crate to our `Cargo.toml` and our `lib.rs`:

``` shell
> cargo add linked_list_allocator
```

```rust
// in src/lib.rs
extern crate linked_list_allocator;
```

Now we can change our global allocator:

```rust
use linked_list_allocator::LockedHeap;

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();
```

We can't initialize the linked list allocator statically, since it needs to initialize the first hole (like described [above](#initialization)). This can't be done at compile time, so the function can't be a `const` function. Therefore we can only create an empty heap and initialize it later at runtime. For that, we add the following lines to our `rust_main` function:

```rust
// in src/lib.rs

#[no_mangle]
pub extern "C" fn rust_main(multiboot_information_address: usize) {
    […]

    // set up guard page and map the heap pages
    memory::init(boot_info);

    // initialize the heap allocator
    unsafe {
        HEAP_ALLOCATOR.lock().init(HEAP_START, HEAP_START + HEAP_SIZE);
    }
    […]
}
```

It is important that we initialize the heap _after_ mapping the heap pages, since the init function writes to the heap memory (the first hole).

Our kernel uses the new allocator now, so we can deallocate memory without leaking it. The example from above should work now without causing an OOM situation:

```rust
// in rust_main in src/lib.rs

for i in 0..10000 {
    format!("Some String");
}
```

### Performance
The linked list based approach has some performance problems. Each allocation or deallocation might need to scan the complete list of holes in the worst case. However, I think it's good enough for now, since our heap will stay relatively small for the near future. When our allocator becomes a performance problem eventually, we can just replace it with a faster alternative.

## Summary
Now we're able to use heap storage in our kernel without leaking memory. This allows us to effectively process dynamic data such as user supplied strings in the future. We can also use `Rc` and `Arc` to create types with shared ownership. And we have access to various data structures such as `Vec` or `Linked List`, which will make our lives much easier. We even have some well tested and optimized [binary heap] and [B-tree] implementations!

[binary heap]:https://en.wikipedia.org/wiki/Binary_heap
[B-tree]: https://en.wikipedia.org/wiki/B-tree

## What's next?
This post concludes the section about memory management for now. We will revisit this topic eventually, but now it's time to explore other topics. The upcoming posts will be about CPU exceptions and interrupts. We will catch all page, double, and triple faults and create a driver to read keyboard input. The [next post] starts by setting up a so-called _Interrupt Descriptor Table_.

[next post]: @/first-edition/posts/09-handling-exceptions/index.md
