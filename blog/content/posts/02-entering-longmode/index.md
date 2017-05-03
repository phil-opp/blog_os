+++
title = "Entering Long Mode"
order = 2
url = "entering-longmode"
date = "2015-08-25"
updated = "2015-10-29"
aliases = [
    "/2015/08/25/entering-longmode/",
    "/rust-os/entering-longmode.html",
]
+++

In the [previous post] we created a minimal multiboot kernel. It just prints `OK` and hangs. The goal is to extend it and call 64-bit [Rust] code. But the CPU is currently in [protected mode] and allows only 32-bit instructions and up to 4GiB memory. So we need to set up _Paging_ and switch to the 64-bit [long mode] first.

[previous post]: ./posts/01-multiboot-kernel/index.md
[Rust]: http://www.rust-lang.org/
[protected mode]: https://en.wikipedia.org/wiki/Protected_mode
[long mode]: https://en.wikipedia.org/wiki/Long_mode

<!-- more --><aside id="toc"></aside>

I tried to explain everything in detail and to keep the code as simple as possible. If you have any questions, suggestions, or issues, please leave a comment or [create an issue] on Github. The source code is available in a [repository][source code], too.

[create an issue]: https://github.com/phil-opp/blog_os/issues
[source code]: https://github.com/phil-opp/blog_os/tree/post_2/src/arch/x86_64

## Some Tests
To avoid bugs and strange errors on old CPUs we should check if the processor supports every needed feature. If not, the kernel should abort and display an error message. To handle errors easily, we create an error procedure in `boot.asm`. It prints a rudimentary `ERR: X` message, where X is an error code letter, and hangs:

```nasm
; Prints `ERR: ` and the given error code to screen and hangs.
; parameter: error code (in ascii) in al
error:
    mov dword [0xb8000], 0x4f524f45
    mov dword [0xb8004], 0x4f3a4f52
    mov dword [0xb8008], 0x4f204f20
    mov byte  [0xb800a], al
    hlt
```
At address `0xb8000` begins the so-called [VGA text buffer]. It's an array of screen characters that are displayed by the graphics card. A [future post] will cover the VGA buffer in detail and create a Rust interface to it. But for now, manual bit-fiddling is the easiest option.

[VGA text buffer]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode
[future post]: ./posts/04-printing-to-screen/index.md

A screen character consists of a 8 bit color code and a 8 bit [ASCII] character. We used the color code `4f` for all characters, which means white text on red background. `0x52` is an ASCII `R`, `0x45` is an `E`, `0x3a` is a `:`, and `0x20` is a space. The second space is overwritten by the given ASCII byte. Finally the CPU is stopped with the `hlt` instruction.

[ASCII]: https://en.wikipedia.org/wiki/ASCII

Now we can add some check _functions_. A function is just a normal label with an `ret` (return) instruction at the end. The `call` instruction can be used to call it. Unlike the `jmp` instruction that just jumps to a memory address, the `call` instruction will push a return address to the stack (and the `ret` will jump to this address). But we don't have a stack yet. The [stack pointer] in the esp register could point to some important data or even invalid memory. So we need to update it and point it to some valid stack memory.

[stack pointer]: http://stackoverflow.com/a/1464052/866447

### Creating a Stack
To create stack memory we reserve some bytes at the end of our `boot.asm`:

```nasm
...
section .bss
stack_bottom:
    resb 64
stack_top:
```
A stack doesn't need to be initialized because we will `pop` only when we `pushed` before. So storing the stack memory in the executable file would make it unnecessary large. By using the [.bss] section and the `resb` (reserve byte) command, we just store the length of the uninitialized data (= 64). When loading the executable, GRUB will create the section of required size in memory.

[.bss]: https://en.wikipedia.org/wiki/.bss

To use the new stack, we update the stack pointer register right after `start`:

```nasm
global start

section .text
bits 32
start:
    mov esp, stack_top

    ; print `OK` to screen
    ...
```
We use `stack_top` because the stack grows downwards: A `push eax` subtracts 4 from `esp` and does a `mov [esp], eax` afterwards (`eax` is a general purpose register).

Now we have a valid stack pointer and are able to call functions. The following check functions are just here for completeness and I won't explain details. Basically they all work the same: They will check for a feature and jump to `error` if it's not available.

### Multiboot check
We rely on some Multiboot features in the next posts. To make sure the kernel was really loaded by a Multiboot compliant bootloader, we can check the `eax` register. According to the Multiboot specification ([PDF][Multiboot specification]), the bootloader must write the magic value `0x36d76289` to it before loading a kernel. To verify that we can add a simple function:

```nasm
check_multiboot:
    cmp eax, 0x36d76289
    jne .no_multiboot
    ret
.no_multiboot:
    mov al, "0"
    jmp error
```
We use the `cmp` instruction to compare the value in `eax` to the magic value. If the values are equal, the `cmp` instruction sets the zero flag in the [FLAGS register]. The `jne` (“jump if not equal”) instruction reads this zero flag and jumps to the given address if it's not set. Thus we jump to the `.no_multiboot` label if `eax` does not contain the magic value.

In `no_multiboot`, we use the `jmp` (“jump”) instruction to jump to our error function. We could just as well use the `call` instruction, which additionally pushes the return address. But the return address is not needed because `error` never returns. To pass `0` as error code to the `error` function, we move it into `al` before the jump (`error` will read it from there).

[Multiboot specification]: http://nongnu.askapache.com/grub/phcoder/multiboot.pdf
[FLAGS register]: https://en.wikipedia.org/wiki/FLAGS_register

### CPUID check
[CPUID] is a CPU instruction that can be used to get various information about the CPU. But not every processor supports it. CPUID detection is quite laborious, so we just copy a detection function from the [OSDev wiki][CPUID detection]:

[CPUID]: http://wiki.osdev.org/CPUID
[CPUID detection]: http://wiki.osdev.org/Setting_Up_Long_Mode#Detection_of_CPUID

```nasm
check_cpuid:
    ; Check if CPUID is supported by attempting to flip the ID bit (bit 21)
    ; in the FLAGS register. If we can flip it, CPUID is available.

    ; Copy FLAGS in to EAX via stack
    pushfd
    pop eax

    ; Copy to ECX as well for comparing later on
    mov ecx, eax

    ; Flip the ID bit
    xor eax, 1 << 21

    ; Copy EAX to FLAGS via the stack
    push eax
    popfd

    ; Copy FLAGS back to EAX (with the flipped bit if CPUID is supported)
    pushfd
    pop eax

    ; Restore FLAGS from the old version stored in ECX (i.e. flipping the
    ; ID bit back if it was ever flipped).
    push ecx
    popfd

    ; Compare EAX and ECX. If they are equal then that means the bit
    ; wasn't flipped, and CPUID isn't supported.
    cmp eax, ecx
    je .no_cpuid
    ret
.no_cpuid:
    mov al, "1"
    jmp error
```
Basically, the `CPUID` instruction is supported if we can flip some bit in the [FLAGS register]. We can't operate on the flags register directly, so we need to load it into some general purpose register such as `eax` first. The only way to do this is to push the `FLAGS` register on the stack through the `pushfd` instruction and then pop it into `eax`. Equally, we write it back through `push ecx` and `popfd`. To flip the bit we use the `xor` instruction to perform an [exclusive OR]. Finally we compare the two values and jump to `.no_cpuid` if both are equal (`je` – “jump if equal”). The `.no_cpuid` code just jumps to the `error` function with error code `1`.

Don't worry, you don't need to understand the details.

[exclusive OR]: https://en.wikipedia.org/wiki/Exclusive_or

### Long Mode check
Now we can use CPUID to detect whether long mode can be used. I use code from [OSDev][long mode detection] again:

[long mode detection]: http://wiki.osdev.org/Setting_Up_Long_Mode#x86_or_x86-64

```nasm
check_long_mode:
    ; test if extended processor info in available
    mov eax, 0x80000000    ; implicit argument for cpuid
    cpuid                  ; get highest supported argument
    cmp eax, 0x80000001    ; it needs to be at least 0x80000001
    jb .no_long_mode       ; if it's less, the CPU is too old for long mode

    ; use extended info to test if long mode is available
    mov eax, 0x80000001    ; argument for extended processor info
    cpuid                  ; returns various feature bits in ecx and edx
    test edx, 1 << 29      ; test if the LM-bit is set in the D-register
    jz .no_long_mode       ; If it's not set, there is no long mode
    ret
.no_long_mode:
    mov al, "2"
    jmp error
```
Like many low-level things, CPUID is a bit strange. Instead of taking a parameter, the `cpuid` instruction implicitely uses the `eax` register as argument. To test if long mode is available, we need to call `cpuid` with `0x80000001` in `eax`. This loads some information to the `ecx` and `edx` registers. Long mode is supported if the 29th bit in `edx` is set. [Wikipedia][cpuid long mode] has detailed information.

[cpuid long mode]: https://en.wikipedia.org/wiki/CPUID#EAX.3D80000001h:_Extended_Processor_Info_and_Feature_Bits

If you look at the assembly above, you'll probably notice that we call `cpuid` twice. The reason is that the CPUID command started with only a few functions and was extended over time. So old processors may not know the `0x80000001` argument at all. To test if they do, we need to invoke `cpuid` with `0x80000000` in `eax` first. It returns the highest supported parameter value in `eax`. If it's at least `0x80000001`, we can test for long mode as described above. Else the CPU is old and doesn't know what long mode is either. In that case, we directly jump to `.no_long_mode` through the `jb` instruction (“jump if below”).

### Putting it together
We just call these check functions right after start:

```nasm
global start

section .text
bits 32
start:
    mov esp, stack_top

    call check_multiboot
    call check_cpuid
    call check_long_mode

    ; print `OK` to screen
    ...
```
When the CPU doesn't support a needed feature, we get an error message with an unique error code. Now we can start the real work.

## Paging
_Paging_ is a memory management scheme that separates virtual and physical memory. The address space is split into equal sized _pages_ and a _page table_ specifies which virtual page points to which physical page. If you never heard of paging, you might want to look at the paging introduction ([PDF][paging chapter]) of the [Three Easy Pieces] OS book.

[paging chapter]: http://pages.cs.wisc.edu/~remzi/OSTEP/vm-paging.pdf
[Three Easy Pieces]: http://pages.cs.wisc.edu/~remzi/OSTEP/

In long mode, x86 uses a page size of 4096 bytes and a 4 level page table that consists of:

- the Page-Map Level-4 Table (PML4),
- the Page-Directory Pointer Table (PDP),
- the Page-Directory Table (PD),
- and the Page Table (PT).

As I don't like these names, I will call them P4, P3, P2, and P1 from now on.

Each page table contains 512 entries and one entry is 8 bytes, so they fit exactly in one page (`512*8 = 4096`). To translate a virtual address to a physical address the CPU[^hardware_lookup] will do the following[^virtual_physical_translation_source]:

![translation of virtual to physical addresses in 64 bit mode](/images/X86_Paging_64bit.svg)

1. Get the address of the P4 table from the CR3 register
2. Use bits 39-47 (9 bits) as an index into P4 (`2^9 = 512 = number of entries`)
3. Use the following 9 bits as an index into P3
4. Use the following 9 bits as an index into P2
5. Use the following 9 bits as an index into P1
6. Use the last 12 bits as page offset (`2^12 = 4096 = page size`)

But what happens to bits 48-63 of the 64-bit virtual address? Well, they can't be used. The “64-bit” long mode is in fact just a 48-bit mode. The bits 48-63 must be copies of bit 47, so each valid virtual address is still unique. For more information see [Wikipedia][wikipedia_48bit_mode].

[wikipedia_48bit_mode]: https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details

An entry in the P4, P3, P2, and P1 tables consists of the page aligned 52-bit _physical_ address of the frame or the next page table and the following bits that can be OR-ed in:

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
52-62 | available | can be used freely by the OS
63 | no execute | forbid executing code on this page (the NXE bit in the EFER register must be set)

### Set Up Identity Paging
When we switch to long mode, paging will be activated automatically. The CPU will then try to read the instruction at the following address, but this address is now a virtual address. So we need to do _identity mapping_, i.e. map a physical address to the same virtual address.

The `huge page` bit is now very useful to us. It creates a 2MiB (when used in P2) or even a 1GiB page (when used in P3). So we could map the first _gigabytes_ of the kernel with only one P4 and one P3 table by using 1GiB pages. Unfortunately 1GiB pages are relatively new feature, for example Intel introduced it 2010 in the [Westmere architecture]. Therefore we will use 2MiB pages instead to make our kernel compatible to older computers, too.
[Westmere architecture]: https://en.wikipedia.org/wiki/Westmere_(microarchitecture)#Technology

To identity map the first gigabyte of our kernel with 512 2MiB pages, we need one P4, one P3, and one P2 table. Of course we will replace them with finer-grained tables later. But now that we're stuck with assembly, we choose the easiest way.

We can add these two tables at the beginning[^page_table_alignment] of the `.bss` section:

```nasm
...

section .bss
align 4096
p4_table:
    resb 4096
p3_table:
    resb 4096
p2_table:
    resb 4096
stack_bottom:
    resb 64
stack_top:
```
The `resb` command reserves the specified amount of bytes without initializing them, so the 8KiB don't need to be saved in the executable. The `align 4096` ensures that the page tables are page aligned.

When GRUB creates the `.bss` section in memory, it will initialize it to `0`. So the `p4_table` is already valid (it contains 512 non-present entries) but not very useful. To be able to map 2MiB pages, we need to link P4's first entry to the `p3_table` and P3's first entry to the the `p2_table`:

```nasm
set_up_page_tables:
    ; map first P4 entry to P3 table
    mov eax, p3_table
    or eax, 0b11 ; present + writable
    mov [p4_table], eax

    ; map first P3 entry to P2 table
    mov eax, p2_table
    or eax, 0b11 ; present + writable
    mov [p3_table], eax

    ; TODO map each P2 entry to a huge 2MiB page
    ret
```
We just set the present and writable bits (`0b11` is a binary number) in the aligned P3 table address and move it to the first 4 bytes of the P4 table. Then we do the same to link the first P3 entry to the `p2_table`.

Now we need to map P2's first entry to a huge page starting at 0, P2's second entry to a huge page starting at 2MiB, P2's third entry to a huge page starting at 4MiB, and so on. It's time for our first (and only) assembly loop:

```nasm
set_up_page_tables:
    ...
    ; map each P2 entry to a huge 2MiB page
    mov ecx, 0         ; counter variable

.map_p2_table:
    ; map ecx-th P2 entry to a huge page that starts at address 2MiB*ecx
    mov eax, 0x200000  ; 2MiB
    mul ecx            ; start address of ecx-th page
    or eax, 0b10000011 ; present + writable + huge
    mov [p2_table + ecx * 8], eax ; map ecx-th entry

    inc ecx            ; increase counter
    cmp ecx, 512       ; if counter == 512, the whole P2 table is mapped
    jne .map_p2_table  ; else map the next entry

    ret
```
Maybe I should first explain how an assembly loop works. We use the `ecx` register as a counter variable, just like `i` in a for loop. After mapping the `ecx-th` entry, we increase `ecx` by one and jump to `.map_p2_table` again if it's still smaller than 512.

To map a P2 entry we first calculate the start address of its page in `eax`: The `ecx-th` entry needs to be mapped to `ecx * 2MiB`. We use the `mul` operation for that, which multiplies `eax` with the given register and stores the result in `eax`. Then we set the `present`, `writable`, and `huge page` bits and write it to the P2 entry. The address of the `ecx-th` entry in P2 is `p2_table + ecx * 8`, because each entry is 8 bytes large.

Now the first gigabyte (512 * 2MiB) of our kernel is identity mapped and thus accessible through the same physical and virtual addresses.

### Enable Paging
To enable paging and enter long mode, we need to do the following:

1. write the address of the P4 table to the CR3 register (the CPU will look there, see the [paging section](#paging))
2. long mode is an extension of [Physical Address Extension] \(PAE), so we need to enable PAE first
3. Set the long mode bit in the EFER register
4. Enable Paging

[Physical Address Extension]: https://en.wikipedia.org/wiki/Physical_Address_Extension

The assembly function looks like this (some boring bit-moving to various registers):

```nasm
enable_paging:
    ; load P4 to cr3 register (cpu uses this to access the P4 table)
    mov eax, p4_table
    mov cr3, eax

    ; enable PAE-flag in cr4 (Physical Address Extension)
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; set the long mode bit in the EFER MSR (model specific register)
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    ; enable paging in the cr0 register
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    ret
```
The `or eax, 1 << X` is a common pattern. It sets the bit `X` in the eax register (`<<` is a left shift). Through `rdmsr` and `wrmsr` it's possible to read/write to the so-called model specific registers at address `ecx` (in this case `ecx` points to the EFER register).

Finally we need to call our new functions in `start`:

```nasm
...
start:
    mov esp, stack_top

    call check_multiboot
    call check_cpuid
    call check_long_mode

    call set_up_page_tables ; new
    call enable_paging     ; new

    ; print `OK` to screen
    mov dword [0xb8000], 0x2f4b2f4f
    hlt
...
```
To test it we execute `make run`. If the green OK is still printed, we have successfully enabled paging!

## The Global Descriptor Table
After enabling Paging, the processor is in long mode. So we can use 64-bit instructions now, right? Wrong. The processor is still in a 32-bit compatibility submode. To actually execute 64-bit code, we need to set up a new Global Descriptor Table.
The Global Descriptor Table (GDT) was used for _Segmentation_ in old operating systems. I won't explain Segmentation but the [Three Easy Pieces] OS book has good introduction ([PDF][Segmentation chapter]) again.

[Segmentation chapter]: http://pages.cs.wisc.edu/~remzi/OSTEP/vm-segmentation.pdf

Today almost everyone uses Paging instead of Segmentation (and so do we). But on x86, a GDT is always required, even when you're not using Segmentation. GRUB has set up a valid 32-bit GDT for us but now we need to switch to a long mode GDT.

A GDT always starts with a 0-entry and contains an arbitrary number of segment entries afterwards. A 64-bit entry has the following format:

Bit(s)                | Name | Meaning
--------------------- | ------ | ----------------------------------
0-41 | ignored | ignored in 64-bit mode
42 | conforming | the current privilege level can be higher than the specified level for code segments (else it must match exactly)
43 | executable | if set, it's a code segment, else it's a data segment
44 | descriptor type | should be 1 for code and data segments
45-46 | privilege | the [ring level]: 0 for kernel, 3 for user
47 | present | must be 1 for valid selectors
48-52 | ignored | ignored in 64-bit mode
53 | 64-bit | should be set for 64-bit code segments
54 | 32-bit | must be 0 for 64-bit segments
55-63 | ignored | ignored in 64-bit mode

[ring level]: http://wiki.osdev.org/Security#Rings

We need one code segment, a data segment is not necessary in 64-bit mode. Code segments have the following bits set: _descriptor type_, _present_, _executable_ and the _64-bit_ flag. Translated to assembly the long mode GDT looks like this:

```nasm
section .rodata
gdt64:
    dq 0 ; zero entry
    dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; code segment
```
We chose the `.rodata` section here because it's initialized read-only data. The `dq` command stands for `define quad` and outputs a 64-bit constant (similar to `dw` and `dd`). And the `(1<<43)` is a [bit shift] that sets bit 43.

[bit shift]: http://www.cs.umd.edu/class/sum2003/cmsc311/Notes/BitOp/bitshift.html

### Loading the GDT
To load our new 64-bit GDT, we have to tell the CPU its address and length. We do this by passing the memory location of a special pointer structure to the `lgdt` (load GDT) instruction. The pointer structure looks like this:

```nasm
gdt64:
    dq 0 ; zero entry
    dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; code segment
.pointer:
    dw $ - gdt64 - 1
    dq gdt64
```
The first 2 bytes specify the (GDT length - 1). The `$` is a special symbol that is replaced with the current address (it's equal to `.pointer` in our case). The following 8 bytes specify the GDT address. Labels that start with a point (such as `.pointer`) are sub-labels of the last label without point. To access them, they must be prefixed with the parent label (e.g., `gdt64.pointer`).

Now we can load the GDT in `start`:

```nasm
start:
    ...
    call enable_paging

    ; load the 64-bit GDT
    lgdt [gdt64.pointer]

    ; print `OK` to screen
    ...
```
When you still see the green `OK`, everything went fine and the new GDT is loaded. But we still can't execute 64-bit code: The code selector register `cs` still has the values from the old GDT. To update it, we need to load it with the GDT offset (in bytes) of the desired segment. In our case the code segment starts at byte 8 of the GDT, but we don't want to hardcode that 8 (in case we modify our GDT later). Instead, we add a `.code` label to our GDT, that calculates the offset directly from the GDT:

```nasm
section .rodata
gdt64:
    dq 0 ; zero entry
.code: equ $ - gdt64 ; new
    dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; code segment
.pointer:
    ...
```
We can't just use a normal label here, since we need the table _offset_. We calculate this offset using the current address `$` and set the label to this value using [equ]. Now we can use `gdt64.code` instead of 8 and this label will still work if we modify the GDT.

[equ]: http://www.nasm.us/doc/nasmdoc3.html#section-3.2.4

In order to finally enter the true 64-bit mode, we need to load `cs` with `gdt64.code`. But we can't do it through `mov`. The only way to reload the code selector is a _far jump_ or a _far return_. These instructions work like a normal jump/return but change the code selector. We use a far jump to a long mode label:

```nasm
global start
extern long_mode_start
...
start:
    ...
    lgdt [gdt64.pointer]

    jmp gdt64.code:long_mode_start
...
```
The actual `long_mode_start` label is defined as `extern`, so it's part of another file. The `jmp gdt64.code:long_mode_start` is the mentioned far jump.

I put the 64-bit code into a new file to separate it from the 32-bit code, thereby we can't call the (now invalid) 32-bit code accidentally. The new file (I named it `long_mode_init.asm`) looks like this:

```nasm
global long_mode_start

section .text
bits 64
long_mode_start:
    ; print `OKAY` to screen
    mov rax, 0x2f592f412f4b2f4f
    mov qword [0xb8000], rax
    hlt
```
You should see a green `OKAY` on the screen. Some notes on this last step:

- As the CPU expects 64-bit instructions now, we use `bits 64`
- We can now use the extended registers. Instead of the 32-bit `eax`, `ebx`, etc. we now have the 64-bit `rax`, `rbx`, …
- and we can write these 64-bit registers directly to memory using `mov qword` (quad word)

_Congratulations_! You have successfully wrestled through this CPU configuration and compatibility mode mess :).

#### One Last Thing
Above, we reloaded the code segment register `cs` with the new GDT offset. However, the data segment registers `ss`, `ds`, `es`, `fs`, and `gs` still contain the data segment offsets of the old GDT. This isn't necessarily bad, since they're ignored by almost all instructions in 64-bit mode. However, there are a few instructions that expect a valid data segment descriptor _or the null descriptor_ in those registers. An example is the the [iretq] instruction that we'll need in the [_Returning from Exceptions_] post.

[iretq]: ./extra/handling-exceptions-with-naked-fns/returning-from-exceptions.md#the-iretq-instruct/indexion
[_Returning from Exceptions_]: ./extra/handling-exceptions-with-naked-fns/returning-from-exceptions.md

To avoid future problems, we reload all data segment registers with null:

```nasm
long_mode_start:
    ; load 0 into all data segment registers
    mov ax, 0
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    ; print `OKAY` to screen
    ...
```

## What's next?
It's time to finally leave assembly behind and switch to [Rust]. Rust is a systems language without garbage collections that guarantees memory safety. Through a real type system and many abstractions it feels like a high-level language but can still be low-level enough for OS development. The [next post] describes the Rust setup.

[Rust]: https://www.rust-lang.org/
[next post]: ./posts/03-set-up-rust/index.md

## Footnotes
[^hardware_lookup]: In the x86 architecture, the page tables are _hardware walked_, so the CPU will look at the table on its own when it needs a translation. Other architectures, for example MIPS, just throw an exception and let the OS translate the virtual address.

[^virtual_physical_translation_source]: Image source: [Wikipedia](https://commons.wikimedia.org/wiki/File:X86_Paging_64bit.svg), with modified font size, page table naming, and removed sign extended bits. The modified file is licensed under the Creative Commons Attribution-Share Alike 3.0 Unported license.

[^page_table_alignment]: Page tables need to be page-aligned as the bits 0-11 are used for flags. By putting these tables at the beginning of `.bss`, the linker can just page align the whole section and we don't have unused padding bytes in between.
