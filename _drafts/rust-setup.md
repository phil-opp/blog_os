---
layout: post
title: 'Setup Rust in small steps'
category: 'rust-os'
---
In the previous posts we created a [minimal Multiboot kernel][multiboot post] and [switched to Long Mode][long mode post]. Now we can finally switch to sweet Rust code. [Rust] is a beautiful high-level language that has no runtime. It allows us to not link the standard library and write bare metal code. Unfortunately the setup is not quite hassle-free yet.

This blog post tries to setup Rust step-by-step and point out the different problems. If you have any questions, problems, or suggestions please [file an issue] or create a comment at the bottom. The code from this post is in a [Github repository], too.

[multiboot post]: {{ site.url }}{{ page.previous.previous.url }}
[long mode post]: {{ site.url }}{{ page.previous.url }}
[Rust]: https://www.rust-lang.org/
[file an issue]: https://github.com/phil-opp/phil-opp.github.io/issues
[Github repository]: https://github.com/phil-opp/blogOS/tree/rust_setup

## Rust Setup
We need a nightly compiler, as we need to use many unstable features. To manage Rust installations I highly recommend [multirust] by brson. It allows you to install nightly, beta, and stable compilers side-by-side and makes it easy to update them. After installing you can just run `multirust update nightly` to install or update to the latest Rust nightly.

[multirust]: https://github.com/brson/multirust

## Creating a Rust project
Normally you would call `cargo new` when you want to create a new project folder. We can't use it because our folder already exists so we will do it manually. Fortunately we just need to add a cargo configuration file named `Cargo.toml`:

```toml
[package]
name = "blog_os"
version = "0.1.0"
authors = ["Philipp Oppermann <dev@phil-opp.com>"]

[lib]
crate-type = ["staticlib"]
```
The `package` section contains basic project metadata and is identical to the `Cargo.toml` created by `cargo new blog_os`. The `lib` section specifies that we want to build a static library, i.e. a library that contains all of its dependencies.

Now we need to place our root source file in `src/lib.rs`:

```rust
#![feature(no_std, lang_items)]
#![no_std]

#[no_mangle]
pub extern fn main() {}

#[lang = "eh_personality"] extern fn eh_personality() {}
#[lang = "panic_fmt"] extern fn panic_fmt() -> ! {loop{}}
```
Let's break it down:

- `#!` defines an [attribute] of the current module. Since we are at the root module, they apply to the crate itself.
- The `features` attribute is used to allow the specified _feature-gated_ attributes in this crate. You can't do that in a stable/beta compiler, so this is one reason we need a Rust nighly.
- The `no_std` attribute prevents the automatic linking of the standard library. We can't use `std` because it relies on operating system features like files, system calls, and various device drivers. Remember that currently the only “feature” of our OS is printing `OKAY` :).
- A `#` without a `!` afterwards defines an attribute for the _following_ item (a function in our case).
- The `no_mangle` attribute disables the automatic [name mangling] that Rust uses to get unique function names. We want to do a `call main` from our assembly code, so this function name must stay as it is.
- We mark our main function as `extern` to make it compatible to the standard C [calling convention].
- The `lang` attribute defines a Rust [language item].
- The `eh_personality` function is used for Rust's [unwinding] on `panic!`. We can leave it empty since we don't have any unwinding support in our OS yet.
- The `panic_fmt` function is the entry point on panic. Right now we can't do anything useful, so we just make sure that it doesn't return (required by the `!` return type).

[attribute]: https://doc.rust-lang.org/book/attributes.html
[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[calling convention]: https://en.wikipedia.org/wiki/Calling_convention
[language item]: https://doc.rust-lang.org/book/lang-items.html
[unwinding]: https://doc.rust-lang.org/std/rt/unwind/

## Building Rust
We can now build it using `cargo build`. It creates a static library at `target/debug/libblog_os.a` that we can link with our assembly kernel. Let's extend our `Makefile` to do that. We add a new `.PHONY` target `cargo` and modify the `$(kernel)` target to link the created static lib ([full file][github makefile]):

```make
# ...
rust_os := target/debug/libblog_os.a
# ...
$(kernel): cargo $(rust_os) $(assembly_object_files) $(linker_script)
       @ld -n -T $(linker_script) -o $(kernel) $(assembly_object_files) $(rust_os)

cargo:
       @cargo build
```
Now `cargo build` is executed on every `make`, even if no source file was changed. And the ISO is recreated on every `make iso`/`make run`, too. We could try to avoid this by adding dependencies on all rust source and cargo configuration files to the `cargo` target, but the ISO creation takes only half a second on my machine and most of the time we will have changed a Rust file when we run it. So we keep it simple for now and let cargo do the bookkeeping of changed files (it does it anyway).

[github makefile]: #TODO

## Calling Rust
Now we can call the main method in `long_mode_start`:

```nasm
bits 64
long_mode_start:
    ; call the rust main
    extern main     ; new
    call main       ; new

    ; print `OKAY` to screen
    mov rax, 0x2f592f412f4b2f4f
    mov qword [0xb8000], rax
    hlt
```
By defining `main` as `extern` we tell nasm that the function is defined in another file. The linker takes care of linking them together (_suprise_). So if we have a typo in the name or forget to mark the rust function as `pub extern`, we'll get a linker error.

When we've done everything right, we still see the green `OKAY` when executing `make run`. That means that we successfully called the Rust function and returned back to assembly.

## Testing
Let's play around with some Rust code:

```rust
pub extern fn main() {
    let x = ["Hello", " ", "World", "!"];
}
```
When we test it using `make run`, it fails with `undefined reference to 'memcpy'`. This function is one of the basic functions of the C library (`libc`). Usually the `libc` crate is linked to every Rust program with the standard library but we opted out through `#![no_std]`. So we could try to fix this by adding the [libc crate] as `extern crate`. But `libc` is just a wrapper for the system `libc`, for example `glibc` on Linux, so this won't work for us. Instead we need to recreate the basic `libc` functions like `memcpy`, `memmove`, `memset`, and `memcmp` in Rust.

[libc crate]: https://doc.rust-lang.org/nightly/libc/index.html

### rlibc
Fortunately there already is a crate that does just that: [rlibc]. When we look at its [source code][rlibc source] we see that it contains no magic, just some [raw pointer] operations in a while loop. So let's add `rlibc` to our crate. We need to add a [crates.io] dependency in our `Cargo.toml`:

```toml
...
[dependencies]
rlibc = "*"
```
and an `extern crate` in our `src/lib.rs`:

```rust
...
extern crate rlibc;

#[no_mangle]
pub extern fn main() {
...
```
Now `make run` doesn't complain about `memcpy` anymore. Instead it will show a pile of new errors:

```
target/debug/libblog_os.a(core-35017696.0.o): In function `ops::f32.Rem::rem::hfcbbcbe5711a6e6emxm':
core.0.rs:(.text._ZN3ops7f32.Rem3rem20hfcbbcbe5711a6e6emxmE+0x1): undefined reference to `fmodf'
target/debug/libblog_os.a(core-35017696.0.o): In function `ops::f64.Rem::rem::hbf225030671c7a35Txm':
core.0.rs:(.text._ZN3ops7f64.Rem3rem20hbf225030671c7a35TxmE+0x1): undefined reference to `fmod'
...
```

[rlibc]: https://crates.io/crates/rlibc
[rlibc source]: https://github.com/rust-lang/rlibc/blob/master/src/lib.rs
[raw pointer]: https://doc.rust-lang.org/book/raw-pointers.html
[crates.io]: https://crates.io

### --gc-sections
The new errors are linker errors about missing `fmod` and `fmodf` functions. These functions are used for the modulo operation (`%`) on floating point numbers in [libcore]. The core library is added implicitly when using `#![no_std]` and provides basic standard library features like `Option` or `Iterator`. According to the documentation it is “dependency-free” but it actually has some dependencies, for example on `fmod` and `fmodf`.

[libcore]: https://doc.rust-lang.org/core/

So how do we fix this problem? We don't use any floating point operations, so we could just provide our own implementations of `fmod` and `fmodf` that just do a `loop{}`. But there's a better way that doesn't fail silently when we use floats some day: We tell the linker to remove unused sections. That's generally a good idea as it reduces kernel size. And we don't have any references to `fmod` and `fmodf` anymore until we use floating point modulo. The magic linker flag is `--gc-sections` which stands for “garbage collect sections”. Let's add it to the `$(kernel)` target in our `Makefile`:

```make
$(kernel): cargo $(rust_os) $(assembly_object_files) $(linker_script)
	@ld -n --gc-sections -T $(linker_script) -o $(kernel) $(assembly_object_files) $(rust_os)
```
Now we can do a `make run` again and… it doesn't boot anymore:

```
GRUB error: no multiboot header found.
```
What happened? Well, the linker removed unused sections. And since we don't use the Multiboot section anywhere `ld` removes it, too. So we need to tell the linker that it should keep this section. We can do through the `KEEP` command in our `linker.ld`:

```
.boot :
{
    /* ensure that the multiboot header is at the beginning */
    KEEP(*(.multiboot))
}
```
Now everything should work (the green `OKAY`) again. But there is another linking issue:

### no-landing-pads

The following snippet still fails:

```rust
    ...
    let test = (0..3).flat_map(|x| 0..x).zip(0..);
```
The error is a linker error again (hence the ugly error message):

```
target/debug/libblog_os.a(blog_os.0.o): In function `blog_os::iter::Iterator::zip<core::iter::FlatMap<core::ops::Range<i32>, core::ops::Range<i32>, closure>,core::ops::RangeFrom<i32>>':
/home/.../src/libcore/iter.rs:223: undefined reference to `_Unwind_Resume'
```
So the linker can't find a function named `_Unwind_Resume` that is referenced in `iter.rs:223` in libcore. This reference is inserted by the compiler, it's not really in the libcore [source][iter.rs:223]. The inserted code is a so-called _landing pad_ that is used for exception handling. The easiest way of fixing this problem is to disable the landing pad creation since we don't supports panics anyway right now. We can do this by passing a `-Z no-landing-pads` flag to rustc. To do this we replace the `cargo build` command in our Makefile:

```make
cargo:
    @cargo rustc -- -Z no-landing-pads
```
Now we fixed all linking issues.

## The final problem
Unfortunately there is one last problem left that gets triggered by the following code:

```rust
let mut a = 42;
a += 1;
```
When we add that code to `main` and test it using `make run`, the OS will constantly reboot itself. Let's try to debug it.

[iter.rs:223]: https://doc.rust-lang.org/nightly/src/core/iter.rs.html#223

### Debugging
Such a boot loop is most likely caused by some [CPU exception][exception table]. When these exceptions aren't handled, a [Triple Fault] occurs and the processor resets itself. We can look at generated CPU interrupts/exceptions using QEMU:

```
> qemu-system-x86_64 -d int -no-reboot -hda build/os-x86_64.iso
SMM: enter
...
SMM: after RSM
...
check_exception old: 0xffffffff new 0x6
     0: v=06 e=0000 i=0 cpl=0 IP=0008:0000000000100200 pc=0000000000100200
     SP=0010:0000000000102fd0 env->regs[R_EAX]=0000000080010010
...
check_exception old: 0xffffffff new 0xd
     1: v=0d e=0062 i=0 cpl=0 IP=0008:0000000000100200 pc=0000000000100200
     SP=0010:0000000000102fd0 env->regs[R_EAX]=0000000080010010
...
check_exception old: 0xd new 0xd
     2: v=08 e=0000 i=0 cpl=0 IP=0008:0000000000100200 pc=0000000000100200
     SP=0010:0000000000102fd0 env->regs[R_EAX]=0000000080010010
...
check_exception old: 0x8 new 0xd
```
Let me first explain the QEMU arguments: The `-d int` logs CPU interrupts to the console and the `-no-reboot` flag closes QEMU instead of constant rebooting. But what does the cryptical output mean? I already removed most of it as we don't need it here. Let's break down the rest:

- The first two blocks, `SMM: enter` and `SMM: after RSM` are created before our OS boots, so we just ignore them.
- The next block, `check_exception old: 0xffffffff new 0x6` is the interesting one. It says: “a new CPU exception with number `0xe` occurred“.
- The last blocks indicate further exceptions. They were thrown because we didn't handle the `0x6` exception, so we're going to ignore them, too.

So let's look at the first exception: `old:0xffffffff` means that the CPU wasn't handling an interrupt when the exception occurred. The register dump tells us that the current instruction was `0x100200` (in `IP`  (instruction pointer) or `pc` (program counter)). By looking at an [exception table] we learn that the number `0x6` indicates a [Invalid Opcode] fault. So the instruction at `0x100200` seems to be invalid. Let's look at it using `objdump`:

```
> objdump -D build/kernel-x86_64.bin | grep "100200:"
100200:	0f 28 05 49 01 00 00 	movaps 0x149(%rip),%xmm0 ...
```
Through `objdump -D` we disassemble our whole kernel and `grep` picks the relevant line. The instruction at `100200` seems to be a valid [movaps] instruction. It's a [SSE] instruction that moves 128 bit between memory and SSE-registers (e.g. `xmm0`). But why the `Invalid Opcode` exception? The answer is hidden behind the [movaps] link: The section _Protected Mode Exceptions_ lists the conditions for the various exceptions. The short code of the `Invalid Opcode` is `#UD`, so the exceptions occurs:
> For an unmasked Streaming SIMD Extensions 2 instructions numeric exception (CR4.OSXMMEXCPT =0). If EM in CR0 is set. If OSFXSR in CR4 is 0. If CPUID feature flag SSE2 is 0.

The rough translation of this cryptic low-level code is: _If SSE isn't enabled_. So apparently Rust uses SSE instructions by default and we didn't enable SSE before. Let's fix this:

[Physical Address Extension]: https://en.wikipedia.org/wiki/Physical_Address_Extension
[exception table]: http://wiki.osdev.org/Exceptions
[Triple Fault]: #TODO
[Invalid Opcode]: http://wiki.osdev.org/Exceptions#Invalid_Opcode
[movaps]: http://www.c3se.chalmers.se/Common/VTUNE-9.1/doc/users_guide/mergedProjects/analyzer_ec/mergedProjects/reference_olh/mergedProjects/instructions/instruct32_hh/vc181.htm
[SSE]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions

### Enabling SSE
To enable SSE we need to return to assembly. We need to add a function that checks if SSE is available and enables it then. Else we want to print an error message. But we can't use our existing `error` function because it uses (now invalid) 32-bit instructions. So we need a new one (in `long_mode_init.asm`):

```nasm
; Prints `ERROR: ` and the given error code to screen and hangs.
; parameter: error code (in ascii) in al
error:
    mov rbx, 0x4f4f4f524f524f45
    mov [0xb8000], rbx
    mov rbx, 0x4f204f204f3a4f52
    mov [0xb8008], rbx
    mov byte [0xb800e], al
    hlt
    jmp error
```
It's the nearly the same as the 32-bit code in the [last post][32-bit error function] (instead of `ERR:` we print `ERROR:` here). Now we can add a function that checks for SSE and enables it:

```nasm
; Check for SSE and enable it. If it's not supported throw error "a".
setup_SSE:
    ; check for SSE
    mov rax, 0x1
    cpuid
    test edx, 1<<25
    jz .no_SSE

    ; enable SSE
    mov rax, cr0
    and ax, 0xFFFB      ; clear coprocessor emulation CR0.EM
    or ax, 0x2          ; set coprocessor monitoring  CR0.MP
    mov cr0, rax
    mov rax, cr4
    or ax, 3 << 9       ; set CR4.OSFXSR and CR4.OSXMMEXCPT at the same time
    mov cr4, rax

    ret
.no_SSE:
    mov al, "a"
    jmp error
```
Notice that we set/unset exactly the bits that can cause the `Invalid Opcode` exception. We insert a `call setup_SSE` right before calling `main` and our Rust code will finally work.

[32-bit error function]: #TODO

### “OS returned!”
Now that we're editing assembly anyway, we should change the `OKAY` message to something more meaningful. My suggestion is a red `OS returned!`:

```nasm
...
call main

.os_returned:
    ; rust main returned, print `OS returned!`
    mov rax, 0x4f724f204f534f4f
    mov [0xb8000], rax
    mov rax, 0x4f724f754f744f65
    mov [0xb8008], rax
    mov rax, 0x4f214f644f654f6e
    mov [0xb8010], rax
    hlt
```
Ok that's enough assembly for now, let's switch back to Rust and print the traditional `Hello World!`.

## Hello World!
Of course we could just write some magic bytes to memory like we did in assembly. But I chose some code that highlight some of Rust's features:

```rust
#![feature(no_std, lang_items)]
#![feature(core_slice_ext, core_str_ext, core_intrinsics)]
#![no_std]

extern crate rlibc;

use core::intrinsics::offset;

#[no_mangle]
pub extern fn main() {
    // ATTENTION: we have a very small stack and no guard page
    let x = ["Hello", " ", "World", "!"];
    let screen_pointer = 0xb8000 as *const u16;

    for (byte, i) in x.iter().flat_map(|s| s.bytes()).zip(0..) {
        let c = 0x1f00 | (byte as u16);
        unsafe {
            let screen_char = offset(screen_pointer, i) as *mut u16;
            *screen_char = c
        }
    }

    loop{}
}
```
Let's break it down:

- the `core_*` features are needed because many functions from the core library are still unstable
- `use` imports a function, in our case [offset] that can be used for C-like pointer arithmetic
- Normally, Rust tries to avoid [stack overflows] through _guard pages_: The page below the stack isn't mapped and such a stack overflow triggers a page fault. But we can't unmap this page easily as we use just one big page, so we need to be very careful to not blow our small stack. Fortunately our stack is just above the page tables. A stack overflow would overwrite an important entry at some point and therefore provoke a page fault (that crashed our kernel).
- `x` is an immutable array of [&str]
- `screen_pointer` is a [raw pointer] to the VGA text buffer
- We iterate over the elements of x, convert every `&str` to a byte iterator (`s.bytes()`) and join these iterators (through `flat_map`). Then we convert the byte iterator to a `(byte, index)` operator through the [zip method].
- By casting the byte as `u16` and OR-ing the colors in (`0x1f` is white text on blue background), we convert it to a screen character.
- Now we just need to write the `screen_char` to the right place in memory. Rust doesn't know the VGA buffer and can't guarantee that writing to it is safe (we could corrupt important data). So we need to tell Rust that we know what we're doing by using an [unsafe block].
- `offset` is like pointer arithmetic in C: It calculates the address of the i-th `u16` (starting at `0xb8000`) and converts it to a pointer.
- To be able to write to it, we must cast the `*const u16` to a `*mut u16`

[offset]: https://doc.rust-lang.org/nightly/core/intrinsics/fn.offset.html
[stack overflows]: https://en.wikipedia.org/wiki/Stack_overflow
[&str]: https://doc.rust-lang.org/nightly/std/primitive.str.html
[zip method]: https://doc.rust-lang.org/nightly/std/iter/trait.Iterator.html#method.zip
[unsafe block]: https://doc.rust-lang.org/book/unsafe.html

## What's next?
Right now we write magic bits to memory every time we want to print something. It's time to end the hackery. In the [next post] we create a VGA buffer module that allows us to print strings in different colors.

[next post]: #TODO {{ site.url }}{{ page.next.url }}
