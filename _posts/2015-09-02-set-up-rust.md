---
layout: post
title: 'Set Up Rust'
redirect_from: "/2015/09/02/setup-rust/"
redirect_from: "/setup-rust.html"
---
In the previous posts we created a [minimal Multiboot kernel][multiboot post] and [switched to Long Mode][long mode post]. Now we can finally switch to [Rust] code. Rust is a high-level language without runtime. It allows us to not link the standard library and write bare metal code. Unfortunately the setup is not quite hassle-free yet.

This blog post tries to set up Rust step-by-step and point out the different problems. If you have any questions, problems, or suggestions please [file an issue] or create a comment at the bottom. The code from this post is in a [Github repository], too.

[multiboot post]: {{ page.previous.previous.url }}
[long mode post]: {{ page.previous.url }}
[Rust]: https://www.rust-lang.org/
[file an issue]: https://github.com/phil-opp/blog_os/issues
[Github repository]: https://github.com/phil-opp/blog_os/tree/set_up_rust

## Installing Rust
We need a nightly compiler, as we will use many unstable features. To manage Rust installations I highly recommend brson's [multirust]. It allows you to install nightly, beta, and stable compilers side-by-side and makes it easy to update them. To use a nightly compiler for the current directory, you can run `multirust override nightly`.

[multirust]: https://github.com/brson/multirust

## Creating a Cargo project
[Cargo] is Rust excellent package manager. Normally you would call `cargo new` when you want to create a new project folder. We can't use it because our folder already exists, so we need to do it manually. Fortunately we only need to add a cargo configuration file named `Cargo.toml`:

[Cargo]: http://doc.crates.io/guide.html

```toml
[package]
name = "blog_os"
version = "0.1.0"
authors = ["Philipp Oppermann <dev@phil-opp.com>"]

[lib]
crate-type = ["staticlib"]
```
The `package` section contains required project metadata such as the [semantic crate version]. The `lib` section specifies that we want to build a static library, i.e. a library that contains all of its dependencies. This is required to link the Rust project with our kernel.

[semantic crate version]: http://doc.crates.io/manifest.html#the-package-section

Now we place our root source file in `src/lib.rs`:

```rust
#![feature(lang_items)]
#![no_std]

#[no_mangle]
pub extern fn rust_main() {}

#[lang = "eh_personality"] extern fn eh_personality() {}
#[lang = "panic_fmt"] extern fn panic_fmt() -> ! {loop{}}
```
Let's break it down:

- `#!` defines an [attribute] of the current module. Since we are at the root module, they apply to the crate itself.
- The `feature` attribute is used to allow the specified _feature-gated_ attributes in this crate. You can't do that in a stable/beta compiler, so this is one reason we need a Rust nighly.
- The `no_std` attribute prevents the automatic linking of the standard library. We can't use `std` because it relies on operating system features like files, system calls, and various device drivers. Remember that currently the only “feature” of our OS is printing `OKAY` :).
- A `#` without a `!` afterwards defines an attribute for the _following_ item (a function in our case).
- The `no_mangle` attribute disables the automatic [name mangling] that Rust uses to get unique function names. We want to do a `call rust_main` from our assembly code, so this function name must stay as it is.
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
We can now build it using `cargo build`. To make sure, we are building it for the x86_64 architecture, we can pass an explicit target:

```bash
cargo build --target=x86_64-unknown-linux-gnu
```
It creates a static library at `target/x86_64-unknown-linux-gnu/debug/libblog_os.a`, which can be linked with our assembly kernel. If you're getting an error about a missing `core` crate, [look here][cross compile libcore].
[cross compile libcore]: /cross-compile-libcore.html

To build and link the rust library on `make`, we extend our `Makefile`([full file][github makefile]):

```make
# ...
target ?= $(arch)-unknown-linux-gnu
rust_os := target/$(target)/debug/libblog_os.a
# ...
$(kernel): cargo $(rust_os) $(assembly_object_files) $(linker_script)
       @ld -n -T $(linker_script) -o $(kernel) $(assembly_object_files) $(rust_os)

cargo:
       @cargo build --target $(target)
```
We added a new `cargo` target that just executes `cargo build` and modified the `$(kernel)` target to link the created static lib .

But now `cargo build` is executed on every `make`, even if no source file was changed. And the ISO is recreated on every `make iso`/`make run`, too. We could try to avoid this by adding dependencies on all rust source and cargo configuration files to the `cargo` target, but the ISO creation takes only half a second on my machine and most of the time we will have changed a Rust file when we run `make`. So we keep it simple for now and let cargo do the bookkeeping of changed files (it does it anyway).

[github makefile]: https://github.com/phil-opp/blog_os/blob/set_up_rust/Makefile

## Calling Rust
Now we can call the main method in `long_mode_start`:

```nasm
bits 64
long_mode_start:
    ; call the rust main
    extern rust_main     ; new
    call rust_main       ; new

    ; print `OKAY` to screen
    mov rax, 0x2f592f412f4b2f4f
    mov qword [0xb8000], rax
    hlt
```
By defining `rust_main` as `extern` we tell nasm that the function is defined in another file. As the linker takes care of linking them together, we'll get a linker error if we have a typo in the name or forget to mark the rust function as `pub extern`.

If we've done everything right, we should still see the green `OKAY` when executing `make run`. That means that we successfully called the Rust function and returned back to assembly.

## Fixing Linker Errors
Now we can try some Rust code:

```rust
pub extern fn rust_main() {
    let x = ["Hello", " ", "World", "!"];
}
```
When we test it using `make run`, it fails with `undefined reference to 'memcpy'`. The `memcpy` function is one of the basic functions of the C library (`libc`). Usually the `libc` crate is linked to every Rust program together with the standard library, but we opted out through `#![no_std]`. We could try to fix this by adding the [libc crate] as `extern crate`. But `libc` is just a wrapper for the system `libc`, for example `glibc` on Linux, so this won't work for us. Instead we need to recreate the basic `libc` functions such as `memcpy`, `memmove`, `memset`, and `memcmp` in Rust.

[libc crate]: https://doc.rust-lang.org/nightly/libc/index.html

### rlibc
Fortunately there already is a crate for that: [rlibc]. When we look at its [source code][rlibc source] we see that it contains no magic, just some [raw pointer] operations in a while loop. To add `rlibc` as a dependency we just need to add two lines to the `Cargo.toml`:

```toml
...
[dependencies]
rlibc = "0.1.4"
```
and an `extern crate` definition in our `src/lib.rs`:

```rust
...
extern crate rlibc;

#[no_mangle]
pub extern fn rust_main() {
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
The new errors are linker errors about missing `fmod` and `fmodf` functions. These functions are used for the modulo operation (`%`) on floating point numbers in [libcore]. The core library is added implicitly when using `#![no_std]` and provides basic standard library features like `Option` or `Iterator`. According to the documentation it is “dependency-free”. But it actually has some dependencies, for example on `fmod` and `fmodf`.

[libcore]: https://doc.rust-lang.org/core/

So how do we fix this problem? We don't use any floating point operations, so we could just provide our own implementations of `fmod` and `fmodf` that just do a `loop{}`. But there's a better way that doesn't fail silently when we use float modulo some day: We tell the linker to remove unused sections. That's generally a good idea as it reduces kernel size. And we don't have any references to `fmod` and `fmodf` anymore until we use floating point modulo. The magic linker flag is `--gc-sections`, which stands for “garbage collect sections”. Let's add it to the `$(kernel)` target in our `Makefile`:

```make
$(kernel): cargo $(rust_os) $(assembly_object_files) $(linker_script)
	@ld -n --gc-sections -T $(linker_script) -o $(kernel) $(assembly_object_files) $(rust_os)
```
Now we can do a `make run` again and… it doesn't boot anymore:

```
GRUB error: no multiboot header found.
```
What happened? Well, the linker removed unused sections. And since we don't use the Multiboot section anywhere, `ld` removes it, too. So we need to tell the linker explicitely that it should keep this section. The `KEEP` command does exactly that, so we add it to the linker script (`linker.ld`):

```
.boot :
{
    /* ensure that the multiboot header is at the beginning */
    KEEP(*(.multiboot_header))
}
```
Now everything should work again (the green `OKAY`). But there is another linking issue, which is triggered by some other example code.

### no-landing-pads

The following snippet still fails:

```rust
    ...
    let test = (0..3).flat_map(|x| 0..x).zip(0..);
```
The error is a linker error again (hence the ugly error message):

```
target/debug/libblog_os.a(blog_os.0.o): In function `blog_os::iter::Iterator::zip<core::iter::FlatMap<core::ops::Range<i32>, core::ops::Range<i32>, closure>,core::ops::RangeFrom<i32>>':
/home/.../src/libcore/iter.rs:654: undefined reference to `_Unwind_Resume'
```
So the linker can't find a function named `_Unwind_Resume` that is referenced in `iter.rs:654` in libcore. This reference is not really there at [line 654 of libcore's `iter.rs`][iter.rs:654]. Instead, it is a compiler inserted _landing pad_, which is used for exception handling.

The easiest way of fixing this problem is to disable the landing pad creation since we don't supports panics anyway right now. We can do this by passing a `-Z no-landing-pads` flag to `rustc` (the actual Rust compiler below cargo). To do this we replace the `cargo build` command in our Makefile with the `cargo rustc` command, which does the same but allows passing flags to `rustc`:

```make
cargo:
    @cargo rustc --target $(target) -- -Z no-landing-pads
```
Now we fixed all linking issues.

(For completeness, there is another flag you should pass to `rustc` as soon as you enable interrupts: `-C no-redzone`. For more information see the [Github issue][redzone issue]).

[redzone issue]:https://github.com/phil-opp/blog_os/issues/10

## The final problem
Unfortunately there is one last problem left, that gets triggered by the following code:

```rust
let mut a = ("hello", 42);
a.1 += 1;
```
When we add that code to `rust_main` and test it using `make run`, the OS will constantly reboot itself. Let's try to debug it.

[iter.rs:654]: https://doc.rust-lang.org/nightly/src/core/iter.rs.html#654

### Debugging
Such a boot loop is most likely caused by some [CPU exception][exception table]. When these exceptions aren't handled, a [Triple Fault] occurs and the processor resets itself. We can look at generated CPU interrupts/exceptions using QEMU:

[exception table]: http://wiki.osdev.org/Exceptions
[Triple Fault]: http://wiki.osdev.org/Triple_Fault

```
> qemu-system-x86_64 -d int -no-reboot -drive format=raw,file=build/os-x86_64.iso
SMM: enter
...
SMM: after RSM
...
check_exception old: 0xffffffff new 0x6
     0: v=06 e=0000 i=0 cpl=0 IP=0008:00000000001001d3 pc=00000000001001d3
     SP=0010:0000000000102ff0 env->regs[R_EAX]=000000000000002a
...
check_exception old: 0xffffffff new 0xd
     1: v=0d e=0062 i=0 cpl=0 IP=0008:00000000001001d3 pc=00000000001001d3
     SP=0010:0000000000102ff0 env->regs[R_EAX]=000000000000002a
...
check_exception old: 0xd new 0xd
     2: v=08 e=0000 i=0 cpl=0 IP=0008:00000000001001d3 pc=00000000001001d3
     SP=0010:0000000000102ff0 env->regs[R_EAX]=000000000000002a
...
check_exception old: 0x8 new 0xd
```
Let me first explain the QEMU arguments: The `-d int` logs CPU interrupts to the console and the `-no-reboot` flag closes QEMU instead of constant rebooting. But what does the cryptical output mean? I already omitted most of it as we don't need it here. Let's break down the rest:

- The first two blocks, `SMM: enter` and `SMM: after RSM` are created before our OS boots, so we just ignore them.
- The next block, `check_exception old: 0xffffffff new 0x6` is the interesting one. It says: “a new CPU exception with number `0x6` occurred“.
- The last blocks indicate further exceptions. They were thrown because we didn't handle the `0x6` exception, so we're going to ignore them, too.

So let's look at the first exception: `old:0xffffffff` means that the CPU wasn't handling an interrupt when the exception occurred. The new exception has number `0x6`. By looking at an [exception table] we learn that `0x6` indicates a [Invalid Opcode] fault. So the lastly executed instruction was invalid. The register dump tells us that the current instruction was `0x1001d3` (through `IP`  (instruction pointer) or `pc` (program counter)). Therefore the instruction at `0x1001d3` seems to be invalid. We can look at it using `objdump`:

[Invalid Opcode]: http://wiki.osdev.org/Exceptions#Invalid_Opcode

```
> objdump -D build/kernel-x86_64.bin | grep "1001d3:"
1001d3:	0f 10 05 16 01 00 00 	movups 0x116(%rip),%xmm0 ...
```
Through `objdump -D` we disassemble our whole kernel and `grep` picks the relevant line. The instruction at `0x1001d3` seems to be a valid `movaps` instruction. It's a [SSE] instruction that moves 128 bit between memory and SSE-registers (e.g. `xmm0`). But why the `Invalid Opcode` exception? The answer is hidden behind the [movaps documentation][movaps]: The section _Protected Mode Exceptions_ lists the conditions for the various exceptions. The short code of the `Invalid Opcode` is `#UD`, so the exception occurs
> For an unmasked Streaming SIMD Extensions 2 instructions numeric exception (CR4.OSXMMEXCPT =0). If EM in CR0 is set. If OSFXSR in CR4 is 0. If CPUID feature flag SSE2 is 0.

[SSE]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions
[movaps]: http://www.c3se.chalmers.se/Common/VTUNE-9.1/doc/users_guide/mergedProjects/analyzer_ec/mergedProjects/reference_olh/mergedProjects/instructions/instruct32_hh/vc181.htm

The rough translation of this cryptic definition is: _If SSE isn't enabled_. So apparently Rust uses SSE instructions by default and we didn't enable SSE before. So the fix for this bug is enabling SSE.

### Enabling SSE
To enable SSE, assembly code is needed again. We want to add a function that tests if SSE is available and enables it then. Else we want to print an error message.

We add it to the `boot.asm` file:

```nasm
; Check for SSE and enable it. If it's not supported throw error "a".
set_up_SSE:
    ; check for SSE
    mov eax, 0x1
    cpuid
    test edx, 1<<25
    jz .no_SSE

    ; enable SSE
    mov eax, cr0
    and ax, 0xFFFB      ; clear coprocessor emulation CR0.EM
    or ax, 0x2          ; set coprocessor monitoring  CR0.MP
    mov cr0, eax
    mov eax, cr4
    or ax, 3 << 9       ; set CR4.OSFXSR and CR4.OSXMMEXCPT at the same time
    mov cr4, eax

    ret
.no_SSE:
    mov al, "a"
    jmp error
```
The code is from the great [OSDev Wiki][osdev sse] again. Notice that it sets/unsets exactly the bits that can cause the `Invalid Opcode` exception.

When we insert a `call set_up_SSE` somewhere in the `start` function (for example after `call enable_paging`), our Rust code will finally work.

[32-bit error function]: {{ page.previous.url }}#some-tests
[osdev sse]: http://wiki.osdev.org/SSE#Checking_for_SSE

### “OS returned!”
Now that we're editing assembly anyway, we should change the `OKAY` message to something more meaningful. My suggestion is a red `OS returned!`:

```nasm
...
call rust_main

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
Ok, that's enough assembly for now. Let's switch back to Rust.

## Hello World!
Finally, it's time for a `Hello World!` from Rust:

```rust
pub extern fn rust_main() {
    // ATTENTION: we have a very small stack and no guard page

    let hello = b"Hello World!";
    let color_byte = 0x1f; // white foreground, blue background

    let mut hello_colored = [color_byte; 24];
    for (i, char_byte) in hello.into_iter().enumerate() {
        hello_colored[i*2] = *char_byte;
    }

    // write `Hello World!` to the center of the VGA text buffer
    let buffer_ptr = (0xb8000 + 1988) as *mut _;
    unsafe { *buffer_ptr = hello_colored };

    loop{}
}
```
Some notes:

- The `b` prefix creates a [byte string], which is just an array of `u8`
- [enumerate] is an `Iterator` method that adds the current index `i` to elements
- `buffer_ptr` is a [raw pointer] that points to the center of the VGA text buffer
- Rust doesn't know the VGA buffer and thus can't guarantee that writing to the `buffer_ptr` is safe (it could point to important data). So we need to tell Rust that we know what we are doing by using an [unsafe block].

[byte string]: https://doc.rust-lang.org/reference.html#characters-and-strings
[enumerate]: https://doc.rust-lang.org/nightly/core/iter/trait.Iterator.html#method.enumerate
[unsafe block]: https://doc.rust-lang.org/book/unsafe.html

### Stack Overflows
Since we still use the small 64 byte [stack from the last post], we must be careful not to [overflow] it. Normally, Rust tries to avoid stack overflows through _guard pages_: The page below the stack isn't mapped and such a stack overflow triggers a page fault (instead of silently overwriting random memory). But we can't unmap the page below our stack right now since we currently use only a single big page. Fortunately the stack is located just above the page tables. So some important page table entry would probably get overwritten on stack overflow and then a page fault occurs, too.

[stack from the last post]: {{ page.previous.url }}#creating-a-stack
[overflow]: https://en.wikipedia.org/wiki/Stack_overflow

## What's next?
Until now we write magic bits to some memory location when we want to print something to screen. In the [next post] we create a abstraction for the VGA text buffer that allows us to print strings in different colors and provides a simple interface.

[next post]: {{ page.next.url }}
