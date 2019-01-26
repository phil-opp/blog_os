+++
title = "Allocating Frames"
weight = 5
path = "allocating-frames"
date  = 2015-11-15
template = "first-edition/page.html"
+++

In this post we create an allocator that provides free physical frames for a future paging module. To get the required information about available and used memory we use the Multiboot information structure. Additionally, we improve the `panic` handler to print the corresponding message and source line.

<!-- more -->

The full source code is available on [Github][source repo]. Feel free to open issues there if you have any problems or improvements. You can also leave a comment at the bottom.

[source repo]: https://github.com/phil-opp/blog_os/tree/first_edition_post_5

## Preparation
We still have a really tiny stack of 64 bytes, which won't suffice for this post. So we increase it to 16kB (four pages) in `boot.asm`:

```asm
section .bss
...
stack_bottom:
    resb 4096 * 4
stack_top:
```

## The Multiboot Information Structure
When a Multiboot compliant bootloader loads a kernel, it passes a pointer to a boot information structure in the `ebx` register. We can use it to get information about available memory and loaded kernel sections.

First, we need to pass this pointer to our kernel as an argument to `rust_main`. To find out how arguments are passed to functions, we can look at the [calling convention of Linux]:

[calling convention of Linux]: https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI

> The first six integer or pointer arguments are passed in registers RDI, RSI, RDX, RCX, R8, and R9

So to pass the pointer to our kernel, we need to move it to `rdi` before calling the kernel. Since we're not using the `rdi`/`edi` register in our bootstrap code, we can simply set the `edi` register right after booting (in `boot.asm`):

```nasm
start:
    mov esp, stack_top
    mov edi, ebx       ; Move Multiboot info pointer to edi
```
Now we can add the argument to our `rust_main`:

```rust
pub extern fn rust_main(multiboot_information_address: usize) { ... }
```

Instead of writing an own Multiboot module, we use the [multiboot2] crate. It gives us some basic information about mapped kernel sections and available memory. I just wrote it for this blog post since I could not find any other Multiboot 2 crate. It's still incomplete, but it does its job.

[multiboot2]: https://docs.rs/multiboot2

So let's add a dependency on the git repository:

```toml
# in Cargo.toml
[dependencies]
...
multiboot2 = "0.1.0"
```

```rust
// in src/lib.rs
extern crate multiboot2;
```

Now we can use it to print available memory areas.

### Available Memory
The boot information structure consists of various _tags_. See section 3.4 of the Multiboot specification ([PDF][multiboot specification]) for a complete list. The _memory map_ tag contains a list of all available RAM areas. Special areas such as the VGA text buffer at `0xb8000` are not available. Note that some of the available memory is already used by our kernel and by the multiboot information structure itself.

[multiboot specification]: http://nongnu.askapache.com/grub/phcoder/multiboot.pdf

To print all available memory areas, we can use the `multiboot2` crate in our `rust_main` as follows:

```rust
let boot_info = unsafe{ multiboot2::load(multiboot_information_address) };
let memory_map_tag = boot_info.memory_map_tag()
    .expect("Memory map tag required");

println!("memory areas:");
for area in memory_map_tag.memory_areas() {
    println!("    start: 0x{:x}, length: 0x{:x}",
        area.base_addr, area.length);
}
```
The `load` function is `unsafe` because it relies on a valid address. Since the memory tag is not required by the Multiboot specification, the `memory_map_tag()` function returns an `Option`. The `memory_areas()` function returns the desired memory area iterator.

The output looks like this:

```
Hello World!
memory areas:
    start: 0x0, length: 0x9fc00
    start: 0x100000, length: 0x7ee0000
```
So we have one area from `0x0` to `0x9fc00`, which is a bit below the 1MiB mark. The second, bigger area starts at 1MiB and contains the rest of available memory. The area from `0x9fc00` to 1MiB is not available since it contains for example the VGA text buffer at `0xb8000`. This is the reason for putting our kernel at 1MiB and not somewhere below.

If you give QEMU more than 4GiB of memory by passing `-m 5G`, you get another unusable area below the 4GiB mark. This memory is normally mapped to some hardware devices. See the [OSDev Wiki][Memory_map] for more information.

[Memory_map]: http://wiki.osdev.org/Memory_Map_(x86)

### Handling Panics
We used `expect` in the code above, which will panic if there is no memory map tag. But our current panic handler just loops without printing any error message. Of course we could replace `expect` by a `match`, but we should fix the panic handler nonetheless:

```rust
#[lang = "panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt() -> ! {
    println!("PANIC");
    loop{}
}
```
Now we get a `PANIC` message. But we can do even better. The `panic_fmt` function has actually some arguments:

```rust
#[lang = "panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(fmt: core::fmt::Arguments, file: &'static str,
    line: u32) -> !
{
    println!("\n\nPANIC in {} at line {}:", file, line);
    println!("    {}", fmt);
    loop{}
}
```
Be careful with these arguments as the compiler does not check the function signature for `lang_items`.

Now we get the panic message and the causing source line. You can try it by inserting a `panic` somewhere.

### Kernel ELF Sections
To read and print the sections of our kernel ELF file, we can use the _Elf-sections_ tag:

```rust
let elf_sections_tag = boot_info.elf_sections_tag()
    .expect("Elf-sections tag required");

println!("kernel sections:");
for section in elf_sections_tag.sections() {
    println!("    addr: 0x{:x}, size: 0x{:x}, flags: 0x{:x}",
        section.addr, section.size, section.flags);
}
```
This should print out the start address and size of all kernel sections. If the section is writable, the `0x1` bit is set in `flags`. The `0x4` bit marks an executable section and the `0x2` bit indicates that the section was loaded in memory. For example, the `.text` section is executable but not writable and the `.data` section just the opposite.

But when we execute it, tons of really small sections are printed. We can use the `objdump -h build/kernel-x86_64.bin` command to list the sections with name. There seem to be over 200 sections and many of them start with `.text.*` or `.data.rel.ro.local.*`. This is because the Rust compiler puts e.g. each function in its own `.text` subsection. That way, unused functions are removed when the linker omits unused sections.

To merge these subsections, we need to update our linker script:

```
ENTRY(start)

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

These lines are taken from the default linker script of `ld`, which can be obtained through `ld ‑verbose`. The `.text` _output_ section contains now all `.text.*` _input_ sections of the static library (and the same applies for the `.rodata` and `.data.rel.ro` sections).

Now there are only 12 sections left and we get a much more useful output:

![qemu output](qemu-memory-areas-and-kernel-sections.png)

If you like, you can compare this output to the `objdump -h build/kernel-x86_64.bin` output. You will see that the start addresses and sizes match exactly for each section. The sections with flags `0x0` are mostly debug sections, so they don't need to be loaded. And the last few sections of the QEMU output aren't in the `objdump` output because they are special sections such as string tables.

### Start and End of Kernel
We can now use the ELF section tag to calculate the start and end address of our loaded kernel:

```rust
let kernel_start = elf_sections_tag.sections().map(|s| s.addr)
    .min().unwrap();
let kernel_end = elf_sections_tag.sections().map(|s| s.addr + s.size)
    .max().unwrap();
```
The other used memory area is the Multiboot Information structure:

```rust
let multiboot_start = multiboot_information_address;
let multiboot_end = multiboot_start + (boot_info.total_size as usize);
```
Printing these numbers gives us:

```
kernel_start: 0x100000, kernel_end: 0x11a168
multiboot_start: 0x11d400, multiboot_end: 0x11d9c8
```
So the kernel starts at 1MiB (like expected) and is about 105 KiB in size. The multiboot information structure was placed at `0x11d400` by GRUB and needs 1480 bytes. Of course your numbers could be a bit different due to different versions of Rust or GRUB (or some differences in the source code).

## A frame allocator
When using paging, the physical memory is split into equally sized chunks (normally 4096 bytes) Such a chunk is called "physical page" or "frame". These frames can be mapped to any virtual page through page tables. For more information about paging take a peek at the [next post].

We will need a free frame in many cases. For example when want to increase the size of our future kernel heap. Or when we create a new page table. Or when we add a new kernel thread and thus need to allocate a new stack. So we need some kind of allocator that keeps track of physical frames and gives us a free one when needed.

There are various ways to write such a frame allocator:

We could create some kind of linked list from the free frames. For example, each frame could begin with a pointer to the next free frame. Since the frames are free, this would not overwrite any data. Our allocator would just save the head of the list and could easily allocate and deallocate frames by updating pointers. Unfortunately, this approach has a problem: It requires reading and writing these free frames. So we would need to map all physical frames to some virtual address, at least temporary. Another disadvantage is that we need to create this linked list at startup. That implies that we need to set over one million pointers at startup if the machine has 4GiB of RAM.

Another approach is to create some kind of data structure such as a [bitmap or a stack] to manage free frames. We could place it in the already identity mapped area right behind the kernel or multiboot structure. That way we would not need to (temporary) map each free frame. But it has the same problem of the slow initial creating/filling. In fact, we will use this approach in a future post to manage frames that are freed again. But for the initial management of free frames, we use a different method.

[bitmap or a stack]: http://wiki.osdev.org/Page_Frame_Allocation#Physical_Memory_Allocators

In the following, we will use Multiboot's memory map directly. The idea is to maintain a simple counter that starts at frame 0 and is increased constantly. If the current frame is available (part of an available area in the memory map) and not used by the kernel or the multiboot structure (we know their start and end addresses), we know that it's free and return it. Else, we increase the counter to the next possibly free frame. That way, we don't need to create a data structure when booting and the physical frames can remain unmapped. The only problem is that we cannot reasonably free frames again, but we will solve that problem in a future post (by adding an intermediate frame stack that saves freed frames).

<!--- TODO link future post -->

So let's start implementing our memory map based frame allocator.

### A Memory Module
First we create a memory module with a `Frame` type (`src/memory/mod.rs`):

```rust
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    number: usize,
}
```
(Don't forget to add the `mod memory` line to `src/lib.rs`.) Instead of e.g. the start address, we just store the frame number. We use `usize` here since the number of frames depends on the memory size. The long `derive` line makes frames printable and comparable.

To make it easy to get the corresponding frame for a physical address, we add a `containing_address` method:

```rust
pub const PAGE_SIZE: usize = 4096;

impl Frame {
    fn containing_address(address: usize) -> Frame {
        Frame{ number: address / PAGE_SIZE }
    }
}
```

We also add a `FrameAllocator` trait:

```rust
pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self, frame: Frame);
}
```
This allows us to create another, more advanced frame allocator in the future.

### The Allocator
Now we can put everything together and create the actual frame allocator. Therefor we create a `src/memory/area_frame_allocator.rs` submodule. The allocator struct looks like this:

```rust
use memory::{Frame, FrameAllocator};
use multiboot2::{MemoryAreaIter, MemoryArea};

pub struct AreaFrameAllocator {
    next_free_frame: Frame,
    current_area: Option<&'static MemoryArea>,
    areas: MemoryAreaIter,
    kernel_start: Frame,
    kernel_end: Frame,
    multiboot_start: Frame,
    multiboot_end: Frame,
}
```
The `next_free_frame` field is a simple counter that is increased every time we return a frame. It's initialized to `0` and every frame below it counts as used. The `current_area` field holds the memory area that contains `next_free_frame`. If `next_free_frame` leaves this area, we will look for the next one in `areas`. When there are no areas left, all frames are used and `current_area` becomes `None`. The `{kernel, multiboot}_{start, end}` fields are used to avoid returning already used fields.

To implement the `FrameAllocator` trait, we need to implement the allocation and deallocation methods:

```rust
impl FrameAllocator for AreaFrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame> {
        // TODO (see below)
    }

    fn deallocate_frame(&mut self, frame: Frame) {
        // TODO (see below)
    }
}
```
The `allocate_frame` method looks like this:

```rust
// in `allocate_frame` in `impl FrameAllocator for AreaFrameAllocator`

if let Some(area) = self.current_area {
    // "Clone" the frame to return it if it's free. Frame doesn't
    // implement Clone, but we can construct an identical frame.
    let frame = Frame{ number: self.next_free_frame.number };

    // the last frame of the current area
    let current_area_last_frame = {
        let address = area.base_addr + area.length - 1;
        Frame::containing_address(address as usize)
    };

    if frame > current_area_last_frame {
        // all frames of current area are used, switch to next area
        self.choose_next_area();
    } else if frame >= self.kernel_start && frame <= self.kernel_end {
        // `frame` is used by the kernel
        self.next_free_frame = Frame {
            number: self.kernel_end.number + 1
        };
    } else if frame >= self.multiboot_start && frame <= self.multiboot_end {
        // `frame` is used by the multiboot information structure
        self.next_free_frame = Frame {
            number: self.multiboot_end.number + 1
        };
    } else {
        // frame is unused, increment `next_free_frame` and return it
        self.next_free_frame.number += 1;
        return Some(frame);
    }
    // `frame` was not valid, try it again with the updated `next_free_frame`
    self.allocate_frame()
} else {
    None // no free frames left
}
```
The `choose_next_area` method isn't part of the trait and thus goes into a new `impl AreaFrameAllocator` block:

```rust
// in `impl AreaFrameAllocator`

fn choose_next_area(&mut self) {
    self.current_area = self.areas.clone().filter(|area| {
        let address = area.base_addr + area.length - 1;
        Frame::containing_address(address as usize) >= self.next_free_frame
    }).min_by_key(|area| area.base_addr);

    if let Some(area) = self.current_area {
        let start_frame = Frame::containing_address(area.base_addr as usize);
        if self.next_free_frame < start_frame {
            self.next_free_frame = start_frame;
        }
    }
}
```
This function chooses the area with the minimal base address that still has free frames, i.e. `next_free_frame` is smaller than its last frame. Note that we need to clone the iterator because the [min_by_key] function consumes it. If there are no areas with free frames left, `min_by_key` automatically returns the desired `None`.

[min_by_key]: https://doc.rust-lang.org/nightly/core/iter/trait.Iterator.html#method.min_by_key

If the `next_free_frame` is below the new `current_area`, it needs to be updated to the area's start frame. Else, the `allocate_frame` call could return an unavailable frame.

We don't have a data structure to store free frames, so we can't implement `deallocate_frame` reasonably. Thus we use the `unimplemented` macro, which just panics when the method is called:

```rust
impl FrameAllocator for AreaFrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame> {
        // described above
    }

    fn deallocate_frame(&mut self, _frame: Frame) {
        unimplemented!()
    }
}
```

Now we only need a constructor function to make the allocator usable:

```rust
pub fn new(kernel_start: usize, kernel_end: usize,
      multiboot_start: usize, multiboot_end: usize,
      memory_areas: MemoryAreaIter) -> AreaFrameAllocator
{
    let mut allocator = AreaFrameAllocator {
        next_free_frame: Frame::containing_address(0),
        current_area: None,
        areas: memory_areas,
        kernel_start: Frame::containing_address(kernel_start),
        kernel_end: Frame::containing_address(kernel_end),
        multiboot_start: Frame::containing_address(multiboot_start),
        multiboot_end: Frame::containing_address(multiboot_end),
    };
    allocator.choose_next_area();
    allocator
}
```
Note that we call `choose_next_area` manually here because `allocate_frame` returns `None` as soon as `current_area` is `None`. So by calling `choose_next_area` we initialize it to the area with the minimal base address.

### Testing it
In order to test it in main, we need to [re-export] the `AreaFrameAllocator` in the `memory` module. Then we can create a new allocator:

[re-export]: https://doc.rust-lang.org/book/crates-and-modules.html#re-exporting-with-pub-use

```rust
let mut frame_allocator = memory::AreaFrameAllocator::new(
    kernel_start as usize, kernel_end as usize, multiboot_start,
    multiboot_end, memory_map_tag.memory_areas());
```

Now we can test it by adding some frame allocations:

```rust
println!("{:?}", frame_allocator.allocate_frame());
```
You will see that the frame number starts at `0` and increases steadily, but the kernel and Multiboot frames are left out (you need to allocate many frames to see this since the kernel starts at frame 256).

The following `for` loop allocates all frames and prints out the total number of allocated frames:

```rust
for i in 0.. {
    if let None = frame_allocator.allocate_frame() {
        println!("allocated {} frames", i);
        break;
    }
}
```
You can try different amounts of memory by passing e.g. `-m 500M` to QEMU. To compare these numbers, [WolframAlpha] can be very helpful.

[WolframAlpha]: http://www.wolframalpha.com/input/?i=%2832605+*+4096%29+bytes+in+MiB

## Conclusion

Now we have a working frame allocator. It is a bit rudimentary and cannot free frames, but it also is very fast since it reuses the Multiboot memory map and does not need any costly initialization. A future post will build upon this allocator and add a stack-like data structure for freed frames.

## What's next?
The [next post] will be about paging again. We will use the frame allocator to create a safe module that allows us to switch page tables and map pages. Then we will use this module and the information from the Elf-sections tag to remap the kernel correctly.

[next post]: ./first-edition/posts/06-page-tables/index.md

## Recommended Posts
Eric Kidd started the [Bare Metal Rust] series last week. Like this post, it builds upon the code from [Printing to Screen], but tries to support keyboard input instead of wrestling through memory management details.

[Bare Metal Rust]: http://www.randomhacks.net/bare-metal-rust/
[Printing to Screen]: ./first-edition/posts/04-printing-to-screen/index.md
