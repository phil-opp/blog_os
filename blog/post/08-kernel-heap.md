+++
title = "Kernel Heap"
date = "2016-04-11"
+++

In the previous posts we have created a [frame allocator] and a [page table module]. Now we are ready to create a kernel heap and a memory allocator. Thus, we will unlock `Box`, `Vec`, `BTreeMap`, and the rest of the [alloc] and [collections] crates.

[frame allocator]: {{% relref "05-allocating-frames.md" %}}
[page table module]: {{% relref "06-page-tables.md" %}}
[alloc]: https://doc.rust-lang.org/nightly/alloc/index.html
[collections]: https://doc.rust-lang.org/nightly/collections/index.html

<!--more--><aside id="toc"></aside>

As always, you can find the complete source code on [Github]. Please file [issues] for any problems, questions, or improvement suggestions. There is also a comment section at the end of this page.

[Github]: https://github.com/phil-opp/blog_os/tree/kernel_heap
[issues]: https://github.com/phil-opp/blog_os/issues

## Introduction
The _heap_ is the memory area for long-lived allocations. The programmer can access it by using types like [Box][Box rustbyexample] or [Vec]. Behind the scenes, the compiler manages that memory by inserting calls to some memory allocator. By default, Rust links to the [jemalloc] allocator (for binaries) or the system allocator (for libraries). However, both rely on [system calls] such as [sbrk] and are thus unusable in our kernel. So we need to create and link our own allocator.

[Box rustbyexample]: http://rustbyexample.com/std/box.html
[Vec]: https://doc.rust-lang.org/book/vectors.html
[jemalloc]: http://www.canonware.com/jemalloc/
[system calls]: https://en.wikipedia.org/wiki/System_call
[sbrk]: https://en.wikipedia.org/wiki/Sbrk

A good allocator is fast and reliable. It also effectively utilizes the available memory and keeps [fragmentation] low. Furthermore, it works well for concurrent applications and scales to any number of processors. It even optimizes the memory layout with respect to the CPU caches to improve [cache locality] and avoid [false sharing].

[cache locality]: http://docs.cray.com/books/S-2315-50/html-S-2315-50/qmeblljm.html
[fragmentation]: https://en.wikipedia.org/wiki/Fragmentation_(computing)
[false sharing]: http://mechanical-sympathy.blogspot.de/2011/07/false-sharing.html

These requirements make good allocators pretty complex. For example, [jemalloc] has over 30.000 lines of code. This complexity is out of scope for our kernel, so we will create a much simpler allocator. However, it should suffice for the foreseeable future, since we'll allocate only when it's absolutely necessary.

## A Bump Allocator

For our own allocator, we start simple. We create an allocator crate in a new `libs` subfolder:

``` shell
> mkdir libs
> cd libs
> cargo new bump_allocator
> cd bump_allocator
```

Normally, this subcrate would have its own `Cargo.lock` and its own `target` output folder. To avoid this, we can create a so-called _[workspace]_. For that we only need to add a single line to our `Cargo.toml`:

[workspace]: http://doc.crates.io/manifest.html#the-workspace-section

```toml
# in Cargo.toml

[workspace]
```
We don't need to add any keys to the `workspace` section, since they have [sensible defaults]. Now our `bump_allocator` subcrate shares the `Cargo.toml` and `target` folder of the root project.

[sensible defaults]: https://github.com/rust-lang/rfcs/blob/master/text/1525-cargo-workspace.md#implicit-relations

### Implementation
Our allocator is very basic. It only keeps track of the next free address:

``` rust
// in libs/bump_allocator/src/lib.rs

#![feature(const_fn)]

#[derive(Debug)]
struct BumpAllocator {
    heap_start: usize,
    heap_size: usize,
    next: usize,
}

impl BumpAllocator {
    /// Create a new allocator, which uses the memory in the
    /// range [heap_start, heap_start + heap_size).
    const fn new(heap_start: usize, heap_size: usize) -> BumpAllocator {
        BumpAllocator {
            heap_start: heap_start,
            heap_size: heap_size,
            next: heap_start,
        }
    }

    /// Allocates a block of memory with the given size and alignment.
    fn allocate(&mut self, size: usize, align: usize) -> Option<*mut u8> {
    	let alloc_start = align_up(self.next, align);
        let alloc_end = alloc_start + size;

        if alloc_end <= self.heap_start + self.heap_size {
            self.next = alloc_end;
            Some(alloc_start as *mut u8)
        } else {
            None
        }
    }
}
```

The `heap_start` and `heap_size` fields just contain the start address and size of our kernel heap. The `next` field contains the next free address and is increased after every allocation. To `allocate` a memory block we align the `next` address using the `align_up` function (decribed below). Then we add up the desired `size` and make sure that we don't exceed the end of the heap. If everything goes well, we update the `next` address and return a pointer to the start address of the allocation. Else, we return `None`.

Note that we need to add a feature flag at the beginning of the file, because we've marked the `new` function as `const`. [Const functions] are unstable, so we need to add the `#![feature(const_fn)]` flag.

[Const functions]: https://github.com/rust-lang/rust/issues/24111

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

Let's start with `align_down`: If the alignment is a valid power of two (i.e. in `{1,2,4,8,…}`), we use some bit-fiddling to return the aligned address. It works because every power of two has exactly one bit set in its binary representation. For example, the numbers `{1,2,4,8,…}` are `{1,10,100,1000,…}` in binary. By subtracting 1 we get `{0,01,011,0111,…}`. These binary numbers have a `1` at exactly the positions that need to be zeroed in `addr`. For example, the last 3 bits need to be zeroed for a alignment of 8.

To align `addr`, we create a [bitmask] from `align-1`. We want a `0` at the position of each `1`, so we invert it using `!`. After that, the binary numbers look like this: `{…11111,…11110,…11100,…11000,…}`. Finally, we zero the correct bits using a binary `AND`.

[bitmask]: https://en.wikipedia.org/wiki/Mask_(computing)

Aligning upwards is simple now. We just increase `addr` by `align-1` and call `align_down`. We add `align-1` instead of `align` because we would otherwise waste `align` bytes for already aligned addresses.

### Deallocate
But how do we deallocate memory in our bump allocator? Well, we don't ;). We just leak all freed memory for now. Thus our allocator quickly runs out of memory in a real system. On the other hand, it's as fast as an allocator can get: It just increases a single variable when allocating and does nothing at all when deallocating. And RAM is cheap nowadays, right? :)

(Don't worry, we will introduce a better allocator later in this post.)

## Custom Allocators in Rust
In order to use our crate as system allocator, we add some attributes at the beginning of the file:

``` rust
// in libs/bump_allocator/src/lib.rs

#![feature(allocator)]

#![allocator]
#![no_std]
```
The `#![allocator]` attribute tells the compiler that it should not link a default allocator when this crate is linked. The attribute is unstable and feature-gated, so we need to add `#![feature(allocator)]` as well. Allocator crates must not depend on [liballoc], because this would introduce a circular dependency. Thus, allocator crates can't use the standard library either (as it depends on `liballoc`). Therefore all allocator crates must be marked as `#![no_std]`.

[liballoc]: https://doc.rust-lang.org/nightly/alloc/index.html

According to [the book][custom-allocators], an allocator crate needs to implement the following five functions:

[custom-allocators]: https://doc.rust-lang.org/book/custom-allocators.html

``` rust
#[no_mangle]
pub extern fn __rust_allocate(size: usize, align: usize) -> *mut u8 {}

#[no_mangle]
pub extern fn __rust_usable_size(size: usize, align: usize) -> usize {}

#[no_mangle]
pub extern fn __rust_deallocate(ptr: *mut u8, size: usize, align: usize) {}

#[no_mangle]
pub extern fn __rust_reallocate(ptr: *mut u8, size: usize, new_size: usize,
                                align: usize) -> *mut u8 {}

#[no_mangle]
pub extern fn __rust_reallocate_inplace(ptr: *mut u8, size: usize,
                                        new_size: usize, align: usize)
                                        -> usize {}
```

These functions are highly unstable and the compiler does not check their types. So make sure that the type, number, and order of parameters are correct when you implement it.

Let's look at each function individually:

- The `__rust_allocate` function allocates a block of memory with the given size (in bytes) and alignment. _Alignment_ means that the start address of the allocation needs to be a multiple of the `align` parameter. This is required because some CPUs can only access e.g. 4 byte aligned addresses. The alignment is always a power of 2.
- The `__rust_usable_size` returns the usable size of an allocation created with the specified size and alignment. The usable size is at least `size`, but might be larger if the allocator uses fixed block sizes. For example, a [buddy allocator] rounds the size of each allocation to the next power of two.
- The `__rust_deallocate` function frees the memory block referenced `ptr` again. The `size` and `align` parameters contain the values that were used to create the allocation. Thus the allocator knows exactly how much memory it needs to free. In constrast, the [free function] of C only has a single `ptr` argument. So a C allocator needs to [maintain information][c free info] about the size of each block itself. In Rust, the compiler maintains this information for us.
- The `__rust_reallocate` function changes the size of the block referenced by `ptr` from `size` to `new_size`. If it's possible to do in in-place, the function resizes the block and returns `ptr` again. Else, it allocates a new block of `new_size` and copies the memory contents from the old block. Then it frees the old block and returns the pointer to the new block.
- The `__rust_reallocate_inplace` function tries to change the size of the block referenced by `ptr` from `size` to `new_size` without relocating the memory block. If it succeeds, it returns `usable_size(new_size, align)`, else it returns `usable_size(size, align)`.

[buddy allocator]: https://en.wikipedia.org/wiki/Buddy_memory_allocation
[free function]: http://www.cplusplus.com/reference/cstdlib/free/
[c free info]: http://stackoverflow.com/questions/1518711/how-does-free-know-how-much-to-free

A more detailed documentation for these functions can be found in the [API docs for alloc::heap][alloc::heap]. Note that all of these functions and custom allocators in general are _unstable_ (as indicated by the `allocator` feature gate).

[alloc::heap]: https://doc.rust-lang.org/nightly/alloc/heap/

### Implementation
Let's implement the allocation functions using our new allocator. First we need a way to access the allocator. The functions do not know anything about our allocator, so we can only access it through a `static`:

``` rust
// in libs/bump_allocator/src/lib.rs

use spin::Mutex;

extern crate spin;

pub const HEAP_START: usize = 0o_000_001_000_000_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

static BUMP_ALLOCATOR: Mutex<BumpAllocator> = Mutex::new(
    BumpAllocator::new(HEAP_START, HEAP_SIZE));
```

We use `0o_000_001_000_000_0000` as heap start address, which is the address starting at the second `P3` entry. It doesn't really matter which address we choose here as long as it's unused. We use a heap size of 100 KiB, which should be large enough for the near future. The static allocator is protected by a spinlock since we need to able to modify it. Our allocator crate is distinct from our main crate, so we need to add the `spin` dependency to its `Cargo.toml` as well. The easiest way is to run `cargo add spin` (using the [cargo-edit] crate).

[cargo-edit]: https://github.com/killercup/cargo-edit

Now we can easily implement the `__rust_allocate` and `__rust_deallocate` functions:

```rust
// in libs/bump_allocator/src/lib.rs

#[no_mangle]
pub extern fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    BUMP_ALLOCATOR.lock().allocate(size, align).expect("out of memory")
}

#[no_mangle]
pub extern fn __rust_deallocate(_ptr: *mut u8, _size: usize,
    _align: usize)
{
    // just leak it
}
```
We use `expect` to panic in out of memory (OOM) situations. We could alternatively return a null pointer, which indicates an OOM situation to the Rust runtime. However, the runtime would react by aborting the process. On Linux, the abort function intentionally raises an [invalid opcode] exception, which would lead to a boot loop for our kernel. So panickying is a better solution for our kernel.

[invalid opcode]: http://wiki.osdev.org/Exceptions#Invalid_Opcode

We never allocate more memory than requested, so the `__rust_usable_size` function is simple:

```rust
#[no_mangle]
pub extern fn __rust_usable_size(size: usize, _align: usize) -> usize {
    size
}
```

In order to keep things simple, we don't support the `__rust_reallocate_inplace` function and always return the old size:

```rust
#[no_mangle]
pub extern fn __rust_reallocate_inplace(_ptr: *mut u8, size: usize,
    _new_size: usize, _align: usize) -> usize
{
    size
}
```

Now only `__rust_reallocate` is left. It's a bit more difficult, since we need to copy the contents of the old allocation to the new allocation. However, we can just steal some code from the official [reallocate implementation for unix][unix realloc]:

[unix realloc]: https://github.com/rust-lang/rust/blob/c66d2380a810c9a2b3dbb4f93a830b101ee49cc2/src/liballoc_system/lib.rs#L98-L101


``` rust
#[no_mangle]
pub extern fn __rust_reallocate(ptr: *mut u8, size: usize, new_size: usize,
                                align: usize) -> *mut u8 {
    use core::{ptr, cmp};

    // from: https://github.com/rust-lang/rust/blob/
    //     c66d2380a810c9a2b3dbb4f93a830b101ee49cc2/
    //     src/liballoc_system/lib.rs#L98-L101

    let new_ptr = __rust_allocate(new_size, align);
    unsafe { ptr::copy(ptr, new_ptr, cmp::min(size, new_size)) };
    __rust_deallocate(ptr, size, align);
    new_ptr
}
```

That's it! We have successfully created a custom allocator. Now we're ready to test it.

## Box, Vec, and Friends

In order to use our new allocator we import it in our main project:

```rust
// in src/lib.rs of our main project

extern crate bump_allocator;
```

Additionally, we need to tell cargo where our `bump_allocator` crate lives:

``` toml
# in Cargo.toml of our main project

[dependencies.bump_allocator]
path = "libs/bump_allocator"
```

Now we're able to import the `alloc` and `collections` crates in order to unlock `Box`, `Vec`, `BTreeMap`, and friends:

```rust
// in src/lib.rs of our main project

#![feature(alloc, collections)]

extern crate bump_allocator;
extern crate alloc;
#[macro_use]
extern crate collections;
```
The `collections` crate provides the [format!] and [vec!] macros, so we use `#[macro_use]` to import them.

[format!]: //doc.rust-lang.org/nightly/collections/macro.format!.html
[vec!]: https://doc.rust-lang.org/nightly/collections/macro.vec!.html

### Testing

Now we should be able to allocate memory on the heap. Let's try it in our `rust_main`:

```rust
// in rust_main in src/lib.rs

use alloc::boxed::Box;
let heap_test = Box::new(42);
```

When we run it, a triple fault occurs and causes permanent rebooting. Let's try debug it using QEMU and objdump as described [in the previous post][qemu debugging]:

[qemu debugging]: http://os.phil-opp.com/remap-the-kernel.html#debugging

```
> qemu-system-x86_64 -d int -no-reboot -cdrom build/os-x86_64.iso
…
check_exception old: 0xffffffff new 0xe
     0: v=0e e=0002 i=0 cpl=0 IP=0008:0000000000102860 pc=0000000000102860
        SP=0010:0000000000116af0 CR2=0000000040000000
…
```
Aha! It's a [page fault] (`v=0e`) and was caused by the code at `0x102860`. The code tried to write (`e=0002`) to address `0x40000000`. This address is `0o_000_001_000_000_0000` in octal, which is the `HEAP_START` address defined above. Of course it page-faults: We have forgotten to map the heap memory to some physical memory.

[page fault]: http://wiki.osdev.org/Exceptions#Page_Fault

### Some Refactoring
In order to map the heap cleanly, we do a bit of refactoring first. We move all memory initialization from our `rust_main` to a new `memory::init` function. Now our `rust_main` looks like this:

```rust
// in src/lib.rs

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

- An additional `.filter(|s| s.is_allocated())` in the calculation of `kernel_start` and `kernel_end`. This ignores all sections that aren't loaded to memory (such as debug sections). Thus, the kernel end address is no longer artifically increased by such sections.
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

[assert_has_not_been_called!]: https://phil-opp.rustdocs.org/once/macro.assert_has_not_been_called!.html

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
Now we're ready to map the heap pages. In order to do it, we need access to the `ActivePageTable` or `Mapper` instance (see the [page table] and [kernel remapping] posts). Therefore we return it from the `paging::remap_the_kernel` function:

[page table]: {{% relref "06-page-tables.md" %}}
[kernel remapping]: {{% relref "07-remap-the-kernel.md" %}}

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
    use bump_allocator::{HEAP_START, HEAP_SIZE};

    let heap_start_page = Page::containing_address(HEAP_START);
    let heap_end_page = Page::containing_address(HEAP_START + HEAP_SIZE-1);

    for page in Page::range_inclusive(heap_start_page, heap_end_page) {
        active_table.map(page, paging::WRITABLE, &mut frame_allocator);
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

We can also use all other types of the `alloc` and `collections` crates, including:

- the reference counted pointers [Rc] and [Arc]
- the owned string type [String] and the [format!] macro
- [Linked List]
- the growable ring buffer [VecDeque]
- [BinaryHeap]
- [BTreeMap] and [BTreeSet]

[Rc]: https://doc.rust-lang.org/nightly/alloc/rc/
[Arc]: https://doc.rust-lang.org/nightly/alloc/arc/
[String]: https://doc.rust-lang.org/nightly/collections/string/struct.String.html
[Linked List]: https://doc.rust-lang.org/nightly/collections/linked_list/struct.LinkedList.html
[VecDeque]: https://doc.rust-lang.org/nightly/collections/vec_deque/struct.VecDeque.html
[BinaryHeap]: https://doc.rust-lang.org/nightly/collections/binary_heap/struct.BinaryHeap.html
[BTreeMap]: https://doc.rust-lang.org/nightly/collections/btree_map/struct.BTreeMap.html
[BTreeSet]: https://doc.rust-lang.org/nightly/collections/btree_set/struct.BTreeSet.html

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

![Linked List Allocator](/images/linked-list-allocator/overview.svg)

In the following, we call a freed block a _hole_. Each hole stores its size and a pointer to the next hole. If a hole is larger than needed, we leave the remaining memory unused. By storing a pointer to the first hole, we are able to traverse the complete list.

#### Initialization
When the heap is created, all of its memory is unused. Thus, it forms a single large hole:

![Heap Initialization](/images/linked-list-allocator/initialization.svg)

The optional pointer to the next hole is set to `None`.

#### Allocation
In order to allocate a block of memory, we need to find a hole that satisfies the size and alignment requirements. If the found hole is larger than required, we split it into two smaller holes. For example, when we allocate a 24 byte block right after initialization, we split the single hole into a hole of size 24 and a hole with the remaining size:

![split hole](/images/linked-list-allocator/split-hole.svg)

Then we use the new 24 byte hole to perform the allocation:

![24 bytes allocated](/images/linked-list-allocator/allocate.svg)

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

![deallocate memory, which leads to adjacent holes](/images/linked-list-allocator/deallocate.svg)

In order to use these adjacent holes for a large allocation, we need to merge them to a single large hole first:

![merge adjacent holes and allocate large block](/images/linked-list-allocator/merge-holes-and-allocate.svg)

The easiest way to ensure that adjacent holes are always merged, is to keep the hole list sorted by address. Thus, we only need to check the predecessor and the successor in the list when we free a memory block. If they are adjacent to the freed block, we merge the corresponding holes. Else, we insert the freed block as a new hole at the correct position.

### Implementation
The detailed implementation would go beyond the scope of this post, since it contains several hidden difficulties. For example:

- Several merge cases: Merge with the previous hole, merge with the next hole, merge with both holes.
- We need to satisfy the alignment requirements, which requires additional splitting logic.
- The minimal hole size of 16 bytes: We must not create smaller holes when splitting a hole.

I created the [linked_list_allocator] crate to handle all of these cases. It consists of a [Heap struct] that provides an `allocate_first_fit` and a `deallocate` method. If you are interested in the implementation details, check out the [source code][linked_list_allocator source].

[linked_list_allocator]: https://crates.io/crates/linked_list_allocator
[Heap struct]: http://phil-opp.github.io/linked-list-allocator/linked_list_allocator/struct.Heap.html
[linked_list_allocator source]: https://github.com/phil-opp/linked-list-allocator

So we just need to implement Rust's allocation modules and integrate it into our kernel. We start by creating a new `hole_list_allocator`[^1] crate inside the `libs` directory:

[^1]: The name `linked_list_allocator` is already taken, sorry :P.

```shell
> cd libs
> cargo new hole_list_allocator
> cd hole_list_allocator
```

We add the `allocator` and `no_std` attributes to `src/lib.rs` like described above:

```rust
// in libs/hole_list_allocator/src/lib.rs

#![feature(allocator)]

#![allocator]
#![no_std]
```

We also add a static allocator protected by a spinlock, but this time we use the `Heap` type of the `linked_list_allocator` crate:

```rust
// in libs/hole_list_allocator/src/lib.rs

#![feature(const_fn)]

use spin::Mutex;
use linked_list_allocator::Heap;

extern crate spin;
extern crate linked_list_allocator;

pub const HEAP_START: usize = 0o_000_001_000_000_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

static HEAP: Mutex<Heap> = Mutex::new(Heap::new(HEAP_START, HEAP_SIZE));
```
Note that we use the same values for `HEAP_START` and `HEAP_SIZE` as in the `bump_allocator`.

We need to add the extern crates to our `Cargo.toml`:

``` shell
> cargo add spin
> cargo add linked_list_allocator
```

However, we get an error when we try to compile it:

```
error: function calls in statics are limited to constant functions,
   struct and enum constructors [E0015]
static HEAP: Mutex<Heap> = Mutex::new(Heap::new(HEAP_START, HEAP_SIZE));
                                      ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
```
The reason is that the `Heap::new` function needs to initialize the first hole (like described [above](#initialization)). This can't be done at compile time, so the function can't be a `const` function. Therefore we can't use it to initialize a static.

There is an easy solution for crates with access to the standard library: [lazy_static]. It automatically initializes the static when it's used the first time. By default, it relies on the `std::sync::once` module and is thus unusable in our kernel. Fortunately it has a `spin_no_std` feature for `no_std` projects.

[lazy_static]: https://github.com/rust-lang-nursery/lazy-static.rs

So let's use the `lazy_static!` macro to fix our `hole_list_allocator`:

```toml
# in libs/hole_list_allocator/Cargo.toml

[dependencies.lazy_static]
version = "0.2.1"
features = ["spin_no_std"]
```

```rust
// in libs/hole_list_allocator/src/lib.rs

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref HEAP: Mutex<Heap> = Mutex::new(unsafe {
        Heap::new(HEAP_START, HEAP_SIZE)
    });
}
```
The `unsafe` block is required since `Heap::new` is `unsafe`. It's unsafe because it assumes that `HEAP_START` is a valid and unused address.

Now we can implement the allocation functions:

```rust
// in libs/hole_list_allocator/src/lib.rs

#[no_mangle]
pub extern fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    HEAP.lock().allocate_first_fit(size, align).expect("out of memory")
}

#[no_mangle]
pub extern fn __rust_deallocate(ptr: *mut u8, size: usize, align: usize) {
    unsafe { HEAP.lock().deallocate(ptr, size, align) };
}
```

The remaining functions are implemented like above:

```rust
// in libs/hole_list_allocator/src/lib.rs

#[no_mangle]
pub extern fn __rust_usable_size(size: usize, _align: usize) -> usize {
    size
}

#[no_mangle]
pub extern fn __rust_reallocate_inplace(_ptr: *mut u8, size: usize,
    _new_size: usize, _align: usize) -> usize
{
    size
}

#[no_mangle]
pub extern fn __rust_reallocate(ptr: *mut u8, size: usize, new_size: usize,
                                align: usize) -> *mut u8 {
    use core::{ptr, cmp};

    // from: https://github.com/rust-lang/rust/blob/
    //     c66d2380a810c9a2b3dbb4f93a830b101ee49cc2/
    //     src/liballoc_system/lib.rs#L98-L101

    let new_ptr = __rust_allocate(new_size, align);
    unsafe { ptr::copy(ptr, new_ptr, cmp::min(size, new_size)) };
    __rust_deallocate(ptr, size, align);
    new_ptr
}
```

Now we just need to replace every use of `bump_allocator` with `hole_list_allocator` in our kernel:

```toml
# in Cargo.toml

[dependencies.hole_list_allocator]
path = "libs/hole_list_allocator"
```

```diff
in src/lib.rs:

-extern crate bump_allocator;
+extern crate hole_list_allocator;

in memory::init in src/memory/mod.rs:

-use bump_allocator::{HEAP_START, HEAP_SIZE};
+use hole_list_allocator::{HEAP_START, HEAP_SIZE};
```

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

[next post]: {{% relref "09-catching-exceptions.md" %}}
