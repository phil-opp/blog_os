+++
title = "アロケータの設計"
weight = 11
path = "allocator-designs/ja"
date = 2020-01-20

[extra]
# Please update this when updating the translation
translation_based_on_commit = "2e3230eca2275226ec33c2dfe7f98f2f4b9a48b4"
# GitHub usernames of the people that translated this post
translators = ["swnakamura"]
+++

この記事ではヒープアロケータをゼロから実装する方法を説明します。バンプアロケータ、連結リストアロケータ、固定サイズブロックアロケータなどの様々なアロケータの設計を示し、それらについて議論します。3つそれぞれのデザインについて、私たちのカーネルに使える基礎的な実装を作ります。

<!-- more -->

このブログの内容は [GitHub] 上で公開・開発されています。何か問題や質問などがあれば issue をたててください (訳注: リンクは原文(英語)のものになります)。また[こちら][at the bottom]にコメントを残すこともできます。この記事のソースコード全体は[`post-11` ブランチ][post branch]にあります。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-11

<!-- toc -->

## はじめに

[前回の記事][previous post]では、カーネルへのヒープ割り当ての基本的なサポートを追加しました。そのために、ページテーブルに[新しいメモリ領域を作成][map-heap]し、[`linked_list_allocator`クレートを使用][use-alloc-crate]してそのメモリを管理しました。ヒープは動作するようになりましたが、このアロケータクレートがどのように動作しているのかを理解しようとすることなく、仕事のほとんどを任せてしまっていました。

[previous post]: @/edition-2/posts/10-heap-allocation/index.ja.md
[map-heap]: @/edition-2/posts/10-heap-allocation/index.ja.md#creating-a-kernel-heap
[use-alloc-crate]: @/edition-2/posts/10-heap-allocation/index.ja.md#aroketakuretowoshi-u

この記事では、既存のアロケータクレートに頼るのではなく、独自のヒープアロケータをゼロから作成する方法を紹介します。単純無比の**バンプアロケータ**、基本の**固定サイズブロックアロケータ**など、さまざまなアロケータの設計について議論し、この知識を使用して（`linked_list_allocator`クレートと比較して）より性能のよいアロケータを実装します。

### 設計目標

アロケータの責任は、利用可能なヒープメモリを管理することです。`alloc`が呼ばれたら未使用のメモリを返し、`dealloc`によって解放されたメモリが再利用できるように記録をとる必要があります。最も重要なことは、すでに他の場所で使用されているメモリを決して渡してはならないということです。これをすると未定義動作が起きてしまいます。

メモリの正しい管理のほかにも、多くの二次的な設計目標があります。たとえば、アロケータは利用可能なメモリを効果的に利用し、[**断片化**][_fragmentation_]があまり起きないようにすべきです。さらに、並列なアプリケーションにもうまく機能し、任意の数のプロセッサに拡張できなくてはなりません。性能を最大化するため、CPUキャッシュに合わせてメモリレイアウトを最適化し、[キャッシュの局所性][cache locality]を改善したり[false sharing]を回避することすらするかもしれません。

[cache locality]: https://www.geeksforgeeks.org/locality-of-reference-and-cache-operation-in-cache-memory/
[_fragmentation_]: https://en.wikipedia.org/wiki/Fragmentation_(computing)
[false sharing]: https://mechanical-sympathy.blogspot.de/2011/07/false-sharing.html

これらの要件により、優れたアロケータは非常に複雑になりえます。例えば、[jemalloc]には3万行以上のコードがあります。ここまで複雑なものは、たった一つのバグが深刻なセキュリティ脆弱性につながりうるカーネルコードでは望ましくない場合が多いでしょう。幸いなことに、カーネルのコードにおけるメモリ割り当てのパターンは、ユーザースペースのコードと比較してはるかに単純であることが多いため、比較的単純なアロケータ設計で十分です。

[jemalloc]: http://jemalloc.net/

以下では、3つのカーネルアロケータの設計を示し、その長所と短所を説明します。

## バンプアロケータ

最も単純なアロケータの設計は**バンプアロケータ**（**スタックアロケータ**とも呼ばれる）です。メモリを直線的に割り当て、割り当てられたバイト数と割り当ての数のみを管理します。このアロケータは非常に特定のユースケースでのみ有用です──なぜなら、一度にすべてのメモリを解放することしかできないという厳しい制約があるからです。

### 考え方

バンプアロケータの考え方は、未使用のメモリの開始位置を指す`next`変数を増やす（"bump" する）ことによって、メモリを順に割り当てるというものです。はじめ、`next`はヒープの開始アドレスに等しいです。`next`は、各割り当てにおいて割り当てサイズだけ増加し、この値が使用済みメモリと未使用メモリの境界を常に指すようにします。

![3つの時点におけるヒープメモリ領域：
 1: ヒープの開始地点に一つの割り当てが存在する。`next`ポインタはその終端を指している。
 2: 二つ目の割り当てが一つ目のすぐ右に追加された。`next`ポインタは二つ目の割り当ての終端を指している。
 3: 三つ目の割り当てが二つ目のすぐ右に追加された。`next`ポインタは三つ目の割り当ての終端を指している。](bump-allocation.svg)

`next`ポインタは1つの方向にしか移動しないため、同じメモリ領域を2回渡すことはありません。これがヒープの終わりに達すると、それ以上のメモリを割り当てることができないので、次の割り当てでメモリ不足エラーが発生します。

多くの場合、バンプアロケータは「割り当てカウンタ」付きで実装されます。これは、`alloc`の呼び出しのたび1増加し、`dealloc`の呼び出しのたび1減少します。割り当てカウンタがゼロになることは、ヒープ上のすべての割り当てが解除されたことを意味します。このとき、`next`ポインタをヒープの開始アドレスにリセットし、ヒープメモリ全体を再び割り当てに使えるようにすることができます。

### 実装

`allocator::bump`サブモジュールを宣言するところから実装を始めましょう：

```rust
// in src/allocator.rs

pub mod bump;
```

サブモジュールの内容は、新しい`src/allocator/bump.rs`ファイルに、以下の内容で作ります：

```rust
// in src/allocator/bump.rs

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    /// 新しい空のバンプアロケータを作る。
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    /// 与えられたヒープ領域でバンプアロケータを初期化する。
    ///
    /// このメソッドはunsafeである。呼び出し元は与えられたメモリ範囲が未使用であることを
    /// 保証しなければならない。また、このメソッドは一度しか呼ばれてはならない。
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}
```

`heap_start`フィールドと`heap_end`フィールドは、ヒープメモリ領域の下限と上限を管理します。呼び出し元は、これらのアドレスが有効であることを保証する必要があります。そうでない場合、アロケータは不正なメモリを返すでしょう。このため、`init`関数の呼び出しは`unsafe`でなければなりません。

`next`フィールドの目的は、常にヒープの最初の未使用バイト、つまり次の割り当ての開始アドレスを指すことです。最初はヒープ全体が未使用であるため、`init`関数では`heap_start`に設定されています。各割り当てで、このフィールドは割り当てサイズだけ増加（"bump"）し、同じメモリ領域を2回返さないようにします。

`allocations`フィールドは、有効な割り当ての単純なカウンタで、最後の割り当てが解放されたときにアロケータをリセットするためにあります。0で初期化します。

インターフェイスを`linked_list_allocator`クレートによって提供されるアロケータと同じにするために、初期化を`new`関数の中で直接実行するのではなく、別の`init`関数を作りました。こうすることで、コードの変更なしにアロケータを切り替えることができます。

### `GlobalAlloc`を実装する

[前回の記事で説明した][global-alloc]ように、すべてのヒープアロケータは、次のように定義されている[`GlobalAlloc`]トレイトを実装する必要があります：

[global-alloc]: @/edition-2/posts/10-heap-allocation/index.ja.md#aroketaintahuesu
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

必要なのは`alloc`と`dealloc`メソッドのみです。他の2つのメソッドにはデフォルト実装があるので省略できます。

#### 最初の実装

`BumpAllocator`の`alloc`メソッドを実装してみましょう。

```rust
// in src/allocator/bump.rs

use alloc::alloc::{GlobalAlloc, Layout};

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // TODO アラインメント・境界のチェック
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

まず、割り当ての開始アドレスとして`next`フィールドを使用します。次に、割り当ての終端アドレス（ヒープの次の未使用アドレスでもある）を指すように`next`フィールドを更新します。`allocations`カウンタを1増やしてから、割り当ての開始アドレスを`*mut u8`ポインタとして返します。

境界チェックやアラインメント調整を行わないので、この実装はまだ安全ではないことに注意してください。まあいずれにせよ、以下のエラーでコンパイルに失敗するのでたいした問題ではないのですが：

```
error[E0594]: cannot assign to `self.next` which is behind a `&` reference
  --> src/allocator/bump.rs:29:9
   |
29 |         self.next = alloc_start + layout.size();
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `self` is a `&` reference, so the data it refers to cannot be written
```

（`self.allocations += 1`の行でも同じエラーが発生します。簡潔のためにここでは省略しました）

このエラーが起こるのは、`GlobalAlloc`トレイトの[`alloc`]および[`dealloc`]メソッドが不変な`&self`参照に対してのみ動作するため、`next`フィールドと`allocations`フィールドを更新できないために発生します。割り当てで毎回`next`を更新することがバンプアロケータの大原則であるため、これは問題ですね。

[`alloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.alloc
[`dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc

#### `GlobalAlloc`と可変性

この可変性の問題にどんな解決策が可能かを見る前に、`GlobalAlloc`トレイトメソッドがなぜ`&self`引数で定義されているのかを考えてみましょう。[前回の記事][global-allocator]で見たように、グローバルヒープアロケータは`GlobalAlloc`トレイトを実装する`static`に`#[global_allocator]`属性を追加することによって定義されます。<ruby>静的<rp> (</rp><rt>スタティック</rt><rp>) </rp></ruby>変数はRustでは不変であるため、この静的なアロケータで`&mut self`を取るメソッドを呼び出すことはできません。よって、`GlobalAlloc`のすべてのメソッドは、不変な`&self`参照のみを取ります。

[global-allocator]:  @/edition-2/posts/10-heap-allocation/index.ja.md#global-allocator-shu-xing

幸いなことに、`&self`参照から`&mut self`参照を取得する方法があります。アロケータを[`spin::Mutex`]スピンロックでラップすることで、同期された[内部可変性][interior mutability]を使えるのです。この型は、[相互排他制御][mutual exclusion]を行う`lock`メソッドを提供し、`&self`参照を`&mut self`参照に安全に変換します。このラッパ型はカーネルですでに複数回使用しています（[VGAテキストバッファ][vga-mutex]など）。

[interior mutability]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
[vga-mutex]: @/edition-2/posts/03-vga-text-buffer/index.ja.md#supinrotuku
[`spin::Mutex`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html
[mutual exclusion]: https://en.wikipedia.org/wiki/Mutual_exclusion

#### `Locked`ラッパ型

spin::Mutexラッパ型の助けを借りれば、バンプアロケータに`GlobalAlloc`トレイトを実装できます。このトレイトを`BumpAllocator`に直接実装するのではなく、ラップされた`spin::Mutex<BumpAllocator>`型に対して実装するのがミソです。

```rust
unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {…}
```

残念ながら、Rustコンパイラは他のクレートで定義された型のトレイト実装を許可していないため、これはまだうまくいきません。

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

これに対処するためには、`spin::Mutex`型をラップする独自の型を作ればよいです：

```rust
// in src/allocator.rs

/// トレイト実装を許してもらうための、spin::Mutexをラップする型
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

この型は、`spin::Mutex<A>`の<ruby>汎用<rp> (</rp><rt>ジェネリック</rt><rp>) </rp></ruby>ラッパです。ラップされる型`A`に制限はないので、アロケータだけでなく、あらゆる種類の型をラップするために使用できます。このラッパは、指定された値をラップする単純な`new`コンストラクタ関数を提供しています。ラップされた`Mutex`で`lock`を呼び出す`lock`関数も、便利なので提供しています。`Locked`型はとても汎用的であり、他のアロケータの実装にも役立つため、親の`allocator`モジュールに入れることにします。

#### `Locked<BumpAllocator>`の実装

`Locked`型は（`spin::Mutex`とは違って）私たちクレートの中で定義されているため、私たちのバンプアロケータに`GlobalAlloc`型を実装するために使用できます。実装の全体は次のようになります：

```rust
// in src/allocator/bump.rs

use super::{align_up, Locked};
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock(); // 可変参照を得る

        let alloc_start = align_up(bump.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            ptr::null_mut() // メモリ不足
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock(); // 可変参照を得る

        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}
```

`alloc`と`dealloc`は両方、まず、`inner`フィールドを通じて[`Mutex::lock`]メソッドを呼び出し、ラップされたアロケータ型への可変参照を取得します。インスタンスはメソッドの終了までロックされたままであるため、（まもなくスレッドのサポートを追加するのですが）マルチスレッドになってもデータ競合が発生することはありません。

[`Mutex::lock`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html#method.lock

前のプロトタイプと比較してみると、`alloc`の実装はアラインメント要件を守るようになっており、割り当てがヒープメモリ領域内にあることを保証するために境界チェックを実行するようになっています。この関数はまず、`next`アドレスを`Layout`引数で指定されたアラインメントに切り上げます。`align_up`関数のコードはすぐ後で示します。次に、要求された割り当てサイズを`alloc_start`に足して、割り当ての終端アドレスを得ます。巨大な割り当てが試みられた際に整数のオーバーフローが起きることを防ぐため、[`checked_add`]メソッドを使っています。オーバーフローが発生した場合、または割り当ての終端アドレスがヒープの終端アドレスよりも大きくなる場合、メモリ不足であることを示すためにヌルポインタを返します。それ以外の場合は、以前のように、`next`アドレスを更新し、`allocations`カウンタを1増やします。最後に、`*mut u8`ポインタに変換された`alloc_start`アドレスを返します。

[`checked_add`]: https://doc.rust-lang.org/std/primitive.usize.html#method.checked_add
[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html

`dealloc`関数は、指定されたポインタと`Layout`引数を無視します。代わりに、単に`allocations`カウンターを減らします。カウンターが`0`に戻ったなら、それはすべての割り当てが再び解放されたことを意味します。このとき、`next`アドレスを`heap_start`アドレスにリセットして、ヒープメモリ全体を再び使用できるようにします。

#### アドレスのアラインメント

`align_up`関数の用途は広いので、親の`allocator`モジュールに入れてもよいでしょう。基本的な実装は以下のようになります：

```rust
// in src/allocator.rs

/// 与えられたアドレス`addr`を`align`に上丸めする
fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr // addr はすでに丸められていた
    } else {
        addr - remainder + align
    }
}
```

この関数はまず、`align`で`addr`を割った[余り][remainder]を計算します。余りが`0`の場合、アドレスはすでに指定されたアラインメントに丸められているということです。それ以外の場合は、（余りが0になるように）余りを引いてアドレスをアラインし、（アドレスが元のアドレスよりも小さくならないように）アラインメントを足します。

[remainder]: https://en.wikipedia.org/wiki/Euclidean_division

実は、これはこの関数を実装する最も効率的な方法ではありません。はるかに高速な実装は次のようになります：

```rust
/// 与えられたアドレス`addr`を`align`に上丸めする
///
/// `align`は2の累乗でなければならない
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
```

この方法では、`align`が2の累乗である必要がありますが、これは`GlobalAlloc`トレイト（およびその[`Layout`]パラメータ）を利用するならば保証されています。この場合、非常に効率的にアドレスを揃えるための[ビットマスク][bitmask]を作成できます。その原理を理解するために、式の右側から一つずつ見ていきましょう：

[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html
[bitmask]: https://en.wikipedia.org/wiki/Mask_(computing)

- `align`は2の累乗であるため、その[2進数表現][binary representation]は1つのビットのみが1であるはずである（例：`0b000100000`）。これは、`align - 1`ではそれより下位のすべてのビットが1であることを意味する（例：`0b000011111`）。
- `!`演算子すなわち[ビットごとの`NOT`][bitwise `NOT`]を行うことで、「`align`より下位のビット」以外がすべて1であるような数字を得ることができる（例：`0b…111111111100000`）
- あるアドレスと`!(align - 1)`の間で[ビットごとの`AND`][bitwise `AND`]を行うことで、アドレスを**下向きに**アラインする。なぜなら、`align`よりも小さいビットがすべて0になるからである。
- 下向きではなく上向きにアラインしたいので、ビットごとの`AND`の前に`addr`を`align - 1`だけ増やしておく。こうすると、すでにアラインされているアドレスには影響がないが、アラインされていないアドレスは次のアラインメント境界に丸められるようになる。

[binary representation]: https://en.wikipedia.org/wiki/Binary_number#Representation
[bitwise `NOT`]: https://en.wikipedia.org/wiki/Bitwise_operation#NOT
[bitwise `AND`]: https://en.wikipedia.org/wiki/Bitwise_operation#AND

どちらの実装を使うかは自由です。結果は同じで、計算方法が違うだけです。

### 使ってみる

`linked_list_allocator`クレートの代わりにバンプアロケータを使うには、`allocator.rs`の`ALLOCATOR`静的変数を更新する必要があります：

```rust
// in src/allocator.rs

use bump::BumpAllocator;

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());
```

ここで、`BumpAllocator::new`と`Locked::new`を`const`関数として宣言しておいたことが効いてきます。`static`の初期化式はコンパイル時に評価可能でなければならないため、もしそれらが通常の関数だったならコンパイルエラーが発生していたでしょう。

[`const` functions]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

バンプアロケータは`linked_list_allocator`によって提供されるアロケータと同じインターフェイスを提供するため、`init_heap`関数の`ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE)`呼び出しを変更する必要はありません。

これで、私たちのカーネルはバンプアロケータを使うようになりました！　前回の記事で作った[`heap_allocation`のテスト][`heap_allocation` tests]を含め、すべての機能がうまくいくはずです。

[`heap_allocation` tests]: @/edition-2/posts/10-heap-allocation/index.ja.md#tesutowozhui-jia-suru
```
> cargo test --test heap_allocation
[…]
Running 3 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
```

### 議論

バンプアロケータの大きな利点は、非常に速いことです。`alloc`や`dealloc`のたびにサイズの合うメモリを動的に探索し様々な管理タスクを行う必要があるほかのアロケータの設計（後述）に比べると、バンプアロケータはたった数個のアセンブリ命令に[最適化することができる][bump downwards]のですから。これによりバンプアロケータは、メモリ割り当ての性能を最大化したいとき、例えば[仮想DOMライブラリ][virtual DOM library]を作成したいときなどに役に立ちます。

[bump downwards]: https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
[virtual DOM library]: https://hacks.mozilla.org/2019/03/fast-bump-allocated-virtual-doms-with-rust-and-wasm/

バンプアロケータがグローバルアロケータとして使われることはまれですが、バンプアロケーションの原理はしばしば[アリーナアロケーション][arena allocation]の形で使われます。これは要するに割り当てをバッチにまとめることで性能を上げるというものです。Rustにおけるアリーナアロケータの例は[`toolshed`]クレートに含まれています。

[arena allocation]: https://mgravell.github.io/Pipelines.Sockets.Unofficial/docs/arenas.html
[`toolshed`]: https://docs.rs/toolshed/0.8.1/toolshed/index.html

#### バンプアロケータの欠点

バンプアロケータの主な制約は、すべてのメモリ割り当てが解放されないと<ruby>割り当て解除<rp> (</rp><rt>デアロケート</rt><rp>) </rp></ruby>されたメモリを再利用できないことです。これは、たった一つでも「寿命の長い」割り当てがあると、メモリの再利用ができなくなってしまうことを意味します。`many_boxes`テストを少し変更したものを追加すると、それが起こるのを見ることができます。

```rust
// in tests/heap_allocation.rs

#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1); // ここを追加
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1); // ここを追加
}
```

`many_boxes`テストと同様、このテストは大量の割り当てを行うことで、アロケータが解放されたメモリを再利用できていない場合にメモリ不足エラーを引き起こします。さらに、このテストではループの間ずっと存在している`long_lived`という割り当てを追加しています。

この新しいテストを実行しようとすると、確かに失敗することがわかります：

```
> cargo test --test heap_allocation
Running 4 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [failed]

Error: panicked at 'allocation error: Layout { size_: 8, align_: 8 }', src/lib.rs:86:5
```

この失敗が発生する理由を詳しく理解してみましょう。まず、ヒープの先頭に変数`long_lived`の割り当てが作成され、`allocations`カウンタが1増加します。ループの反復ごとに、一時的な割り当てが作成され、次の反復が始まる前にすぐ解放されます。これは、`allocations`カウンタが反復の開始時に一時的に2に増加し、終了時に1に減少することを意味します。問題は、バンプアロケータは**すべての**割り当てが解放された時、つまり`allocations`カウンタが0に減ったときにのみメモリを再利用できるということです。これはループの間には起こらないため、各ループ反復で新しいメモリ領域が割り当てられ、結果として大量の反復の後にメモリ不足エラーを引き起こします。

#### テストを成功させるには

このテストを成功させるために、私たちのバンプアロケータに行える工夫が二つほど考えられます：

- `dealloc`を更新し、解放されたメモリが前回の`alloc`によって返されたものであるかを、その終端アドレスと`next`ポインタを比較することでチェックするようにします。もし等しいなら、`next`を解放された割り当ての先頭に戻しても大丈夫でしょう。こうすれば、それぞれの反復は同じメモリブロックを使うようになります。
- ヒープの**末尾**からメモリを割り当てていく`alloc_back`メソッドと、そのための`next_back`フィールドを追加するという方法もあります。長期間生存する割り当てには手動でこちらを使うようにすることで、ヒープ上における短期間の割り当てと長期間の割り当てを分離するのです。この「分離」は、どの割り当てがどのくらい生存するか事前にわかっていないと使えないということに注意してください。また、割り当てを手動で行うのは面倒だしunsafeかもしれないという欠点もあります。

どちらのアプローチでもテストを成功させられますが、非常に限られたケースでしかメモリを再利用できないため、一般的な解決策とはいえません。問題は、解放された**すべての**メモリを再利用する一般的な解決策はあるのか、ということです。

#### 解放されたすべてのメモリを再利用するには？

[前回の記事][heap-intro]で学んだように、割り当ては任意の期間生存する可能性があり、どのような順序でも解放されえます。これは、次の例に示すように、個数に上限のない、非連続な未使用メモリ領域を管理する必要があることを意味します：

[heap-intro]: @/edition-2/posts/10-heap-allocation/index.ja.md#dong-de-dainamituku-memori
![](allocation-fragmentation.svg)

この図は、ヒープの経時変化を示しています。最初は、ヒープ全体が未使用で、`next`アドレスは`heap_start`に等しいです（1行目）。その後、最初の割り当てが行われます（2行目）。3行目では、2つ目のメモリブロックが割り当てられ、最初の割り当ては解放されています。4行目ではたくさんの割り当てが追加されています。それらの半分は非常に短命であり、すでに5行目では解放されていますが、この行では新しい割り当ても追加されています。

5行目が根本的な問題を示しています：サイズの異なる未使用のメモリ領域が5つありますが、`next`ポインタはそのうち最後の領域の先頭を指すことしかできません。たとえば今回なら、長さ4の配列に、ほかの未使用メモリ領域の開始アドレスとサイズを保存することはできます。しかし、未使用メモリ領域の数が8個とか16個、1000個にもなる例だって簡単に作れてしまうので、これは一般的な解決策ではありません。

普通、要素数に上限がないときは、ヒープに割り当てられたコレクションを使ってしまえばいいです。これは私たちの場合には実際には不可能です──なぜなら、ヒープアロケータが自分自身に依存するのは不可能ですから（無限再帰やデッドロックを起こしてしまうでしょう）。なので別の解決策を見つける必要があります。

## <ruby>連結<rp> (</rp><rt>リンクト</rt><rp>) </rp></ruby>リストアロケータ

アロケータを実装する際、任意の数の空きメモリ領域を管理するためによく使われる方法は、これらの領域自体を管理領域として使用することです。この方法は、未使用メモリ領域もまた仮想アドレスにマッピングされており、対応する物理フレームも存在しはするが、そこに保存された情報はもはや必要ない、ということを利用します。解放された領域に関する情報をそれらの領域自体に保存することで、追加のメモリを必要とせずにいくらでも解放された領域を管理できます。

最もよく見られる実装方法は、解放されたメモリの中に、各ノードが解放されたメモリ領域であるような一つの連結リストを作るというものです：

![](linked-list-allocation.svg)

リストの各ノードには、メモリ領域のサイズと次の未使用メモリ領域へのポインタの2つのフィールドが含まれています。このアプローチでは、未使用領域がいくつあろうと、そのすべてを最初の未使用領域（`head`と呼ばれる）へのポインタだけで管理できます。結果として生じるこのデータ構造は、しばしば[フリーリスト][_free list_]と呼ばれます。

[_free list_]: https://en.wikipedia.org/wiki/Free_list

名前から想像がつくかもしれませんが、この方法は`linked_list_allocator`クレートが使用しているものです（訳注：連結リストアロケータはlinked list allocatorの訳）。このテクニックを使用するアロケータは、しばしば**プールアロケータ**とも呼ばれます。

### 実装

以下では、解放されたメモリ領域を管理するために上記の方法を使用する、独自のシンプルな`LinkedListAllocator`型を作成します。記事のこの部分は今後の記事には必要ありませんので、実装の詳細を飛ばしていただいてもかまいません。

#### アロケータ型

まず、新しい`allocator::linked_list`サブモジュールの中に<ruby>非公開<rp> (</rp><rt>プライベート</rt><rp>) </rp></ruby>の`ListNode`構造体を作ることから始めましょう：

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

図に示したように、リストのノードは`size`フィールドと、次のノードへのオプショナルなポインタを持ちます。後者は`Option<&'static mut ListNode>`型によって表されます。`&'static mut`型はポインタで指されている[所有された][owned]オブジェクトを意味します。要するに、スコープの終了時にオブジェクトを解放するデストラクタを持たないような[`Box`]型です。

[owned]: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html
[`Box`]: https://doc.rust-lang.org/alloc/boxed/index.html

以下の`ListNode`のメソッドを実装します：

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

この型は`new`という単純なコンストラクタ関数を持ち、表現する領域の開始・終端アドレスを計算するメソッドを持っています。`new`関数は[const関数][const function]としていますが、これは後で静的な連結リストアロケータを作る際に必要になるためです。

[const function]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

`ListNode`構造体を部品として使うことで、`LinkedListAllocator`構造体を作ることができます：

```rust
// in src/allocator/linked_list.rs

pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    /// 空のLinkedListAllocatorを作る。
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// 与えられたヒープ境界でアロケータを初期化する。
    ///
    /// この関数はunsafeである。なぜなら、呼び出し元は渡すヒープ境界が
    /// 有効でヒープが未使用であることを保証しなければならないからである。
    /// このメソッドは一度しか呼ばれてはならない。
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    /// 与えられたメモリ領域をリストの先頭に追加する。
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        todo!();
    }
}
```

この構造体は、最初のヒープ領域を指す`head`ノードを持っています。ここでは`next`ポインタの値にしか興味がないので、`ListNode::new`関数では`size`を0にしてしまいます。`head`を単に`&'static mut ListNode`にするのではなく`ListNode`にすると、`alloc`メソッドの実装が単純にできるというメリットがあります。

バンプアロケータと同じように、`new`関数はアロケータをヒープ境界で初期化したりはしません。この理由は、APIの互換性を保つためというのに加え、初期化ルーチンがノードをヒープメモリに書き込む必要があり、これは実行時にしか行えないということがあります。`new`関数は`ALLOCATOR`静的変数を初期化するのに使われるので、[`const`関数][`const` function]すなわちコンパイル時に評価できる関数である必要があります。この理由によって、ここでも、非constな`init`メソッドを別に提供しているというわけです。

[`const` function]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

`init`メソッドは`add_free_region`メソッドを使っていますが、この実装はすぐ後で示します。今のところは、[`todo!`]マクロを実装の代わりに置いておいて、常にパニックするようにしておきましょう。

[`todo!`]: https://doc.rust-lang.org/core/macro.todo.html

#### `add_free_region`メソッド

`add_free_region`メソッドは連結リストの最も基本的な操作である**プッシュ**操作を提供します。今はこのメソッドは`init`からしか呼んでいませんが、このメソッドは私たちが`dealloc`を実装する際にも中心的な役割を果たします。`dealloc`メソッドは割り当てられたメモリ領域が解放されたときに呼ばれるのだということを思い出してください。その解放されたメモリ領域を管理するために、それを連結リストにプッシュする必要があるのです。

`add_free_region`メソッドの実装は以下のようになります：

```rust
// in src/allocator/linked_list.rs

use super::align_up;
use core::mem;

impl LinkedListAllocator {
    /// 与えられたメモリ領域をリストの先頭に追加する。
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // 解放された領域がListNodeを格納できることを確かめる
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // 新しいリストノードを作り、それをリストの先頭に追加する
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr)
    }
}
```

このメソッドはメモリ領域のアドレスと大きさを引数として取り、リストの先頭にそれを追加します。まず、与えられた領域が`ListNode`を格納するのに必要なサイズとアラインメントを満たしていることを確認します。次に、ノードを作成し、それを以下のようなステップでリストに追加します：

![](linked-list-allocator-push.svg)

Step 0は`add_free_region`が呼ばれる前のヒープの状態を示しています。Step 1では、`add_free_region`メソッドが図において`freed`と書かれているメモリ領域で呼ばれました。初期チェックを終えると、このメソッドは[`Option::take`]メソッドを使ってノードの`next`ポインタを現在の`head`ポインタに設定し、これによって`head`ポインタは`None`に戻ります。

[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

Step 2では、このメソッドは新しく作られた`node`を`write`メソッドを使って解放されたメモリ領域の先頭に書き込みます。次に`head`ポインタがこの新しいノードを指すようにします。解放された領域は常にリストの先頭に挿入されていくので、結果として生じるポインタ構造はいささか混沌としているように思われますが、`head`ポインタからポインタをたどっていけば、解放されたそれぞれの領域に到達できるというのには変わりありません。

[`write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write

#### `find_region`メソッド

連結リストの二つ目の基本操作は要素を探してリストからそれを取り除くことです。これは`alloc`メソッドの実装の中核となる操作です。この操作を`find_region`メソッドとして以下のように実装しましょう：

```rust
// in src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// 与えられたサイズの解放された領域を探し、リストからそれを
    /// 取り除く。
    ///
    /// リストノードと割り当ての開始アドレスからなるタプルを返す。
    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut ListNode, usize)>
    {
        // 現在のリストノードへの参照。繰り返しごとに更新していく
        let mut current = &mut self.head;
        // 連結リストから十分大きな領域を探す
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // 領域が割り当てに適している -> リストから除く
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // 割り当てに適していない -> 次の領域で繰り返す
                current = current.next.as_mut().unwrap();
            }
        }

        // 適した領域が見つからなかった
        None
    }
}
```

このメソッドは`current`変数と`while let`ループを使ってリストの各要素に関して反復を行っています。はじめ、`current`は（ダミーの）`head`ノードに設定されています。繰り返しごとに（`else`ブロックで）これは現在のノードの`next`フィールドへと更新されます。領域が与えられたサイズとアラインメントの割り当てに適しているなら、その領域がリストから取り除かれて`alloc_start`アドレスとともに返されます。

[`while let` loop]: https://doc.rust-lang.org/reference/expressions/loop-expr.html#predicate-pattern-loops

`current.next`ポインタが`None`になった場合、ループから抜けます。これは、リスト全体を反復したものの割り当てに適した領域が見つからなかったことを意味します。その場合`None`を返します。領域が適しているか否かは`alloc_from_region`によってチェックされていますが、この関数の実装はすぐに示します。

適した領域がリストから除かれる様子をもう少し詳しく見てみましょう：

![](linked-list-allocator-remove-region.svg)

Step 0はポインタに修正を行う前の状況を表しています。`region`と`current`という領域と、`region.next`と`current.next`というポインタが図中に示されています。Step 1では、`region.next`と`current.next`ポインタが[`Option::take`]メソッドによって`None`に戻されています。ポインタの元の値は`next`と`ret`というローカル変数に格納されています。

Step 2では、ポインタ`current.next`がローカル変数であるポインタ`next`（元々は`region.next`ポインタだったもの）に設定されています。これにより、`current`は`region`の次の領域を指すようになっているので、`region`はもはやこの連結リストの要素ではありません。この関数はその後、ローカル変数`ret`に格納されていた`region`へのポインタを返します。

##### `alloc_from_region`関数

`alloc_from_region`関数は領域が与えられたサイズとアラインメントの割り当てに適しているかどうかを返します。以下のように定義されます：

```rust
// in src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// 与えられた領域で与えられたサイズとアラインメントの
    /// 割り当てを行おうとする。
    ///
    /// 成功した場合、割り当ての開始アドレスを返す。
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
        -> Result<usize, ()>
    {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            // 領域が小さすぎる
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            // 領域の残りが小さすぎてListNodeを格納できない（割り当ては
            // 領域を使用部と解放部に分けるので、この条件が必要）
            return Err(());
        }

        // 領域は割り当てに適している
        Ok(alloc_start)
    }
}
```

まず、この関数は行おうとしている割り当ての開始・終端アドレスを、先ほど定義した`align_up`関数と[`checked_add`]メソッドを使って計算します。オーバーフローが起こったり、（割り当ての）終端アドレスが領域の終端アドレスよりも後ろにあったりした場合は、割り当ては領域に入りきらないのでエラーを返します。

その後でこの関数は、必要な理由がやや分かりにくいチェックを行っています。このチェックが必要になるのは、多くの場合適した領域にも割り当てがぴったりフィットするわけではないので、割り当て後も一部の領域が使用可能なままになるからです。領域のこの部分は割り当て後も自分自身の`ListNode`を格納しなければならないので、それが可能なくらいのサイズがないといけません。このチェックはまさにそれを確かめています：割り当てが完璧にフィットするか（`excess_size == 0`）、または`ListNode`を格納するのに十分超過領域が大きいかを調べています。

#### `GlobalAlloc`を実装する

`add_free_region`と`find_region`メソッドによって基本となる操作が提供されたので、ついに`GlobalAlloc`トレイトを実装することができます。バンプアロケータの時と同じように、このトレイトを`LinkedListAllocator`に直接実装するのではなく、ラップされた`Locked<LinkedListAllocator>`に実装するようにします。[`Locked`ラッパ][`Locked` wrapper]はスピンロックによって内部可変性を追加するので、これにより`&self`参照しか取らない`alloc`や`dealloc`メソッドでもアロケータを変更できるようになります。

[`Locked` wrapper]: @/edition-2/posts/11-allocator-designs/index.ja.md#lockedratupaxing

実装は以下のようになります：

```rust
// in src/allocator/linked_list.rs

use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // レイアウト調整を行う
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                allocator.add_free_region(alloc_end, excess_size);
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // レイアウト調整を行う
        let (size, _) = LinkedListAllocator::size_align(layout);

        self.lock().add_free_region(ptr as usize, size)
    }
}
```

`dealloc`メソッドのほうが単純なのでこちらから見ていきましょう：このメソッドではまず、何かしらのレイアウト調整（すぐ後で説明します）を行っています。その次に、`&mut LinkedListAllocator`という参照を[`Locked`ラッパ][`Locked` wrapper]の[`Mutex::lock`]関数を呼ぶことによって取得します。最後に、`add_free_region`関数で割り当て解除された領域をフリーリストに追加します。

`alloc`メソッドはもう少し複雑です。（`dealloc`と）同じようにレイアウト調整を行い、[`Mutex::lock`]でアロケータの可変参照を得るところから始めます。次に`find_region`メソッドを使って割り当てに適したメモリ領域を見つけ、それをリストから取り除きます。これが成功せず`None`が返された場合、適したメモリ領域がないため、（このメソッドは）`null_mut`を返すことでエラーを表します。

成功した場合、`find_region`メソッドは（リストからすでに除かれた）適した領域と、割り当ての開始アドレスからなるタプルを返します。（それを受け、`alloc`は）`alloc_start`と割り当てのサイズ、および領域の終端アドレスを使うことで、割り当ての終端アドレスと超過サイズを再び計算します。もし超過サイズがゼロでないなら、`add_free_region`を呼んでメモリ領域の超過サイズをフリーリストに戻します。最後に、`alloc_start`アドレスを`*mut u8`ポインタにキャストして返します。

#### レイアウト調整

……で、`alloc`と`dealloc`両方の最初に行っていたレイアウト調整はいったい何なのでしょうか？　これらは、それぞれの割り当てブロックが`ListNode`を格納することができることを保証しているのです。これが重要なのは、このメモリブロックはいつか割り当て解除されることになるので、そのときそこに`ListNode`を書き込む必要が出てくるからです。ブロックが`ListNode`より小さかったり正しいアラインメントがなされていなかったりすると、未定義動作につながります。

レイアウト調整は`size_align`関数によって行われています。この定義は以下のようになっています：

```rust
// in src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// 与えられたレイアウトを調整し、割り当てられるメモリ領域が
    /// `ListNode`を格納することもできるようにする。
    ///
    /// 調整されたサイズとアラインメントをタプルとして返す。
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

まず、この関数は渡された[`Layout`]の[`align_to`]メソッドを使って、そのアラインメントを`ListNode`のアラインメントにまで（必要なら）増やします。次に[`pad_to_align`]メソッドを使って、レイアウトのサイズがアラインメントの倍数であるようにし、次のメモリブロックのアラインメントもまた`ListNode`を格納できる適切なものになるようにします。
次に、[`max`]メソッドによって割り当てが最低でも`mem::size_of::<ListNode>`の大きさになるようにします。こうしておけば、`dealloc`関数は安心して`ListNode`を解放されたメモリブロックに書き込むことができます。

[`align_to`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align_to
[`pad_to_align`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.pad_to_align
[`max`]: https://doc.rust-lang.org/std/cmp/trait.Ord.html#method.max

### 使ってみる

今や、`allocator`モジュール内の`ALLOCATOR`静的変数を新しい`LinkedListAllocator`で置き換えられます：

```rust
// in src/allocator.rs

use linked_list::LinkedListAllocator;

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> =
    Locked::new(LinkedListAllocator::new());
```

`init`関数はバンプアロケータでも連結リストアロケータでも同じ振る舞いをするようにしたので、`init_heap`内における`init`関数の呼び出しを修正する必要はありません。

`heap_allocation`テストをもう一度実行すると、バンプアロケータでは失敗していた`many_boxes_long_lived`テストを含めすべてのテストをパスします：

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

これは、私たちの連結リストアロケータが、二つ目以降の割り当てのメモリが解放されたときも、それを再利用できていることを示しています。

### 議論

解放されたメモリをすぐに再利用できるため、連結リストアロケータは汎用のアロケータとしてバンプアロケータよりはるかに優れています。しかし欠点もあります。そのうちいくつかは私たちの実装が高度でないために起きているのですが、アロケータの設計自体にも根本的な欠点があるのです。

#### 解放されたブロックを結合する

私たちの実装の大きな問題は、ヒープをより小さなブロックへと分割してはいくものの、それらを結合し直すことは全くやっていないことです。次の例を考えてみましょう：

![](linked-list-allocator-fragmentation-on-dealloc.svg)

最初の行では、ヒープ上に三つの割り当てが作られています。2行目ではそのうち2つが、3行目では3つ目が解放されています。今やヒープ全体が未使用状態に戻ったわけですが、まだ4つの別々のブロックに分かれたままです。この時点で、4つのブロックどれもサイズが足らず、巨大な割り当てが不可能ということがあり得るかもしれません。時間がたち、このプロセスがさらに続くと、ヒープはさらに小さいブロックへと分割されています。いつかのタイミングで、ヒープがあまりにも断片化したせいで、普通の割り当てすら失敗するようになってしまうでしょう。

この問題を解決するためには、隣り合う解放されたブロックを結合する必要があります。上の例の場合、以下を意味します：

![](linked-list-allocator-merge-on-dealloc.svg)

図中の`2`の行では、以前のように、3つの割り当てのうち2つが解放されています。ここで、ヒープを断片化したままにしておくのではなく、追加で`2a`のステップを行って右端の二つのブロックを結合して一つに戻しましょう。`3`行目では（以前のように）3つめの領域が解放され、3つの異なるブロックで表される完全に未使用のヒープができました。追加で`3a`の結合ステップを行い、これらの隣り合ったブロックを結合して一つに戻します。

`linked_list_allocator`クレートはこのような結合戦略を以下のように実装しています：`deallocate`にて、解放されたメモリブロックを連結リストの先頭に入れるかわりに、リストを常に開始アドレスでソートされた状態にしておくのです。こうすると、`deallocate`関数の呼び出しが行われたときに、リスト内で隣り合うブロックのアドレスとサイズを調べることで、結合を即座に行うことができます。もちろん、このようにすると割り当て解除操作は遅くなってしまいますが、上で見たようなヒープの断片化は防ぐことができます。

#### 性能

前述したように、バンプアロケータはとんでもなく速く、ほんの数個のアセンブリ命令に最適化することができます。これらと比べると、連結リストアロケータの性能はずっと悪いです。問題は、割り当ての要求に対し、適したブロックが見つかるまで連結リスト全体を調べ上げる必要があるかもしれないことです。

リスト長は未使用のメモリブロックの数によって決まるので、プログラムごとに性能は大きく変わりえます。いくつかしか割り当てを行わないプログラムは、割り当ての性能が比較的よいと感じることでしょう。しかし、大量の割り当てでヒープを断片化させてしまうプログラムの場合、連結リストがとても長くなり、そのほとんどがとても小さなブロックしか持たないということになるので、割り当ての性能は非常に悪くなってしまうでしょう。

この性能の問題は、私たちの実装が簡素なせいで起きているのではなく、連結リストを使った方法の根本的な問題であるということに注意してください。アロケータの性能はカーネルレベルのコードにとって非常に重要になるので、ここからは第三のアプローチ──性能を向上する代わりに、メモリの利用効率を犠牲にするもの──を見ていきましょう。

## 固定サイズブロックアロケータ

以下では、割り当ての要求を遂行するために固定サイズのメモリブロックを使うアロケータの設計を示します。こうすると、アロケータはしばしば必要なものより大きなブロックを返すので、[内部断片化][internal fragmentation]によるメモリの無駄が発生します。いっぽうで、適切なブロックを見つけるのに必要な時間が（連結リストアロケータと比べて）激減するので、割り当ての性能はずっとよくなります。

### 導入

**固定サイズブロックアロケータ**の背後にある発想は以下のようなものです：要求された量ぴったりのメモリを返す代わりに、いくつかのブロックサイズを決めて、割り当てのサイズを次のブロックサイズに切り上げるようにするのです。たとえば、ブロックサイズを16, 64, 512バイトとしたら、4バイトの割り当ては16バイトのブロックを、48バイトの割り当ては64バイトのブロックを、128バイトの割り当ては512バイトのブロックを返します。

連結リストアロケータと同じように、未使用メモリ部に連結リストを作ることによって未使用メモリを管理します。しかし、様々なブロックサイズのブロックを持つ一つのリストを使うのではなく、それぞれのサイズクラスごとに別のリストを作ります。それぞれのリストは一つのサイズのブロックのみを格納するのです。例えば、ブロックサイズが16, 64, 512のとき、3つの別々の連結リストがメモリ内にできます：

![](fixed-size-block-example.svg).

`head`ポインタも一つではなく、`head_16`, `head_64`, `head_512`という、対応するサイズの最初の未使用ブロックを指す3つのポインタがあることになります。一つのリスト内のノードはすべて同じサイズです。たとえば、`head_16`ポインタから始まるリストには16バイトのブロックのみが含まれます。これが意味するのは、ヘッドポインタの名前でそれぞれのリストのノードサイズは指定されているので、ノード内にそれらを格納する必要はないということです。

リスト内のそれぞれの要素は同じサイズを持っているので、割り当ての要求に要素が適しているかはすべての要素について同じです。これは、以下の手順をとることで非常に効率的に割り当てを行えるということを意味します：

- 要求された割り当てサイズを次のブロックサイズに切り上げる。たとえば、上の例で12バイトの割り当てが要求されたら、ブロックサイズを16バイトとする。
- リストのヘッドポインタを手に入れる。ブロックサイズが16なら、`head_16`を使う。
- リストから最初のブロックを取り除きそれを返す。

注目すべきは、常にリストの最初の要素を返せばよく、リスト全体を走査する必要はないということです。よって、連結リストアロケータに比べて割り当てはずっと高速になります。

#### ブロックサイズと無駄になるメモリ

ブロックサイズの決め方によっては、切り上げによって多くのメモリを失うことになります。例えば、128バイトの割り当てに対し512バイトのブロックが返されるとき、割り当てられたメモリの3/4は使われません。適切なブロックサイズを使うことで、無駄になるメモリの量をある程度にまで減らすことはできます。例えば、ブロックサイズとして2の累乗（4, 8, 16, 32, 64, 128, ……）を使うと、無駄になるメモリを最悪でもメモリサイズの半分、平均してメモリサイズの1/4とすることができます。

ブロックサイズをプログラムにおいてよく使われるサイズに基づいて最適化するというのも、よく行われます。例えば、24バイトのメモリ割り当てをよく行うプログラムにおけるメモリ効率を向上するため、24バイトのブロックサイズを追加することができるでしょう。このように、無駄になるメモリの量はしばしば性能上の利点を失うことなく減らすことができます。

#### 割り当て解除

割り当てと同様、割り当ての解除もとても重要です。以下の手順をとります：

- 解放された割り当てサイズを次のブロックサイズに切り上げる。これが必要になるのは、コンパイラが`dealloc`に渡してくるのは要求したときの割り当てサイズであり、`alloc`によって返されたブロックのサイズではないためである。`alloc`と`dealloc`で同じサイズ修正関数を使うことで、正しい量のメモリを解放していることは保証される。
- リストのヘッドポインタを手に入れる。
- ヘッドポインタを更新することで、解放されたブロックをリストの先頭に追加する。

注目すべきは、割り当て解除においてもリストの走査は必要ないということです。これが意味するのは、`dealloc`に必要な時間はリスト長によらず一定だということです。

#### <ruby>代替<rp> (</rp><rt>フォールバック</rt><rp>) </rp></ruby>アロケータ

（2KBを超えるような）大きな割り当ては、とくにオペレーティングシステムのカーネルにおいては珍しいことが多いので、そのような割り当てに対しては<ruby>代替<rp> (</rp><rt>フォールバック</rt><rp>) </rp></ruby>のアロケータを使うのがよいかもしれません。例えば、2048バイトより大きな割り当てに対してはメモリの無駄を減らすために連結リストアロケータにフォールバックするのです。そのようなサイズの割り当ての数は非常に少ないはずなので、連結リストの長さが長くなることはなく、割り当て・割り当ての解除も比較的速くできるでしょう。

#### 新しいブロックを作る

上では、リスト内には特定のサイズのブロックがつねに十分あり、すべての割り当ての要求を満足できることを仮定していました。しかし、いつかの時点で、あるブロックサイズの連結リストが空になってしまうでしょう。そのとき、割り当ての要求を満足するために特定のサイズの未使用ブロックを作り出す方法が二つ考えられます：

- 代替アロケータ（もしあるなら）から新しいブロックを割り当てる
- 別のリストからより大きなブロックを持ってきて、それを分割する。この方法は、ブロックサイズが2の累乗であるときに最もうまくいく。例えば、32バイトのブロックは二つの16バイトのブロックに分割できる。

実装がずっと簡単になるので、私たちの実装では代替アロケータから新しいブロックを割り当てることにしましょう。

### 実装

固定サイズブロックアロケータの仕組みを理解したので、実装を始めることができます。以前のパートで作成した連結リストアロケータの実装は使わないので、もし連結リストアロケータの実装部分を飛ばしていたとしても、この部分は読み進めることができます。

#### リストノード

実装は、新しい`allocator::fixed_size_block`モジュールに`ListNode`型を作るところから始めましょう。

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

この型は[連結リストアロケータの実装][linked list allocator implementation]における`ListNode`型と似ていますが、`size`フィールドがありません。固定サイズブロックアロケータにおいては、リスト内のすべてのブロックが同じサイズを持つため、必要ないのです。

[linked list allocator implementation]: #aroketaxing

#### ブロックサイズ

つぎに、私たちの実装におけるブロックサイズをもつ定数スライス`BLOCK_SIZES`を定義します：

```rust
// in src/allocator/fixed_size_block.rs

/// 使用するブロックサイズ。
///
/// これらは2の累乗でなければならない。なぜなら、これらは
/// （2の累乗でなければならない）ブロックのアラインメントとしても使われるからである。
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];
```

ブロックサイズとして、8から2048までの2の累乗を使います。8より小さいブロックサイズを定義しないのは、それぞれのブロックは、解放されたときに次のブロックを指す64ビットのポインタを格納することができなければならないからです。2048バイトより大きな割り当てに対しては、代替の連結リストアロケータに任せましょう。

実装を簡単にするために、ブロックのサイズとメモリに要求されるアラインメントを同じにすることにします。つまり、16バイトのブロックはつねに16バイトの境界に、512バイトのブロックは512バイトの境界に合わせられます。アラインメントは常に2の累乗でなければならないので、他のブロックサイズは許されないのです。2の累乗でないブロックサイズが必要になった場合は、（例えば、`BLOCK_ALIGNMENTS`配列を定義することで）この実装を修正することもできます。

#### アロケータ型

`ListNode`型と`BLOCK_SIZES`スライスを使って、私たちのアロケータ型を定義することができます：

```rust
// in src/allocator/fixed_size_block.rs

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}
```

`list_heads`フィールドはブロックサイズごとの`head`ポインタの配列です。これは`BLOCK_SIZES`に`len()`を使うことで配列長とすることで実装しています。最大のブロックサイズよりも大きな割り当てに対する代替アロケータとして、`linked_list_allocator`の提供するアロケータを使います。私たち自身で実装した`LinkedListAllocator`を使っても良いのですが、これには[解放されたブロックを結合][merge freed blocks]する機能が実装されていません。

[merge freed blocks]: #jie-fang-saretaburotukuwojie-he-suru

`FixedSizeBlockAllocator`を作るには、他のアロケータ型に実装したのと同じ`new`関数と`init`関数を実装すればよいです：

```rust
// in src/allocator/fixed_size_block.rs

impl FixedSizeBlockAllocator {
    /// 空のFixedSizeBlockAllocatorを作る。
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// アロケータを与えられたヒープ境界で初期化する。
    ///
    /// この関数はunsafeである；呼び出し元は与えるヒープ境界が有効であり
    /// ヒープが未使用であることを保証しなければならないからである。
    /// このメソッドは一度しか呼ばれてはならない。
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback_allocator.init(heap_start, heap_size);
    }
}
```

`new`関数がするのは、`list_heads`配列を空のノードで初期化し、`fallback_allocator`として[`empty`]で空の連結リストアロケータを作ることだけです。`EMPTY`定数が必要なのは、Rustコンパイラに配列を定数値で初期化したいのだと伝えるためです。配列を直接`[None; BLOCK_SIZES.len()]`で初期化するとうまくいきません──なぜなら、そうするとコンパイラは`Option<&'static mut ListNode>`が`Copy`トレイトを実装していることを要求するようになるのですが、そうはなっていないからです。これは現在のRustコンパイラの制約であり、将来解決するかもしれません。

[`empty`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.empty

このunsafeな`init`関数は`fallback_allocator`の[`init`]関数を呼ぶだけで、`list_heads`配列の初期化などは行いません。これらの配列の初期化は、`alloc`と`dealloc`呼び出しが行われたときに初めて行います。

[`init`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.init

利便性のため、`fallback_allocator`を使って割り当てを行う非公開のメソッド`fallback_alloc`も作ります：

```rust
// in src/allocator/fixed_size_block.rs

use alloc::alloc::Layout;
use core::ptr;

impl FixedSizeBlockAllocator {
    /// 代替アロケータを使って割り当てを行う。
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}
```

`linked_list_allocator`クレートの[`Heap`]型は[`GlobalAlloc`]を実装してはいません（[ロックを使わない限り不可能なため][not possible without locking]）。代わりに、[`allocate_first_fit`]というインターフェイスの少し違うメソッドを提供しています。これは、`*mut u8`を返したり、エラーを表すためにヌルポインタを使うのではなく、`Result<NonNull<u8>, ()>`を返します。[`NonNull`]型は、ヌルポインタでないことが保証されている生ポインタの抽象化です。`Ok`の場合は[`NonNull::as_ptr`]メソッドへ、`Err`の場合ヌルポインタへと対応づけることで、これを簡単に`*mut u8` 型に戻すことができます。

[`Heap`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html
[not possible without locking]: #globalalloctoke-bian-xing
[`allocate_first_fit`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.allocate_first_fit
[`NonNull`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html
[`NonNull::as_ptr`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html#method.as_ptr

#### リストのインデックスを計算する

`GlobalAlloc`トレイトを実装する前に、与えられた[`Layout`]を格納できる最小のブロックサイズを返すようなヘルパ関数`list_index`を定義します：

```rust
// in src/allocator/fixed_size_block.rs

/// 与えられたレイアウトに対して適切なブロックサイズを選ぶ。
///
/// `BLOCK_SIZES`配列のインデックスを返す。
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}
```

ブロックは少なくとも与えられた`Layout`の要求するサイズとアラインメントを持っていないといけません。私たちはブロックサイズがブロックのアラインメントでもあると定義していたので、これは`required_block_size`がレイアウトの[`size()`]と[`align()`]属性の[最大値][maximum]であるということを意味します。`BLOCK_SIZES`スライスの中でそれよりも大きいブロックを探すために、まず[`iter()`]メソッドでイテレータを得て、つぎに[`position()`]メソッドで`required_block_size`以上の大きさを持つ最初のブロックのインデックスを見つけます。

[maximum]: https://doc.rust-lang.org/core/cmp/trait.Ord.html#method.max
[`size()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.size
[`align()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align
[`iter()`]: https://doc.rust-lang.org/std/primitive.slice.html#method.iter
[`position()`]:  https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.position

ブロックサイズそのものではなく、`BLOCK_SIZES`スライスのインデックスを返していることに注意してください。これは、ここで返したインデックスを`list_heads`配列のインデックスとして使いたいからです。

#### `GlobalAlloc`を実装する

最後のステップは、`GlobalAlloc`トレイトを実装することです：

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

他のアロケータの時と同じく、`GlobalAlloc`トレイトをアロケータ型に直接実装するのではなく、[`Locked`ラッパ][`Locked` wrapper]を使って同期された内部可変性を追加しています。`alloc`と`dealloc`の実装は結構長いので、以下で一つ一つ示していきます。

##### `alloc`

`alloc`メソッドの実装は以下のようになります：

```rust
// src/allocator/fixed_size_block.rsの`impl`ブロックの中

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
                    // リストにブロックがない→新しいブロックを割り当てる
                    let block_size = BLOCK_SIZES[index];
                    // すべてのブロックサイズが2の累乗であるときにのみ正しく動く
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

一つ一つ見ていきましょう：

まず、`Locked::lock`メソッドを使ってラップされたアロケータのインスタンスへの可変参照を手に入れます。次に、ついさっき定義した`list_index`関数を呼んで、与えられたレイアウトに対して適切なブロックサイズを計算し、`list_heads`配列の対応するインデックスを得ます。これが`None`だったなら、割り当てに適したブロックサイズはないので、`fallback_alloc`関数を使って`fallback_allocator`を使います。

もしリストのインデックスが`Some`なら、`list_heads[index]`から始まる対応するリストから[`Option::take`]メソッドを使って最初のノードを取り出すことを試みます。リストが空でないなら、`match`文の`Some(node)`節に入り、（ふたたび[`take`][`Option::take`]を使って）`node`の次の要素を取り出しリストの先頭のポインタとします。最後に、取り出された`node`ポインタを`*mut u8`として返します。

[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

もしリストのヘッドが`None`だったなら、ブロックリストが空であったということです。この場合、[上で説明した](#xin-siiburotukuwozuo-ru)ように新しいブロックを作らなくてはなりません。そのために、まず現在のブロックサイズを`BLOCK_SIZES`スライスから得て、それを新しいブロックのサイズとアラインメント両方として使います。それによって新しい`Layout`を作り、`fallback_alloc`メソッドを使って割り当てを行います。レイアウトとアラインメントの調整をしているのは、割り当て解除の際にこのブロックがブロックリストに追加されるからです。

#### `dealloc`

`dealloc`メソッドの実装は以下のようになります：

```rust
// in src/allocator/fixed_size_block.rs

use core::{mem, ptr::NonNull};

// `unsafe impl GlobalAlloc`ブロックの中

unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            let new_node = ListNode {
                next: allocator.list_heads[index].take(),
            };
            // ブロックがノードを格納できるサイズとアラインメントを持っていることを確認
            assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
            assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
            let new_node_ptr = ptr as *mut ListNode;
            new_node_ptr.write(new_node);
            allocator.list_heads[index] = Some(&mut *new_node_ptr);
        }
        None => {
            let ptr = NonNull::new(ptr).unwrap();
            allocator.fallback_allocator.deallocate(ptr, layout);
        }
    }
}
```

`alloc`と同じように、まず`lock`メソッドを使ってアロケータの可変参照を得て、`list_index`関数で与えられた`Layout`に対応するブロックリストを得ます。インデックスが`None`なら、`BLOCK_SIZES`にはサイズの合うブロックサイズがなかった、つまりこの割り当てが代替アロケータによって行われたことを意味します。従って、代替アロケータの[`deallocate`][`Heap::deallocate`]を使ってメモリを解放します。このメソッドは`*mut u8`ではなく[`NonNull`]を受け取るので、先にポインタを変換しておく必要があります（ここの`unwrap`はポインタがヌル値だったときのみ失敗するのですが、コンパイラが`dealloc`を呼ぶときにはそれは決して起きないはずです）。

[`Heap::deallocate`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.deallocate

もし`list_index`がブロックのインデックスを返したなら、解放されたメモリブロックをリストに追加しなければなりません。このために、まず現在のリストの先頭を指す新しい`ListNode`を（ここでも[`Option::take`]を使って）作ります。新しいノードを解放されたメモリブロックに書き込む前に、`index`によって指定されている現在のブロックサイズが`ListNode`を格納するのに必要なサイズとアラインメントを満たしていることをassertします。その後与えられた`*mut u8`ポインタを`*mut ListNode`ポインタに変換し、これに対しunsafeな[`write`][`pointer::write`]メソッドを使うことで書き込みを実行します。最後のステップはリストの先頭ポインタ──これに対して`take`を呼んだので現在は`None`です──を設定することです。このために、生の`new_node_ptr`を可変参照に変換します。

[`pointer::write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write

いくつか注目すべきことがあります：

- 私たちは、ブロックリストによって割り当てられたブロックと代替アロケータによって割り当てられたブロックを区別していません。これにより、`alloc`で作られた新しいブロックは`dealloc`でブロックリストに追加されるので、そのサイズのブロックの数は増えることになります。
- 私たちの実装において、新しいブロックが作られる唯一の場所は`alloc`メソッドです。つまり、最初は空のブロックリストから始めて、それらのブロックサイズの割り当てが行われたときに初めてリストを埋めていくということです。
- `alloc`と`dealloc`で`unsafe`な操作を行っていますが、`unsafe`ブロックは必要ありません。これは、Rustは現在unsafeな関数の中身全体を大きな`unsafe`ブロックとして扱っているからです。明示的に`unsafe`ブロックを使うと、どの操作がunsafeなのかそうでないのかが明白になるという利点があるので、この挙動を変更する[RFCが提案](https://github.com/rust-lang/rfcs/pull/2585)されています。

### 使う

私たちが今作った`FixedSizeBlockAllocator`を使うには、`allocator`モジュールの`ALLOCATOR`静的変数を更新する必要があります：

```rust
// in src/allocator.rs

use fixed_size_block::FixedSizeBlockAllocator;

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(
    FixedSizeBlockAllocator::new());
```

`init`関数は、私たちの実装してきたすべてのアロケータで同じように振る舞うので、`init_heap`内における`init`関数の呼び出しを修正する必要はありません。


`heap_allocation`テストをもう一度実行すると、すべてのテストが変わらずパスしているはずです：

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

私たちの新しいアロケータはうまく動いてるみたいですね！

### 議論

固定サイズブロック方式は連結リスト方式よりはるかに優れた性能を持っていますが、（2の累乗をブロックサイズとして使うとき）最大でメモリの半分を無駄にします。このトレードオフに価値があるかは、行われる割り当ての種類に大きく依存します。オペレーティングシステムのカーネルについては、性能が非常に重要なので、固定サイズブロック方式はよりよい選択であるように思われます。

実装の面では、現在の実装には様々な改善可能な箇所があります。

- ブロックが必要になってから代替アロケータで割り当てる代わりに、リストを事前に埋めておき最初の割り当ての性能を向上させる方が良いかもしれません。
- 実装を簡単にするため、2の累乗のブロックサイズのみを許すことで、ブロックサイズをアラインメントとしても使えるようにしました。アラインメントを別のやり方で格納する（もしくは計算する）ことで、任意の他のブロックサイズを使うこともできるでしょう。こうすると、より多くのブロックサイズ（例えば、よくある割り当てサイズのもの）を追加でき、無駄になるメモリを最小化できます。
- 現在、新しいブロックを作ることはしますが、それらを解放することは行っていません。これは断片化につながり、最終的には巨大な割り当ての失敗につながるかもしれません。それぞれのブロックサイズの最大リスト長を制限する方が良いかもしれません。最大長に達すると、その後の割り当て解除はリストに加える代わりに代替アロケータを使って解放するようにします。
- 4KiB以上の割り当てについて、連結リストアロケータで代替するかわりに特別なアロケータを使うことが考えられます。発想としては、4KiBのページの上で動作する仕組みである[ページング][paging]を利用し、連続した仮想メモリのブロックを非連続な物理フレームへと対応づけるのです。こうすると、巨大な割り当てに関する未使用メモリの断片化はもはや問題ではなくなります。
- この「ページアロケータ」があるなら、ブロックサイズを4KiBまで増やし、連結リストアロケータはなくしてしまっても良いかもしれません。このやり方の利点は、断片化が少なくなり、性能の予測性が高まる──つまり、最悪の場合の性能がより良くなる──ことです。

[paging]: @/edition-2/posts/08-paging-introduction/index.ja.md

上で述べた実装の改善点は、あくまで提案に過ぎないということを忘れないでください。オペレーティングシステムのアロケータは、概してカーネル特有の作業のために高度に最適化されていますが、これは詳細なプロファイリングをしてこそ可能になるものなのです。

### 亜種

また、固定サイズブロックアロケータの設計には多くの亜種があります。有名な例として**スラブアロケータ**と**バディアロケータ**の二つがあり、これらはLinuxのような有名なカーネルにおいても使われています。以下では、これらの二つの設計を軽く紹介します。

#### スラブアロケータ

[スラブアロケータ][slab allocator]の発想は、カーネルで使われる型をいくつか選び、それらに直接対応するブロックサイズを使うというものです。こうすると、それらの型の割り当てサイズはブロックサイズに完全に一致するので、メモリは一切無駄になりません。時には、未使用ブロック内の型インスタンスを事前初期化することでさらに性能を向上させられるかもしれません。

[slab allocator]: https://en.wikipedia.org/wiki/Slab_allocation

スラブアロケータはしばしば他のアロケータと組み合わせて使われます。例えば、固定サイズブロックアロケータと組み合わせて、割り当てられたブロックをさらに分割しメモリの無駄を減らすことができます。一つの巨大な割り当ての上で[オブジェクトプール][object pool pattern]を実装するのにもよく使われます。

[object pool pattern]: https://en.wikipedia.org/wiki/Object_pool_pattern

#### バディアロケータ

[バディアロケータ][buddy allocator]では、解放されたブロックの管理に連結リストを使う代わりに、[二分木][binary tree]を使い、ブロックサイズを2の累乗にします。あるサイズの新しいブロックが必要になったら、より大きいサイズのブロックを二つに割り、木に二つの子ノードを作ります。ブロックが解放されたときは毎回、木での隣のブロックを調べます。もし隣も解放されているなら、二つのブロックを合わせて二倍の大きさのブロックに戻します。

この合体ステップのおかげで、[外部断片化][external fragmentation]が少なくなり、解放されたブロックが大きな割り当てに再利用できます。代替アロケータも使わないので、性能の予測可能性も高まります。最大の問題は、2の累乗のブロックサイズしか使えないので、大量のメモリが[内部断片化][internal fragmentation]で無駄になるかもしれないことです。このためバディアロケータはしばしば、割り当てたブロックをより小さな複数のブロックに分割するスラブアロケータと組み合わせて使われます。

[buddy allocator]: https://en.wikipedia.org/wiki/Buddy_memory_allocation
[binary tree]: https://en.wikipedia.org/wiki/Binary_tree
[external fragmentation]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#External_fragmentation
[internal fragmentation]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#Internal_fragmentation


## まとめ

この記事では様々なアロケータの設計を概観しました。一つの`next`ポインタを増やしていくことでメモリを線形に渡していく、基本の[バンプアロケータ][bump allocator]の実装を学びました。バンプアロケータはとても速いですが、割り当てがすべて解放されてからでないとメモリを再利用できません。そのため、グローバルアロケータとして使われることはまれです。

[bump allocator]: @/edition-2/posts/11-allocator-designs/index.ja.md#banpuaroketa

次に、解放されたメモリブロック自体を使って[フリーリスト][free list]と呼ばれる連結リストを作る[連結リストアロケータ][linked list allocator]を作りました。このリストによって、さまざまなサイズ・任意の数の解放されたブロックを格納することができます。この手法は、メモリが一切無駄にならない一方、割り当ての要求によってリスト全体を走査する必要が出てくる可能性があり、性能が悪いです。私たちの実装では、隣接する解放されたブロックを結合することをしていないので、[外部断片化][external fragmentation]も起きてしまいます。

[linked list allocator]: @/edition-2/posts/11-allocator-designs/index.ja.md#lian-jie-rinkuto-risutoaroketa
[free list]: https://en.wikipedia.org/wiki/Free_list

連結リスト方式の性能の問題を解決するため、決められたブロックサイズの集合を事前に定義しておく[固定サイズブロックアロケータ][fixed-size block allocator]を作りました。ブロックサイズごとに別々の[フリーリスト][free list]が存在するので、割り当て・割り当て解除はリストの先頭で挿入・取り出しを行えば良いだけになり、非常に速いです。それぞれの割り当てはそれより大きなブロックサイズに丸められるので、[内部断片化][internal fragmentation]によっていくらかのメモリが無駄になります。

[fixed-size block allocator]: @/edition-2/posts/11-allocator-designs/index.ja.md#gu-ding-saizuburotukuaroketa

アロケータの設計はもっとたくさんあり、それぞれ異なるトレードオフがあります。[スラブアロケータ][Slab allocation]はよくある固定サイズの構造の割り当てをうまく最適化できますが、どのような状況でも使えるとは限りません。[バディアロケータ][Buddy allocation]は二分木を使って解放されたブロックを結合し直しますが、2の累乗のブロックサイズしか使えないので、大量のメモリを無駄にしてしまいます。また、カーネルの実装ごとに行う作業の内容は違うので、どんな状況にも対応できる「最強の」アロケータの設計などないということを覚えておくのが大事です。

[Slab allocation]: @/edition-2/posts/11-allocator-designs/index.ja.md#surabuaroketa
[Buddy allocation]: @/edition-2/posts/11-allocator-designs/index.ja.md#badeiaroketa


## 次は？

この記事で、メモリ管理の実装に関してはいったん終わりとします。次は[**マルチタスク**][_multitasking_]について、手始めに[**async/await**][_async/await_]の形を取った協調的マルチタスクから学んでいきます。その後の記事で、[**スレッド**][_threads_]、[**マルチプロセス**][_multiprocessing_]、[**プロセス**][_processes_]についても学びます。

[_multitasking_]: https://en.wikipedia.org/wiki/Computer_multitasking
[_threads_]: https://en.wikipedia.org/wiki/Thread_(computing)
[_processes_]: https://en.wikipedia.org/wiki/Process_(computing)
[_multiprocessing_]: https://en.wikipedia.org/wiki/Multiprocessing
[_async/await_]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html
