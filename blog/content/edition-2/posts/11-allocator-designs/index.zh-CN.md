+++
title = "分配器设计"
weight = 11
path = "zh-CN/allocator-designs"
date = 2020-01-20

[extra]
chapter = "Memory Management"
# Please update this when updating the translation
translation_based_on_commit = "4e512846617109334af6ae9b1ed03e223cf4b1d0"
# GitHub usernames of the people that translated this post
translators = ["ttttyy"]
# GitHub usernames of the people that contributed to this translation
translation_contributors = []
+++


这篇文章讲解了如何从零开始实现堆分配器。文中介绍并探讨了三种不同的分配器设计，包括bump分配器，链表分配器和固定大小块分配器。对于这三种设计，我们都将构建一个基础实现，供我们的内核使用。
<!-- more -->

这个系列的 blog 在 [GitHub] 上开放开发，如果你有任何问题，请在这里开一个 issue 来讨论。当然你也可以在 [底部][at the bottom] 留言。你可以在 [`post-11`][post branch] 找到这篇文章的完整源码。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-11

<!-- toc -->

## 介绍

在 [上一篇文章][previous post] 中，我们为内核添加了基本的堆分配支持。为此，我们在页表中 [创建了一个新的内存区域][map-heap] ，并使用[`linked_list_allocator` crate][use-alloc-crate] 来管理它。现在我们有了一个可以工作的堆，但是我们将大部分工作留给了分配器crate而没有试着理解它是如何工作的。


[previous post]: @/edition-2/posts/10-heap-allocation/index.md
[map-heap]: @/edition-2/posts/10-heap-allocation/index.md#creating-a-kernel-heap
[use-alloc-crate]: @/edition-2/posts/10-heap-allocation/index.md#using-an-allocator-crate

在本文中，我们将展示如何从零开始实现我们自己的堆分配器，而不是依赖于一个现有的分配器crate。我们将讨论不同的分配器设计，包括一个简化的 _bump 分配器_ 和一个基础的 _固定大小块分配器_ ，并且使用这些知识实现一个性能更好的分配器（相比于`linked_list_allocator` crate）。


### 设计目标

分配器的职责就是管理可用的堆内存。它需要在`alloc`调用中返回未使用的内存，跟踪被`dealloc`方法释放的内存，以便能再次使用。更重要的是，它必须永远不重复分配已在其他地方使用的内存，因为这会导致未定义的行为。


除了正确性以外，还有许多次要的设计目标。举例来说，分配器应该高效利用可用的内存，并且尽量减少 [碎片化][_fragmentation_] 。此外，它还应适用于并发应用程序，并且可以扩展到任意数量的处理器。为了达到最佳性能，它甚至可以针对CPU缓存优化内存布局，以提高 [缓存局部性][cache locality] 并避免 [假共享][false sharing] 。


[cache locality]: https://www.geeksforgeeks.org/locality-of-reference-and-cache-operation-in-cache-memory/
[_fragmentation_]: https://en.wikipedia.org/wiki/Fragmentation_(computing)
[false sharing]: https://mechanical-sympathy.blogspot.de/2011/07/false-sharing.html

这些需求使得优秀的分配器变得非常复杂。例如，[jemalloc] 有超过30,000行代码。这种复杂性不是内核代码所期望的，因为一个简单的bug就能导致严重的安全漏洞。幸运的是，内核代码的内存分配模式通常比用户空间代码简单得多，所以相对简单的分配器设计通常就足够了。

[jemalloc]: http://jemalloc.net/

接下来，我们将展示三种可能的内存分配器设计并且解释它们的优缺点。

## Bump分配器

最简单的分配器设计是 _bump分配器_（也被称为 _栈分配器_ ）。它线性分配内存，并且只跟踪已分配的字节数量和分配的次数。它只适用于非常特殊的使用场景，因为他有一个严重的限制：它只能一次性释放全部内存。

### 设计思想

bump分配器的设计思想是通过增加（_"bumping"_）一个指向未使用内存起点的 `next` 变量的值来线性分配内存。一开始，`next`指向堆的起始地址。每次分配内存时，`next`的值都会增加相应的分配大小，从而始终指向已使用和未使用内存之间的边界。


![堆内存区域在三个时间点的状态：
 1：一次分配发生在堆的起始位置，`next` 指针指向它的末尾。
 2：在第一次分配之后，又添加了第二次分配，`next` 指针指向第二次分配的末尾。
 3：在第二次分配之后，又添加了第三次分配，`next` 指针指向第三次分配的末尾。
 ](bump-allocation.svg)

`next` 指针只朝一个方向移动，因此同一块内存区域永远不会被重复分配。当它到达堆的末尾时，不再有内存可以分配，下一次分配将导致内存不足错误。


一个bump分配器通常会配合一个分配计数器来实现，每次调用 `alloc` 时增加1；每次调用 `dealloc` 减少1。当分配计数器为零时，这意味着堆上的所有分配都已被释放。在这种情况下，`next` 指针可以被重置为堆的起始地址，使整个堆内存再次可用于分配。

### 实现

我们从声明一个新的 `allocator::bump` 子模块开始实现：

```rust
// in src/allocator.rs

pub mod bump;
```

子模块的内容位于一个新的 `src/allocator/bump.rs` 文件中，我们将使用下面的内容创建它：

```rust
// in src/allocator/bump.rs

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    /// 创建一个新的空的bump分配器
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    /// 用给定的堆边界初始化bump分配器
    /// 这个方法是不安全的，因为调用者必须确保给定
    /// 的内存范围没有被使用。同样，这个方法只能被调用一次。

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}
```

`heap_start` 和 `heap_end` 字段跟踪堆内存区域的下界和上界。调用者需要保证这些地址是可用的，否则分配器将返回无效的内存。因此，`init` 函数需要声明为 `unsafe` 。


`next` 字段的作用是始终指向堆的第一个未使用字节，即下一次分配的起始地址。在 `init` 函数中，它被设置为`heap_start` ，因为开始时整个堆都是未使用的。每次分配时，这个字段都会增加相应的分配大小（_“bumped”_），以确保我们不会两次返回相同的内存区域。

`allocations` 字段是一个用于记录活动分配数的简单计数器，其目标是在释放最后一次分配后重置分配器。它的初始值为0。

我们选择创建一个单独的 `init` 函数，而不是直接在 `new` 中执行初始化，是为了保持接口与 `linked_list_allocator` crate 提供的分配器接口一致。这样，分配器就可以在不额外更改代码的情况下进行切换。


### 实现`GlobalAlloc`

正如 [上篇文章所述][global-alloc] ，所有的堆分配器都必须实现 [`GlobalAlloc`] 特征，其定义如下：


[global-alloc]: @/edition-2/posts/10-heap-allocation/index.md#the-allocator-interface
[`GlobalAlloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html

```rust
pub unsafe trait GlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 { ... }
    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize
    ) -> *mut u8 { ... }
}
```

只有 `alloc` 和 `dealloc` 方法是必须实现的；其他两个方法已有默认实现，可以省略。

#### 第一次实现尝试


让我们试着为我们的 `BumpAllocator` 实现 `alloc` 方法：

```rust
// in src/allocator/bump.rs

use alloc::alloc::{GlobalAlloc, Layout};

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // TODO 内存对齐和边界检查
        let alloc_start = self.next;
        self.next = alloc_start + layout.size();
        self.allocations += 1;
        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        todo!();
    }
}
```

首先，我们使用 `next` 字段作为分配的起始地址。然后我们将 `next` 字段更新为分配的结束地址，即堆上的下一个未使用地址。在返回分配起始地址的 `*mut u8` 指针之前，我们将 `allocations` 计数器加一。

注意，我们目前没有执行任何边界检查或是对齐调整，所以这个实现目前是不安全的。但这对我们的实现来说并不重要，因为它会编译失败并报告错误：


```
error[E0594]: cannot assign to `self.next` which is behind a `&` reference
  --> src/allocator/bump.rs:29:9
   |
29 |         self.next = alloc_start + layout.size();
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `self` is a `&` reference, so the data it refers to cannot be written
```

（同样的错误也会发生在 `self.allocations += 1` 行。这里为了简洁起见省略了它。）

出现这个错误是因为 `GlobalAlloc` 特征的 [`alloc`] 和 [`dealloc`] 方法只能在一个不可变的 `&self` 引用上操作，因此，更新 `next` 和 `allocations` 字段是不可能的。问题在于，每次分配时更新 `next` 字段正是bump分配器的核心机制。

[`alloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.alloc
[`dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc

#### `GlobalAlloc` 和可变性

在我们为可变性问题寻找可能的解决方案前，让我们先理解一下为什么 `GlobalAlloc` 特征的方法是用 `&self` 参数定义的：就像我们在[上一篇文章][global-allocator]中看到的那样，全局堆分配器是通过向实现 `GlobalAlloc` 特征的 `static` 变量上添加 `#[global_allocator]` 属性来定义的。静态变量是 Rust 中的不可变变量，所以无法在静态分配器上调用接受 `&mut self` 的方法。因此，`GlobalAlloc` 特征的所有方法都只接受不可变的 `&self` 引用。


[global-allocator]:  @/edition-2/posts/10-heap-allocation/index.md#the-global-allocator-attribute

幸运的是，有一种方法能从 `&self` 引用中获取一个 `&mut self` 引用：我们可以通过将分配器封装在 [`spin::Mutex`] 自旋锁中来实现同步的 [内部可变性][interior mutability] 。这个类型提供的 `lock` 方法能够执行 [互斥][mutual exclusion] ，从而安全地将 `&self` 引用转换为 `&mut self` 引用。我们已经在我们的内核中多次使用了这个封装器类型，例如用于 [VGA 文本缓冲区][vga-mutex] 。



[interior mutability]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
[vga-mutex]: @/edition-2/posts/03-vga-text-buffer/index.md#spinlocks
[`spin::Mutex`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html
[mutual exclusion]: https://en.wikipedia.org/wiki/Mutual_exclusion

#### `Locked` 封装类型

在 `spin::Mutex`封装类型的帮助下，我们可以为我们的bump分配器实现 `GlobalAlloc` 特征。诀窍是不直接在 `BumpAllocator` 上实现该特征，而是在 `spin::Mutex<BumpAllocator>` 类型实现。

```rust
unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {…}
```

不幸的是，这样还是不行，因为Rust编译器不允许为定义在其他crates中的类型实现特征。

```
error[E0117]: only traits defined in the current crate can be implemented for arbitrary types
  --> src/allocator/bump.rs:28:1
   |
28 | unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^--------------------------
   | |                           |
   | |                           `spin::mutex::Mutex` is not defined in the current crate
   | impl doesn't use only types from inside the current crate
   |
   = note: define and implement a trait or new type instead
```

为了解决这个问题，我们需要围绕 `spin::Mutex` 实现我们自己的包装器类型。

```rust
// in src/allocator.rs

/// 允许特征实现的围绕 `spin::Mutex` 类型的封装器。
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}
```

这个类型是围绕 `spin::Mutex<A>` 的泛型封装器。它不施加任何对封装类型 `A` 的限制，所以它可以用来封装所有种类的类型，而不仅仅是分配器。它提供了一个简单的 `new` 构造函数，用于封装给定的值。为了方便起见，它还提供了一个 `lock` 函数，用于调用封装的 `Mutex` 上的 `lock` 。由于 `Locked` 类型对于其他分配器实现也很有帮助，所以我们将它放在父 `allocator` 模块中。

#### `Locked<BumpAllocator>` 类型的实现

`Locked` 类型已在我们自己的crate中定义（而不是直接使用 `spin::Mutex`）。因此，可以使用它来为我们的bump分配器实现 `GlobalAlloc` 特征。完整的实现如下：


```rust
// in src/allocator/bump.rs

use super::{align_up, Locked};
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock(); // 获取可变引用

        let alloc_start = align_up(bump.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            ptr::null_mut() // 内存不足
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock(); // 获取可变引用

        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}
```

`alloc` 和 `dealloc` 的第一步都是调用 [`Mutex::lock`] 方法来通过 `inner` 字段获取封装类型的可变引用。封装实例在方法结束前保持锁定，因此不会在多线程上下文中发生数据竞争（我们很快会添加线程支持）。

[`Mutex::lock`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html#method.lock

与之前的原型相比，现在的 `alloc` 实现遵循了对齐要求并执行了边界检查，确保分配的内存区域在堆内存区域内。第一步是将 `next` 地址向上对齐到 `Layout` 参数指定的对齐值。稍后展示 `align_up` 函数的实现。接着，我们将所请求的分配大小加到 `alloc_start` 地址上，得到该次分配的结束地址。为了防止在大内存分配时发生整数溢出，我们使用了 [`checked_add`] 方法。如果发生溢出或分配结束地址大于堆结束地址，我们就返回一个空指针以表示内存不足情况。否则，我们更新 `next` 地址并像之前一样增加 `allocations` 计数器。最后，我们返回转换为 `*mut u8` 指针 `alloc_start` 地址。


[`checked_add`]: https://doc.rust-lang.org/std/primitive.usize.html#method.checked_add
[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html

`dealloc` 函数忽略了传入的指针和 `Layout` 参数。它仅仅是将 `allocations` 计数器减一。如果计数器再次变为 `0` ，则意味着所有分配都已再次释放。在这种情况下，它将 `next` 地址重置为 `heap_start` 地址，使整个堆内存重新可用。

#### 地址对齐

`align_up` 函数足够通用，因此我们可以将它放到父 `allocator` 模块中。其基本实现如下：

```rust
// in src/allocator.rs

/// 向上对齐给定地址 `addr` 到对齐值 `align`。
fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr // 地址已经对齐
    } else {
        addr - remainder + align
    }
}
```

这个函数首先计算 `addr` 除以 `align` 的[余数][remainder]。如果余数为 `0` ，则地址已经与给定的对齐值对齐。否则，我们通过减去余数（以便余数为 `0`）并加上对齐值（以便地址不小于原始地址）来对齐地址。


[remainder]: https://en.wikipedia.org/wiki/Euclidean_division

注意这不是实现此函数最高效的方法，一个更快的实现如下所示：

```rust
/// 向上对齐给定地址 `addr` 到对齐值 `align` 。 
///
/// 要求对齐值是2的幂
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
```

此方法要求 `align` 必须是2的幂，通过 `GlobalAlloc` 特征（及其 [`Layout`] 参数）可以保证这一点。这使得我们可以创建[位掩码][bitmask]来高效地对齐地址。为了理解其工作原理，我们从表达式的右侧逐步解析： 


[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html
[bitmask]: https://en.wikipedia.org/wiki/Mask_(computing)

- 因为 `align` 是2的幂，它的[二进制表示][binary representation]仅有一个比特位为1（例如：`0b000100000`）。这意味着 `align - 1` 在该比特位下的所有低位均为1（例如：`0b00011111`）。  
- 通过 `!` 运算符执行[按位取反][bitwise `NOT`]操作, 我们得到一个数，其除了低于 `align`的比特位为0外，其余位均为1。
- 通过将给定地址和 `!(align - 1)` 执行[按位与][bitwise `AND`]操作，我们将该地址 _向下_ 对齐。这是通过将所有低于 `align` 的比特位清除来实现的。
- 因为我们想要向上对齐而不是向下对齐，在执行按位 `AND` 操作之前，先将 `addr` 增加 `align - 1` 的值。这种方式下，已对齐的地址保持不变，而未对齐的地址将被对齐到下一个对齐边界。

[binary representation]: https://en.wikipedia.org/wiki/Binary_number#Representation
[bitwise `NOT`]: https://en.wikipedia.org/wiki/Bitwise_operation#NOT
[bitwise `AND`]: https://en.wikipedia.org/wiki/Bitwise_operation#AND

你选择使用哪一个变体，这取决于你。它们计算的结果相同，只是使用的方法不同。

### 用法
                                 
为了使用我们的bump分配器，我们需要更新 `allocator.rs` 中的 `ALLOCATOR` 静态变量：

```rust
// in src/allocator.rs

use bump::BumpAllocator;

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());
```

我们需要将 `BumpAllocator::new` 和 `Locked::new` 定义为 [`const` 函数][`const` functions] 。如果它们是一般的函数，将会发生编译错误，因为一个 `static` 变量的初始化表达式会在编译时求值。


[`const` functions]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

我们不需要修改我们的 `init_heap` 函数中的 `ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE)` 调用，因为bump分配器提供的接口与 `linked_list_allocator` 提供的接口是一致的。

现在我们的内核使用了我们的bump分配器！一切正常，包括我们在上一篇文章中创建的 [`heap_allocation` tests]：

[`heap_allocation` tests]: @/edition-2/posts/10-heap-allocation/index.md#adding-a-test

```
> cargo test --test heap_allocation
[…]
Running 3 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
```

### 讨论

bump分配最大的优势就是它非常快。相比其他的需要主动地寻找合适的内存块并且在 `alloc` 和 `dealloc` 时执行各种簿记工作的分配器设计（见下文），bump分配器 [可以对其进行优化][bump downwards] ，使其仅降至仅有几条汇编指令。这使得bump分配器在优化分配性能时非常有用，例如当创建一个 [虚拟 DOM 库][virtual DOM library] 时。


[bump downwards]: https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
[virtual DOM library]: https://hacks.mozilla.org/2019/03/fast-bump-allocated-virtual-doms-with-rust-and-wasm/

bump分配器通常不被用作全局分配器，但bump分配的原理通常以 [arena分配][arena allocation] 的形式应用，其核心思想是将独立的小块内存分配操作批量合并处理以提高性能。Rust 的一个arena分配器的例子包含在 [`toolshed`] crate 中。


[arena allocation]: https://mgravell.github.io/Pipelines.Sockets.Unofficial/docs/arenas.html
[`toolshed`]: https://docs.rs/toolshed/0.8.1/toolshed/index.html

#### bump分配器的缺点

bump分配器的主要限制是它只能在所有已分配的内存都已释放后才能重用已释放的内存。这意味着单个长期存在的分配就可以阻止内存重用。我们可以通过添加 `many_boxes` 测试的变体来看到这一点：

```rust
// in tests/heap_allocation.rs

#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1); // 新的
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1); // 新的
}
```

与 `many_boxes` 测试类似，此测试创建了大量的分配，以触发内存不足错误（如果分配器没有重用空闲的内存）。此外，该测试还创建了一个 `long_lived` 分配，它的生命周期贯穿整个循环执行过程。

当我们运行新的测试时，我们会看到它确实失败了：

```
> cargo test --test heap_allocation
Running 4 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [failed]

Error: panicked at 'allocation error: Layout { size_: 8, align_: 8 }', src/lib.rs:86:5
```

让我们试着理解为什么会发生此错误：首先，`long_lived` 分配在堆的起始位置被创建，然后 `allocations` 计数器增加1。对于在循环中的每一次迭代，一个分配会创建并在下一次迭代开始前被直接释放。这意味着 `allocations` 计数器在迭代的一开始短暂地增加为2并在迭代结束时减少为1。现在问题是bump分配器只有在 _所有_ 分配均被释放之后才能重用内存，例如，当 `allocations` 计数器变为0时。因为这在循环结束前不会发生，每次循环迭代分配一个新的内存区域，在一定次数迭代后将导致内存不足错误。

#### 解决测试问题？

有两个潜在的技巧可以用来解决我们bump分配器的测试问题：

- 我们可以更新 `dealloc` 方法，通过比较其结束地址与 `next` 指针来检查释放的分配是否与 `alloc` 返回的最后一个分配的结束地址相等。在相等的情况下，我们可以安全地将 `next` 指针恢复为已释放分配的起始地址。这样，每次循环迭代都可以重用相同的内存块。

- 我们可以添加一个 `alloc_back` 方法，该方法使用一个额外的 `next_back` 字段从堆的 _末尾_ 分配内存。然后我们可以为所有长生命周期的分配手动调用此分配方法，从而在堆上实现短生命周期和长生命周期的分配的分离。注意这种分离只有在清楚地知道每个分配会存活多久的前提下才能正常工作。此方法的另一个缺陷是手动进行内存分配是繁琐且不安全的。


虽然这两种方法都可以解决这个测试问题，但因为它们都只能在非常特殊的场景下重用内存，它们都不是通用的解决方案。问题是：存在一种通用的解决方案来重用 _所有_ 已释放的内存吗？                    

#### 重用所有已释放的内存？

从 [上一篇文章][heap-intro] 中我们知道，分配可以存活任意长的时间，也可以以任意顺序被释放。这意味着我们需要跟踪一个可能无界的不连续的未使用内存区域，如下图所示：

[heap-intro]: @/edition-2/posts/10-heap-allocation/index.md#dynamic-memory

![](allocation-fragmentation.svg)

这张图展示了堆随时间变化的情况。一开始，整个堆都是未使用的，`next` 地址等于 `heap_start`（第一行）。然后，第一次分配发生（第2行）。在第3行，分配了一个新的内存块并释放了第一个内存块。在第4行添加了更多的分配。其中半数分配是非常短暂的，在第5行已经被释放，此时还新增了一个新的分配。

第五行展示了根本性问题：我们有5个大小不同的未使用内存区域，但 `next` 指针只能指向最后一个区域的开头。虽然我们可以在这个例子中使用一个大小为4的数组来存储其他未使用内存区域的起始地址和大小，但这不是一个通用的解决方案，因为我们可以轻松创建一个使用8、16或1000个未使用内存区域的示例。

通常，当存在潜在无限数量的元素时，我们可以使用一个堆分配集合。这在我们的场景中是不可能的，因为堆分配器不能依赖于它自身（会造成无限递归或死锁）。因此我们需要寻找一种不同的解决方案。

## 链表分配器

在实现分配器时一个常用的跟踪任意数量的未使用内存区域的技巧是将未使用的内存区域本身用作后备存储。这利用了未使用区域仍然映射到虚拟地址并由物理帧支持，但存储的信息不再被需要这一事实。通过将有关已释放区域的信息存储在区域中，我们可以在不需要额外内存的情况下跟踪无限数量的已释放区域。

最常见的实现方法是在已释放的内存中构造一个单链表，每一个节点都是一个已释放的内存区域：

![](linked-list-allocation.svg)

每个链表节点有两个字段：内存区域的大小和指向下一个未使用内存区域的指针。通过这种方法，我们只需要一个指向第一个未使用区域（称为 `head` ）的指针就能跟踪所有未使用的区域而不管它们的数量多少。最终形成的数据结构通常被称为  [_free list_] 。

[_free list_]: https://en.wikipedia.org/wiki/Free_list

你能从这个名字中猜到，这就是 `linked_list_allocator` crate 中用到的技术。使用这种技术的分配器也常被称为 _池分配器_ 。

### 实现

接下来，我们会创建我们自己的简单的 `LinkedListAllocator` 类型，用于跟踪已释放的内存区域。本部分内容在后续章节中非必需，所以你可以根据自己的喜好跳过实现细节。

#### 分配器类型 {#allocator-type}

我们首先在一个新的 `allocator::linked_list` 子模块中创建一个私有的 `ListNode` 结构体：

```rust
// in src/allocator.rs

pub mod linked_list;
```

```rust
// in src/allocator/linked_list.rs

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}
```

正如图示所示，链表节点包含一个 `size` 字段和一个指向下一个节点的可选的指针，用 `Option<&'static mut ListNode>` 类型表示。`&'static mut` 类型的语义上描述了一个由指持有的所有权对象。本质上，它是一个缺少在作用域结束时释放对象的析构函数的 [`Box`]智能指针。

[owned]: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html
[`Box`]: https://doc.rust-lang.org/alloc/boxed/index.html

我们为 `ListNode` 实现以下方法：

```rust
// in src/allocator/linked_list.rs

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}
```

此类型包含一个名为 `new` 的构造函数，以及用于计算代表区域起始地址和结束地址的方法。我们将 `new` 函数定义为[常量函数][const function]，这一特性在后续构建静态链表分配器时是必需的。

[const function]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

通过将 `ListNode` 结构体作为基础组件，我们现在可以创建 `LinkedListAllocator` 结构体了：

```rust
// in src/allocator/linked_list.rs

pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    /// 创建一个空的LinkedListAllocator。
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// 用给定的堆边界初始化分配器
    ///
    /// 这个函数是不安全的，因为调用者必须保证给定的堆边界是有效的并且堆是未使用的。
    /// 此方法只能调用一次
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    /// 将给定的内存区域添加到链表前端。
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        todo!();
    }
}
```

此结构体包含一个指向第一个堆区域的 `head` 节点。我们只关注 `next` 指针的值，所以我们在 `ListNode::new` 函数中将 `size` 设置为0。将 `head` 定义为 `ListNode` 类型而不是 `&'static mut ListNode` 类型的优势在于，`alloc` 方法的实现会更简单。

和bump分配器一样，`new` 函数并未用堆边界初始化分配器。除了保持API兼容性外，这是因为初始化操作需要将链表节点写入堆内存，而这只能在运行时发生。但是，`new` 函数必须被定义为可以在编译期求值的[常量函数][const function]，因为该函数将用于初始化 `ALLOCATOR` 静态变量。出于这个原因，我们再次提供一个独立的非常量 `init` 方法。

[`const` function]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

`init` 方法使用一个 `add_free_region` 方法，该方法的实现会在稍后展示。现在，我们用 [`todo!`] 宏提供一个总是会触发panic的占位符实现。


[`todo!`]: https://doc.rust-lang.org/core/macro.todo.html

#### `add_free_region` 方法

`add_free_region` 方法提供链表的基础 _push_ 操作。我们目前只从 `init` 方法调用它，但它也会是我们 `dealloc` 实现的核心方法。记住，当再次释放已分配的内存区域时，会调用 `dealloc` 方法。为了跟踪此已释放的内存区域，我们希望将其推送到链表中。


`add_free_region` 方法的实现如下：

```rust
// in src/allocator/linked_list.rs

use super::align_up;
use core::mem;

impl LinkedListAllocator {
    /// 将给定的内存区域添加到链表前端。
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        /// 确保给定的内存区域足以存储 ListNode
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // 创建一个新的 ListNode 并将其添加到链表前端
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        unsafe {
            node_ptr.write(node);
            self.head.next = Some(&mut *node_ptr)
        }
    }
}
```

此方法接受一个内存区域的地址和大小作为参数并且将它添加到链表前端。首先，它会确保给定的内存区域是否满足存储 `ListNode` 的所需的最小大小和对齐要求。然后，它会通过以下步骤创建一个新的节点并将其插入链表中：

![](linked-list-allocator-push.svg)

步骤0展示了调用 `add_free_region` 方法之前的堆内存状态。在步骤1中，该方法以图中标记为 `freed` 的内存区域作为参数被调用。在初始检查之后，方法会在栈上创建一个新的 `node`，其大小与已释放的内存区域相同。随后，它使用[`Option::take`]方法将 `node` 的 `next` 指针设置为当前的 `head` 指针，从而将 `head` 指针重置为 `None` 。

[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

步骤2中，该方法通过 [`write`] 方法将这个新创建的 `node` 写入在空闲内存区域的开始部分。然后，它将 `head` 指针指向这个新节点。结果指针结构看起来有点混乱，因为总是将空闲区域插入到列表的开头，但如果我们跟随着指针，我们会看到每个空闲区域仍然可以从 `head` 指针到达。


[`write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write


#### `find_region` 方法

链表的第二个基础操作就是在链表中找到一个节点并移除它。这是实现 `alloc` 方法的中心操作，接下来我们将通过 `find_region` 方法来实现这个操作。

```rust
// in src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// 查找给定大小和对齐方式的空闲区域并将其从链表中移除。
    ///
    /// 返回一个包含链表节点和分配内存区域起始地址的元组。
    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut ListNode, usize)>
    {
        // 当前链表节点的引用，每次迭代更新
        let mut current = &mut self.head;
        // 在链表中查找合适大小的内存区域
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // 区域适用于分配 -> 从链表中移除该节点
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // 区域不适用 -> 继续下一个区域
                current = current.next.as_mut().unwrap();
            }
        }
        // 未找到合适的区域
        None
    }
}
```
此方法使用一个 `current` 变量和一个 [`while let` 循环] 来遍历链表元素。在开始时，`current` 被设置为（虚拟）`head` 节点。在每次迭代中，它都会被更新为当前节点的 `next` 字段（在 `else` 块中）。如果该区域适用于给定大小和对齐方式的分配，该区域会从链表中移除并与 `alloc_start` 地址一起返回。


[`while let` loop]: https://doc.rust-lang.org/reference/expressions/loop-expr.html#predicate-pattern-loops

当 `current.next` 指针变成 `None` 时，循环退出。这意味着我们遍历了整个链表，但没有找到合适的区域进行分配。在这种情况下，我们返回 `None`。内存区域是否合适是由 `alloc_from_region` 函数检查的，它的实现将在稍后展示。

让我们更详细地了解如何从链表中移除一个合适的内存区域：

![](linked-list-allocator-remove-region.svg)

步骤0展示了任何指针调整之前的状态。`region` 和 `current` 内存区域以及 `region.next` 和 `current.next` 指针都在图中被标记。在步骤1中，通过使用 [`Option::take`] 方法将 `region.next` 和 `current.next` 指针都重置为 `None` 。原指针的值被存储在名为 `next` 和 `ret` 的本地变量中。


步骤2中，`current.next` 指针被设置为本地的 `next` 指针，即原始的 `region.next` 指针。这样做的效果是 `current` 现在直接指向 `region` 后面的内存区域，因此 `region` 不再是链表中的节点。函数随后返回存储在本地 `ret` 变量中的指向 `region` 的指针。

##### `alloc_from_region` 函数

`alloc_from_region` 函数返回一个区域是否满足指定大小和对齐要求的分配需求。它的定义如下：

```rust
// in src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// 尝试将给定区域用于给定大小和对齐要求的分配。
    ///
    /// 成功时返回分配该内存区域的起始地址。
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
        -> Result<usize, ()>
    {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            // 区域太小
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            // 区域剩余部分太小，不足以存储 ListNode结构体（必须满足此条件，
            // 因为分配将区域分为已用和空闲部分）
            return Err(());
        }

        // 内存区域满足分配要求。
        Ok(alloc_start)
    }
}
```

首先，该函数使用我们之前定义的 `align_up` 函数和 [`checked_add`] 方法计算潜在分配的起始和结束地址。如果发生溢出或如果结束地址超出了该区域结束地址，分配就不适合该区域，因此我们将返回一个错误。


该函数随后执行一项并不显而易见的检查。这个检查是必要的，因为大部分情况分配请求无法完全适配某个内存区域，所以在分配之后，该区域仍剩余部分可用的内存空间。此剩余空间必须在分配之后能存储其自身的 `ListNode` ，所以它必须足够大才能这样做。该检查准确地验证了这一点：要么分配完全适配（`excess_size == 0`），要么剩余空间足以存储一个  `ListNode` 。

#### 实现 `GlobalAlloc`

有了在 `add_free_region` and `find_region` 方法中定义的基础操作，我们终于能实现 `GlobalAlloc` 特征了。和bump分配器一样，我们不会直接实现 `GlobalAlloc` 特征，而是为 `LinkedListAllocator` 类型实现 [`Locked` 包装器][`Locked` wrapper]。该包装器通过自旋锁添加内部可变性，这样我们就可以在 `alloc` 和 `dealloc` 方法仅获取到 `&self` 引用的情况下修改分配器实例。

[`Locked` wrapper]: @/edition-2/posts/11-allocator-designs/index.md#a-locked-wrapper-type

其实现如下：

```rust
// in src/allocator/linked_list.rs

use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // 执行布局调整
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                unsafe {
                    allocator.add_free_region(alloc_end, excess_size);
                }
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // 执行布局调整
        let (size, _) = LinkedListAllocator::size_align(layout);

        unsafe { self.lock().add_free_region(ptr as usize, size) }
    }
}
```

让我们从 `dealloc` 方法开始，因为它更简单：首先，该方法执行布局调整，我们将在稍后解释它。然后，该方法通过调用 [`Locked` 包装器][`Locked` wrapper]上的 [`Mutex::lock`] 函数获取一个 `&mut LinkedListAllocator` 引用。最后调用 `add_free_region` 函数将已释放的内存区域添加到空闲链表中。

`alloc` 函数稍有些复杂。它同样从布局调整开始，并且调用 [`Mutex::lock`] 函数来获取一个可变的分配器引用。然后，它调用 `find_region` 方法来查找一个适合分配的内存区域，并从空闲列表中删除该内存区域。如果此调用失败并返回 `None`，则该函数返回 `null_mut` 以表示错误，因为没有找到合适的内存区域。

在成功的场景下，`find_region` 方法返回一个包含适合分配的内存区域（不再在链表中）和分配起始地址的元组。通过 `alloc_start`、分配大小和区域结束地址，它重新计算分配结束地址和剩余空间大小。如果剩余空间大小不为零，则调用 `add_free_region` 将内存区域的剩余空间添加回空闲链表。最后，它将 `alloc_start` 地址转化为 `*mut u8` 指针返回。

#### 布局调整

我们在 `alloc` 和 `dealloc` 调用的布局调整究竟是什么呢？它确保每个已分配的块足以存储一个 `ListNode` 。这是很重要的，因为内存块会在某个时刻被释放，释放时我们会在块中写入一个 `ListNode` 。如果一个块的大小比 `ListNode` 还要小或者没有正确地对齐，将导致未定义的行为。

在 `size_align` 函数中执行的布局调整，其定义如下：
```rust
// in src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// 调整给定的内存布局，使最终分配的内存区域
    /// 足以存储一个 `ListNode` 。
    ///
    /// 将调整后的大小和对齐方式作为（size, align）元组返回。
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}
```

首先，该函数在传入的 [`Layout`] 上调用 [`align_to`] 方法将对齐方式提升至 `ListNode` 的对齐要求。然后，它使用 [`pad_to_align`] 方法将大小向上取整到对齐值的倍数，以确保下一个内存块的起始地址也有正确的对齐方式存储 `ListNode` 。最后，它使用 [`max`] 方法强制最小分配的大小至少为 `mem::size_of::<ListNode>` 。以确保 `dealloc` 函数可以安全地在已释放的内存块写入 `ListNode` 。


[`align_to`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align_to
[`pad_to_align`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.pad_to_align
[`max`]: https://doc.rust-lang.org/std/cmp/trait.Ord.html#method.max

### 用法

我们可以更新 `allocator` 模块中的 `ALLOCATOR` 静态变量，以使用我们的新 `LinkedListAllocator` ：

```rust
// in src/allocator.rs

use linked_list::LinkedListAllocator;

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> =
    Locked::new(LinkedListAllocator::new());
```

因为 `init` 函数在bump分配器和链表分配器的行为相同，所以我们不需要修改 `init_heap` 中的 `init` 调用。

当我们再次运行 `heap_allocation` 测试时，我们看到所有测试都通过了，包括使用bump分配器时失败的 `many_boxes_long_lived` 测试：

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

这表明我们的链表分配器可以重用已释放的内存，以满足后续的分配。

### 讨论

和bump分配器相比，链表分配器更适合作为一个通用分配器，主要是因为它可以直接重用已释放的内存。然而，它也有一些缺点，一部分是由于我们的基础实现所致，另一部分则是由于分配器设计本身的缺陷。

#### 合并已释放的内存块 {#merge-free-blocks}


我们的实现主要的问题就是它只将堆分成更小的内存块，但从不将它们合并到一起。考虑下面的例子：

![](linked-list-allocator-fragmentation-on-dealloc.svg)

在第一行中，我们在堆上创建了三个分配。其中两个分配在第二行被释放，第三行中释放了第三个分配。现在，整个堆再次变为未使用状态，但它被分成了四个独立的内存块。此时，没有一个块足够大，所以无法再创建一个大的分配。随着时间的推移，这个过程继续进行，堆被分成了越来越小的块。在某个时刻，堆已经变得如此碎片化，以至于即使是正常大小的分配也会失败。

为了解决这个问题，我们需要合并相邻的已释放内存块。对于上述示例，这意味着如下操作：

![](linked-list-allocator-merge-on-dealloc.svg)

和之前一样，在第二行中，两个分配被释放。我们现在在 `2a` 行中执行额外的一步来合并最右侧两个相邻的空闲块而不是保持堆碎片化。在第 `3` 行中，第三个分配也被释放（和之前一样），结果是整个未使用的堆被划分成三个独立的块。在第 `3a` 行中额外的合并步骤中，我们再次将三个相邻的块合并到一起。 

`linked_list_allocator` crate 通过如下方式实现这一合并策略：在 `deallocate` 调用中，它不会将已释放的内存块插入链表的头部，而是始终保持按起始地址排序维护链表。这样，在 `deallocate` 调用中就可以直接通过检查链表中相邻块的地址和大小来执行合并操作。当然，这样做会使释放操作变慢，但避免了我们上面看到的堆碎片化问题。

#### 性能表现

我们在之前了解到的，bump分配器的性能非常好，因为它只需要几个简单的汇编指令就可以完成。链表分配器的性能要差得多，因为一次分配或许需要遍历整个链表才能找到一个合适的内存块。



因为链表长度取决于未使用内存块的数量，不同程序的性能表现可能差异极大。对于仅创建少量分配的程序，分配性能相对较好。而对于因大量分配导致堆碎片化的程序，分配性能会非常差，因为链表会非常长，大部分内存块尺寸极小。

值得强调的是，相比于我们基础的实现而言，链表方法本身的缺陷才是造成性能问题的主要原因。因为在内核级代码中分配性能相当重要，所以我们将在下文中探索第三种通过降低内存使用率换取性能提升的分配器设计。

## 固定大小块分配器

接下来，我们展示一种使用固定大小的内存块来满足分配请求的分配器设计。使用这种方法，分配器往往会返回比实际需要更大的内存块，这将会由于 [内部碎片][internal fragmentation] 导致浪费内存，但它会显著减少寻找合适的内存块的时间（相比链表分配器而言），从而获得更好的分配性能。

### 介绍

_固定大小块分配器_ 背后的思想如下：我们不再精确分配请求所需的内存大小，而是定义一个固定的块大小列表，并且将每个分配向上取整为列表中的下一个内存块大小。例如，对于 16、64 和 512 的块大小，一个 4 字节的分配将返回一个 16 字节的块，一个 48 字节的分配将返回一个 64 字节的块，一个 128 字节的分配将返回一个 512 字节的块。


和链表分配器相同，我们通过在未使用的内存区域中创建链表来跟踪未使用的内存。然而，不再使用单一链表管理不同尺块大小的内存区域，而是为每个尺寸类别创建一个单独的链表。每个列表只存储相同大小的块。例如，对于块大小为 16、64 和 512 的情况，内存中会存在三个单独的链表：


![](fixed-size-block-example.svg).

不同于单个的 `head` 指针，我们现在有三个 `head` 指针 `head_16`、`head_64` 和 `head_512`，它们分别指向对应块大小的第一个未使用内存块。每个链表中的所有节点都具有相同的大小。例如，`head_16` 指针指向的链表只包含 16 字节的块。这意味着我们不再需要在每个链表节点中存储大小，因为它已经由头指针的名称指定。

因为链表中的每个节点都有相同的大小，所以每个节点都同样适合分配请求。这意味着我们可以使用以下步骤非常高效地执行分配操作：

- 将请求的分配大小向上取整为下一个块的大小。举例来说，当分配请求12字节时，按上述示例我们选择块大小为16
- 获取该链表的头指针，例如，对于块大小 16，我们需要使用 `head_16`。
- 移除该链表中的第一个块并返回它。

值得注意的是，我们只需要返回链表的第一个元素，不需要遍历整个链表。因此，分配性能相比于链表分配器要更好。

#### 块大小和浪费的内存

根据块大小的不同，向上取整时会浪费大量内存。举个例子，当一个512字节的块被分配给128字节的分配请求时，已分配内存的四分之三是未使用的。通过定义合理的块大小，限制浪费内存的大小是可能的。举例来说，我们使用2的幂（4，8，16，32，64，128，…）作为块大小时，在最差的情况下我们限制浪费内存的大小为已分配大小的一半，平均情况下是四分之一的已分配内存大小。

基于程序中常见的分配内存大小来优化块大小也是普遍做法。举例来说，如果程序中频繁分配24字节的内存时，我们可以额外添加24字节的块大小。这样做可以减少浪费的内存，但不会影响性能。

#### 内存释放

和内存分配类似，内存释放也非常高效。它包括以下步骤：

- 将需要释放的块的大小取整到下一个块大小，这是必需的，因为编译器只将请求的大小传入 `dealloc` ，而不是 `alloc` 返回的块大小。通过使用在 `alloc` 中 `dealloc` 中相同的尺寸调整函数，我们能确保释放了正确的内存大小。
- 获取链表的头指针
- 通过更新头指针将已释放的块放到链表头部

值得注意的是，释放内存时不需要遍历链表。这意味着释放内存的时间与链表的长度无关。

#### 后备分配器

考虑到大尺寸内存分配（ >2&nbsp;KB ）较少出现，尤其是在操作系统内核中，因此将这些分配回退到不同的分配器是有意义的。例如，我们可以将大于2048字节的分配回退到链表分配器，以减少内存浪费。由于预期这种大小的分配很少，链表规模会保持较小，分配和释放操作的性能也较好。

#### 创建新块 {#create-new-block}


以上的叙述中，我们一直假定有足够的特定大小的未使用块可供分配。然而，在某个特定的块大小的链表为空时，我们有两种方法可以创建新的未使用的特定大小的块来满足分配请求：

- 从后备分配器分配一个新块（如果有的话）
- 从不同的链表中分配一个更大的块。如果块大小是2的幂，这种方法效果最好。例如，一个32字节的块可以被分成两个16字节的块。


对于我们的实现，我们将从后备分配器分配新的块，因为实现起来要简单得多。

### 实现

现在我们知道一个固定大小块分配器是如何工作的，我们可以开始我们的实现。我们将不依赖于上一节中创建的链表分配器的实现，因此即使你跳过了链表分配器的实现部分，也可以继续跟随本节内容。


#### 链表节点

我们通过在一个新的 `allocator::fixed_size_block` 模块中创建一个 `ListNode` 类型开始我们的实现：

```rust
// in src/allocator.rs

pub mod fixed_size_block;
```

```rust
// in src/allocator/fixed_size_block.rs

struct ListNode {
    next: Option<&'static mut ListNode>,
}
```

这个类型和我们 [链表分配器实现][linked list allocator implementation] 中的 `ListNode` 类型类似，不同之处在于我们没有 `size` 字段。该字段在固定大小块分配器设计中不需要，因为每个链表中的块都有相同的大小。

[linked list allocator implementation]: #allocator-type

#### 块大小

接下来，我们定义一个常量 `BLOCK_SIZES` 切片，其中包含我们在实现中使用的块大小：

```rust
// in src/allocator/fixed_size_block.rs

/// 要使用的块大小
///
/// 各块大小必须为2的幂，因为它们同时被
/// 用作块内存对齐（对齐方式必须始终为2的幂）
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];
```

我们将使用从8到2048的2的幂作为块大小。我们不定义任何小于8的块大小，因为每个块在释放时都必须能够存储一个指向下一个块的64位指针。对于大于2048字节的分配，我们将回退到链表分配器。

为了简化实现，我们将块的大小定义为其在内存中所需的对齐方式。因此，一个16字节的块始终对齐在16字节边界，一个512字节的块始终对齐512字节边界。由于对齐方式必须始终是2的幂，这意味着任何其他块大小都是无效的。如果我们在未来需要非2的幂的块大小，我们可以调整我们的实现来支持（例如，通过定义一个 `BLOCK_ALIGNMENTS` 数组）。

#### 分配器类型

有了 `ListNode` 类型和 `BLOCK_SIZES` 切片，我们现在可以定义我们的分配器类型：

```rust
// in src/allocator/fixed_size_block.rs

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}
```

`list_heads` 字段是一个 `head` 指针的数组，一个指针对应一个块大小。数组的长度通过 `BLOCK_SIZES` 切片的  `len()` 确定。我们使用 `linked_list_allocator` 作为分配请求大小大于最大的块大小时的后备分配器。我们也可以使用我们自己实现的 `LinkedListAllocator` 。但是它的缺点在于不能 [合并空闲块][merge freed blocks] 。   

[merge freed blocks]: #merge-free-blocks

为了构造一个 `FixedSizeBlockAllocator`，我们提供与我们为其他分配器类型实现的相同的 `new` 和 `init` 函数：

```rust
// in src/allocator/fixed_size_block.rs

impl FixedSizeBlockAllocator {
    /// 创建一个空的FixedSizeBlockAllocator。
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// 用给定的堆边界初始化分配器
    ///
    /// 此函数是不安全的，因为调用者必须保证给定的堆边界是有效的且堆是
    /// 未使用的。此方法只能调用一次。
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe { self.fallback_allocator.init(heap_start, heap_size); }
    }
}
```

`new` 函数只是用空节点初始化 `list_heads` 数组，并创建一个 [`empty`] 链表分配器作为 `fallback_allocator` 。`EMPTY` 常量是为了告诉 Rust 编译器我们希望使用常量值初始化数组。直接初始化数组为 `[None; BLOCK_SIZES.len()]` 不起作用，因为编译器会要求 `Option<&'static mut ListNode>` 实现 `Copy` 特征，而但该类型并未实现。这是 Rust 编译器的当前限制，将来可能会改进。

[`empty`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.empty

不安全的 `init` 函数只调用 `fallback_allocator` 的 [`init`] 函数，而不做 `list_heads` 数组的任何额外初始化。相反，我们将在 `alloc` 和 `dealloc` 调用时惰性初始化列表。


[`init`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.init

为了方便起见，我们还创建了一个私有的 `fallback_alloc` 方法来使用 `fallback_allocator` 进行分配：

```rust
// in src/allocator/fixed_size_block.rs

use alloc::alloc::Layout;
use core::ptr;

impl FixedSizeBlockAllocator {
    /// 使用后备分配器分配
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}
```

`linked_list_allocator` crate的 [`Heap`] 类型未实现 [`GlobalAlloc`]（因为它[没有锁机制是不可能的]）。取而代之的是，它提供了一个 [`allocate_first_fit`] 方法，它的接口略有不同。与返回 `*mut u8` 和使用空指针来表示错误不同，它返回一个 `Result<NonNull<u8>, ()>` 。`NonNull` 类型是对保证非空指针的原始指针的抽象。通过将 `Ok` 分支映射到 [`NonNull::as_ptr`] 方法，将 `Err` 映射到空指针，我们可以很轻松地将其转换回 `*mut u8`  类型。

[`Heap`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html
[not possible without locking]: #globalalloc-and-mutability
[`allocate_first_fit`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.allocate_first_fit
[`NonNull`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html
[`NonNull::as_ptr`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html#method.as_ptr

#### 计算列表索引

在我们实现 `GlobalAlloc` 特征之前，我们定义一个 `list_index` 辅助函数，它返回给定 [`Layout`] 的最小可能块大小：

```rust
// in src/allocator/fixed_size_block.rs

/// 为给定布局选择适当的块大小
///
/// 返回 `BLOCK_SIZES` 数组中的索引
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}
```

块大小必须满足给定 `Layout` 的最小大小和对齐要求。由于我们定义了块大小即其对齐方式，这意味着 `required_block_size` 是布局的 [`size()`] 和 [`align()`] 属性的 [最大值]。为了在 `BLOCK_SIZES` 切片中找到下一个更大的块，我们首先使用 [`iter()`] 方法获取迭代器，然后使用 [`position()`] 方法找到第一个大于等于 `required_block_size` 的块的索引。

[maximum]: https://doc.rust-lang.org/core/cmp/trait.Ord.html#method.max
[`size()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.size
[`align()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align
[`iter()`]: https://doc.rust-lang.org/std/primitive.slice.html#method.iter
[`position()`]:  https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.position

注意我们不返回块大小本身，而是返回 `BLOCK_SIZES` 切片的索引。这是因为我们希望将返回的索引用作 `list_heads` 数组的索引。

#### 实现 `GlobalAlloc`

最后一步是实现 `GlobalAlloc` 特征：

```rust
// in src/allocator/fixed_size_block.rs

use super::Locked;
use alloc::alloc::GlobalAlloc;

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        todo!();
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        todo!();
    }
}
```

和其他分配器类似，我们不会直接为我们的分配器类型实现 `GlobalAlloc` 特征，而是使用 [`Locked` 包装器][`Locked` wrapper] 来添加同步的内部可变性。由于 `alloc` 和 `dealloc` 实现相对较长，我们接下来逐一介绍。

[`Locked` wrapper]: https://docs.rs/linked-list-allocator/0.9.0/linked_list_allocator/struct.Locked.html

##### `alloc`

`alloc` 方法的实现如下

```rust
// in `impl` block in src/allocator/fixed_size_block.rs

unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            match allocator.list_heads[index].take() {
                Some(node) => {
                    allocator.list_heads[index] = node.next.take();
                    node as *mut ListNode as *mut u8
                }
                None => {
                    // 没有块存在于列表中 => 分配新块
                    let block_size = BLOCK_SIZES[index];
                    // 只有当所有块大小都是 2 的幂时才有效
                    let block_align = block_size;
                    let layout = Layout::from_size_align(block_size, block_align)
                        .unwrap();
                    allocator.fallback_alloc(layout)
                }
            }
        }
        None => allocator.fallback_alloc(layout),
    }
}
```

我们逐步来看

首先，我们使用 `Locked::lock` 方法来获取对被包装的分配器实例的可变引用。接下来，我们调用刚刚定义的 `list_index` 函数来为给定布局计算合适的块大小，并获取其在 `list_heads` 数组中对应的索引。如果该索引为 `None`，表示没有适合分配的块大小，因此我们调用 `fallback_alloc` 函数来调用 `fallback_allocator`。

如果列表索引为 `Some` ，我们尝试使用 [`Option::take`] 方法从对应列表的开头移除第一个节点。如果列表不为空，我们进入 `Some(node)` 分支，其中我们将列表头指针指向弹出节点的后继节点（再次使用 [`take`][`Option::take`]）。最后，我们将弹出节点指针转换为 `*mut u8` 类型返回。


[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

如果链表头是 `None`，则表明该尺寸的内存块链表为空。这意味着我们需要像[上文](#create-new-block)中描述的那样构造一个新块。为此，我们首先从 `BLOCK_SIZES` 切片中获取当前块大小，并将其作为新块的大小和对齐方式。然后我们基于此大小和对齐方式创建一个新的 `Layout` 并调用 `fallback_alloc` 方法执行分配。调整布局和对齐的原因是确保内存块将在释放时能被正确地添加到对应的块列表中。



#### `dealloc`

`dealloc` 方法的实现如下：

```rust
// in src/allocator/fixed_size_block.rs

use core::{mem, ptr::NonNull};

// 在 `unsafe impl GlobalAlloc` 代码块中

unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            let new_node = ListNode {
                next: allocator.list_heads[index].take(),
            };
            // 验证块是否满足存储节点所需的大小和对齐方式要求
            assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
            assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
            let new_node_ptr = ptr as *mut ListNode;
            unsafe {
                new_node_ptr.write(new_node);
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
        }
        None => {
            let ptr = NonNull::new(ptr).unwrap();
            unsafe {
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}
```

和 `alloc` 方法类似，我们首先使用 `lock` 方法获取一个可变的分配器引用，接着调用 `list_index` 函数获取给定 `Layout` 的对应的块列表。如果索引为 `None` ，在 `BLOCK_SIZES` 中没有匹配的块大小，说明此分配是由后备分配器分配的。因此我们使用它的 [`deallocate`][`Heap::deallocate`] 方法来重新释放内存。该方法期望接收 [`NonNull`] 而不是 `*mut u8` ，因此我们需要转换指针。（ `unwrap` 调用尽在指针为空时失败，而当编译器调用 `dealloc` 这种请狂永远不会发生。） 


[`Heap::deallocate`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.deallocate

如果 `list_index` 返回一个块索引，我们需要将已释放的内存块添加到链表中。为此，我们首先创建一个新的 `ListNode`，它指向当前列表头（通过再次调用 [`Option::take`]）。在将新节点写入已释放的内存块之前，我们首先断言当前块大小由 `index` 指定的大小和对齐方式对于存储 `ListNode` 是足够的。然后，我们通过将给定的 `*mut u8` 指针转换为 `*mut ListNode` 指针，然后在其上调用不安全的 [`write`][`pointer::write`] 方法来执行写入。最后一步是将列表头指针设置为我们刚刚写入的 `ListNode`。为此，我们将原始的 `new_node_ptr` 转换为可变引用。


[`pointer::write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write

还有一些需要注意的事项：

- 我们不区分从块列表中分配的块和从后备分配器中分配的块。这意味着在 `alloc` 中创建的新块会在调用 `dealloc` 时会被添加到相应的块列表中，从而增加该大小的块数量。
- 在我们的实现中，`alloc` 方法是唯一可以创建新块的地方，这意味着初始时我们的块链表均为空，仅当请求对应尺寸的分配时，这些链表才会懒加载。
- 在 `alloc` 和 `dealloc` 中，我们无需显式使用 `unsafe` 代码块，即使我们做了一些 `unsafe` 操作。原因是rust将整个不安全的函数体视作一个大的 `unsafe` 代码块。由于使用显式的 `unsafe` 代码块可有一个优势即可以清楚地知道哪些操作是不安全的，哪些是安全的， 已有 [提议的RFC](https://github.com/rust-lang/rfcs/pull/2585) 要求修改此行为。



### 用法

为了使用我们新的 `FixedSizeBlockAllocator`，我们需要更新 `allocator` 模块中的 `ALLOCATOR` 静态变量：

```rust
// in src/allocator.rs

use fixed_size_block::FixedSizeBlockAllocator;

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(
    FixedSizeBlockAllocator::new());
```

因为我们的 `init` 函数对于我们实现的所有分配器都具有相同的行为，所以我们不需要修改 `init_heap` 中的 `init` 调用。

当我们再次运行 `heap_allocation` 测试时，所有测试都仍然是全部通过：

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

我们的分配器似乎运行正常！

### 讨论

尽管固定大小块分配器相比于链表分配器有更好的性能，但当使用2的幂作为块大小时，它会浪费一半的内存。这个取舍是否值得取决于应用的类型。对于操作系统内核来说，性能是至关重要的，因此固定大小块分配器看起来是更好的选择。

从实现角度说，我们现有的实现还有一些地方可以提升

- 相较于使用后备分配器懒分配内存块，更好的做法是预填块列表来提高初始分配的性能。

- 为了简化实现，我们将块大小限制为2的幂，一便将它们用作块对齐方式。若通过其他方式存储（或计算）块对齐方式，我们可以添加更多块大小，如常见分配尺寸，以减少内存浪费。
- 我们目前仅创建新块，但从不再次释放它们。这导致了内存碎片，最终可能导致大尺寸内存分配失败。可能有必要为每个块大小设置最大列表长度。当达到最大长度时，后续的释放操作将使用后备分配器而不是添加到列表中。
- 相比于回退到链表分配器，我们也可以有一个专门的分配器用于大于4&nbsp;KiB的分配。其基本思想是利用 [paging] ，它在4&nbsp;KiB页面上操作，将连续的虚拟内存映射到非连续的物理帧。这样，对于大型分配，未使用内存的碎片问题不再是问题。
- 有了这样的页分配器，我们就可以添加大于4&nbsp;KiB的块大小，同时完全放弃链表分配器。这样做的主要优势是减少碎片，提高性能可预测性，即更好的最坏情况性能。


[paging]: @/edition-2/posts/08-paging-introduction/index.md

需要注意的是以上提到的改进仅为建议。在操作系统内核中使用的分配器通常都针对特定工作负载进行了高度优化，而这能只有通过广泛的性能分析才能实现。

### 变体

固定大小块分配器还有许多变体。两个广泛应用的例子是 _slab分配器_ 和 _伙伴分配器_，它们也被用于Linux等流行的内核中。下面我们将简单介绍这两种设计。

#### Slab分配器

[slab分配器][slab allocator] 的核心思想是使用与内核中选择的类型直接对应的块大小。这样，这些类型的分配精确匹配块大小，没有浪费任何内存。有时，甚至可能预先初始化未使用块中的类型实例，以进一步提高性能。

[slab allocator]: https://en.wikipedia.org/wiki/Slab_allocation

Slab分配器常和其他分配器组合使用。举个例子，它可以和一个固定大小块分配器一起使用，对已分配的内存块进一步细分以减少内存浪费。它还常被用来在单次大块分配上实现 [对象池模式][object pool pattern] 。

[object pool pattern]: https://en.wikipedia.org/wiki/Object_pool_pattern

#### 伙伴分配器

[伙伴分配器][buddy allocator] 使用一个 [二叉树][binary tree] 数据结构而不是链表来管理空闲块，并使用2的幂作为块大小。当需要一个特定大小的块时，它会将一个更大的块拆成两半，从而在树中创建两个子节点。当一个块再次被释放时，会检查它在树上的相邻块。如果相邻块也是空闲的，那么这两个块就会合并为一个双倍尺寸的块。

合并过程的优势在于减少了 [内部碎片][internal fragmentation] ，因此小的空闲块也能被一个大的分配重用。同时它也不需要一个后备分配器，因此性能更容易预测。然而，伙伴分配器只支持2的幂作为块大小，这会因为 [内部碎片][internal fragmentation] 问题导致浪费大量内存。因此，伙伴分配器通常与slab分配器结合使用，进一步将分配的块拆分成多个较小的块。

[buddy allocator]: https://en.wikipedia.org/wiki/Buddy_memory_allocation
[binary tree]: https://en.wikipedia.org/wiki/Binary_tree
[external fragmentation]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#External_fragmentation
[internal fragmentation]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#Internal_fragmentation


## 总结

这篇文章介绍了不同的分配器设计。我们学习了如何实现一个基本的 [bump分配器][bump allocator] ，它通过增加一个 `next` 指针线性地分配内存。虽然这种分配很快，但只有在所有分配都被释放后才能重用内存。因此，它很少被用作全局分配器。

[bump allocator]: @/edition-2/posts/11-allocator-designs/index.md#bump-allocator

接着，我们创建了一个 [链表分配器][linked list allocator] ，它使用空闲的内存块本身来创建一个链表，称为 [空闲链表][free list] 。这个链表使我们能够存储不同大小的任意数量的空闲块。虽然没有发生内存浪费，但这种方法的性能较差，因为分配请求可能需要遍历整个列表。我们的实现也因为没有合并相邻的空闲块而存在 [外部碎片][external fragmentation] 问题。

[linked list allocator]: @/edition-2/posts/11-allocator-designs/index.md#linked-list-allocator
[free list]: https://en.wikipedia.org/wiki/Free_list

为了解决链表方法的性能问题，我们创建了一个 [固定大小块分配器][fixed-size block allocator] ，它预先定义了一组固定的块大小。对于每个块大小，都存在一个单独的 [空闲链表][free list] ，以便分配和释放只需要在列表的头部插入/弹出，因此它非常快。由于每个分配都被舍入到下一个更大的块大小，因此由于 [内部碎片][internal fragmentation] 而导致浪费了一些内存。然而，这种方法对于大部分分配来说是快速的，并且内存浪费对于大部分用例来说是可接受的。

为了解决链表方法的性能问题，我们创建了一个预定义了固定块大小的 [固定大小块分配器][fixed-size block allocator] 。对于每个块大小，都存在一个单独的 [空闲链表][free list] ，以便分配和释放操作只需要在列表的前面插入/弹出，因此非常快。由于每个分配都被向上取整到下一个更大的块大小，因此由于 [内部碎片][internal fragmentation] 而导致浪费了一些内存。

[fixed-size block allocator]: @/edition-2/posts/11-allocator-designs/index.md#fixed-size-block-allocator


分配器设计还存在多种权衡方案。[Slab分配][Slab allocation] 适用于优化常见固定大小结构的分配，但它并不适用于所有场景。[伙伴分配][Buddy allocation] 使用二叉树实现空闲块的合并，但由于只支持2的幂作为块大小，因此浪费了大量内存。还要记住的是，每个内核实现都有一个独特的工作负载，所以没有适合所有场景的“最佳”分配器设计。


[Slab allocation]: @/edition-2/posts/11-allocator-designs/index.md#slab-allocator
[Buddy allocation]: @/edition-2/posts/11-allocator-designs/index.md#buddy-allocator


## 下篇预告

通过本文，我们暂时完成了我们内存管理的实现。在下一篇文章中，我们将开始探索 [_多任务处理_][_multitasking_] ，首先从 [_async/await_] 的形式开始协作式多任务处理。随后的文章，我们将探讨 [_线程_][_threads_] 、[_多处理_][_multiprocessing_] 和 [_进程_][_processes_] 。

[_multitasking_]: https://en.wikipedia.org/wiki/Computer_multitasking
[_threads_]: https://en.wikipedia.org/wiki/Thread_(computing)
[_processes_]: https://en.wikipedia.org/wiki/Process_(computing)
[_multiprocessing_]: https://en.wikipedia.org/wiki/Multiprocessing
[_async/await_]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html
