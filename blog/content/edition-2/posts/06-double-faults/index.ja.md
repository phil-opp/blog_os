+++
title = "Double Faults"
weight = 6
path = "ja/double-fault-exceptions"
date  = 2018-06-18

[extra]
chapter = "Interrupts"
# Please update this when updating the translation
translation_based_on_commit = "27ac0e1acc36f640d7045b427da2ed65b945756b"
# GitHub usernames of the people that translated this post
translators = ["garasubo"]
+++

この記事ではCPUが例外ハンドラの呼び出しに失敗したときに起きる、ダブルフォルト例外について詳細に見ていきます。この例外をハンドルすることによって、システムリセットを起こす重大な**トリプルフォルト**を避けることができます。あらゆる場合においてトリプルフォルトを防ぐにはダブルフォルトを別にカーネルスタック上でキャッチするために**Interrup Stack Table**をセットアップする必要があります。

<!-- more -->

このブログの内容は [GitHub] 上で公開・開発されています。何か問題や質問などがあれば issue をたててください (訳注: リンクは原文(英語)のものになります)。また[こちら][at the bottom]にコメントを残すこともできます。この記事の完全なソースコードは[`post-06` ブランチ][post branch]にあります。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-06

<!-- toc -->

## ダブルフォルトとは
簡単に言うとダブルフォルトとはCPUが例外ハンドラを呼び出すことに失敗したときに起きる特別な例外です。例えば、ページフォルトが起きたが、ページフォルトハンドラが[Interrupt Descriptor Table][IDT] (IDT)に登録されていないときに発生します。つまり、C++での`catch(...)`やJavaやC#の`catch(Exception e)`ような、例外のあるプログラミング言語のcatch-allブロックのようなものです。

[IDT]: @/edition-2/posts/05-cpu-exceptions/index.md#the-interrupt-descriptor-table

ダブルフォルトは通常の例外のように振る舞います。ベクター番号`8`を持ち、IDTに通常のハンドラ関数として定義できます。ダブルフォルトがハンドルされないと、重大な_トリプルフォルト_が起きてしまうため、ダブルフォルトハンドラを設定するのはとても重要です。トリプルフォルトはキャッチすることができず、ほとんどのハードウェアはシステムリセットを起こします。

### ダブルフォルトを起こす
ハンドラ関数を定義していない例外を発生させることでダブルフォルトを起こしてみましょう。

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    // trigger a page fault
    unsafe {
        *(0xdeadbeef as *mut u64) = 42;
    };

    // as before
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    loop {}
}
```

不正なアドレスである`0xdeadbeef`に書き込みを行うため`unsafe`を使います。この仮想アドレスはページテーブル上で物理アドレスにマップされていないため、ページフォルトが発生します。私達の[IDT]にはページフォルトが登録されていないため、ダブルフォルトが発生します。

今、私達のカーネルをスタートさせると、ブートが無限に繰り返されるのがわかります。このブートループの理由は以下の通りです

1. CPUが`0xdeadbeef`に書き込みを試みページフォルトを起こします
2. CPUはIDTに対応するエントリを探しに行き、ハンドラ関数が指定されていないとわかります。結果、ページフォルトハンドラが呼び出せず、ダブルフォルトが発生します
3. CPUはダブルフォルトダブルフォルトハンドラのIDTエントリを見にいきますが、このエントリもハンドラ関数を指定していません。結果、_トリプルフォルト_が発生します
4. トリプルフォルトは重大なエラーなので、QEMUはほとんどの実際のハードウェアと同様にシステムリセットを発行します

このトリプルフォルトを防ぐためには、ページフォルトかダブルフォルトのハンドラ関数を定義しないといけません。私達はすべての場合におけるトリプルフォルトを防ぎたいので、すべてのハンドルされていない例外のタイプで呼び出されるダブルフォルトハンドラを定義するところからはじめましょう。

## ダブルフォルトハンドラ
ダブルフォルトは通常のエラーコードのある例外なので、ブレークポイントハンドラと同じようにハンドラ関数を指定することができます。

```rust
// in src/interrupts.rs

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler); // new
        idt
    };
}

// new
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame, _error_code: u64) -> !
{
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}
```

私達のハンドラは短いエラーメッセージを出力して、例外スタックフレームをダンプします。ダブルフォルトハンドラのエラーコードは常に0なので、プリントすることはないでしょう。ブレークポイントハンドラとの一つの違いは、ダブルフォルトハンドラは[**発散する**]ことです。なぜかというと、`x86_64`アーキテクチャではダブルフォルト例外から復帰するすることは許されていないからです。

[**発散する**]: https://doc.rust-lang.org/stable/rust-by-example/fn/diverging.html

ここで私達のカーネルをスタートさせると、ダブルフォルトハンドラが呼び出されていることがわかることでしょう。

![QEMU printing `EXCEPTION: DOUBLE FAULT` and the exception stack frame](qemu-catch-double-fault.png)

動きました！ここで何が起きているかというと、

1. CPUが`0xdeadbeef`に書き込もうとして、ページフォルトが起きる
2. 以前と同様に、CPUはIDT中の対応するエントリを見にいくが、ハンドラ関数が定義されていないことがわかり、結果、ダブルフォルトが起きる
3. CPUは、今は存在ている、ダブルフォルトハンドラにジャンプする

CPUはダブルフォルトハンドラを呼べるようになったので、トリプルフォルト（と起動ループ）はもう起こりません。

ここまでは簡単です。なんでこの話題のためにポストが必要だったのでしょうか？実は、私達は_ほとんどの_ダブルフォルトをキャッチすることはできますが、このアプローチでは十分でないケースが存在するのです。

## ダブルフォルトの原因
特別な場合を見にいく前に、ダブルフォルトの正確な原因を知る必要があります。ここまで、私達はとてもあいまいな定義を使ってきました。

> ダブルフォルトとはCPUが例外ハンドラを呼び出すことに失敗したときに起きる特別な例外です。

**「呼び出すことに失敗する」**とは正確には何をいみするのでしょうか？ハンドラが存在しない？ハンドラが[スワップアウト]された？また、ハンドラそのものが例外を発生させたらどうなるのでしょうか？

[スワップアウト]: http://pages.cs.wisc.edu/~remzi/OSTEP/vm-beyondphys.pdf

例えば以下のようなことがおこったらどうでしょう？

1. ブレークポイント例外が発生したが、対応するハンドラがスワップアウトされていたら？
2. ページフォルトが発生がしたが、ページフォルトハンドラがスワップアウトされていたら？
3. ゼロ除算ハンドラがブレークポイント例外を発生したが、ブレークポイントハンドラがスワップアウトされていたら？
4. カーネルがスタックをオーバーフローさせて_ガードページ_にヒットしたら？

Fortunately, the AMD64 manual ([PDF][AMD64 manual]) has an exact definition (in Section 8.2.9). According to it, a “double fault exception _can_ occur when a second exception occurs during the handling of a prior (first) exception handler”. The _“can”_ is important: Only very specific combinations of exceptions lead to a double fault. These combinations are:
幸いにもAMD64のマニュアル（[PDF][AMD64 manual]）には正確な定義が書かれています（8.2.9章）。それによると「ダブルフォルト例外は直前の（一度目の）例外ハンドラの処理中に二度目の例外が発生したとき**起きうる**」と書かれています。**起きうる**というのが重要で、とても特別な例外の組み合わせでのみダブルフォルトとなります。この組み合わせは以下のようになっています

最初の例外 | 二度目の例外
----------------|-----------------
[ゼロ除算],<br>[無効TSS],<br>[セグメント不在],<br>[スタックセグメントフォルト],<br>[一般保護例外] | [無効TSS],<br>[セグメント不在],<br>[スタックセグメントフォルト],<br>[一般保護例外]
[ページフォルト] | [ページフォルト],<br>[無効TSS],<br>[セグメント不在],<br>[スタックセグメントフォルト],<br>[一般保護例外]

[ゼロ除算]: https://wiki.osdev.org/Exceptions#Divide-by-zero_Error
[無効TSS]: https://wiki.osdev.org/Exceptions#Invalid_TSS
[セグメント不在]: https://wiki.osdev.org/Exceptions#Segment_Not_Present
[スタックセグメントフォルト]: https://wiki.osdev.org/Exceptions#Stack-Segment_Fault
[一般保護例外]: https://wiki.osdev.org/Exceptions#General_Protection_Fault
[ページフォルト]: https://wiki.osdev.org/Exceptions#Page_Fault


[AMD64 manual]: https://www.amd.com/system/files/TechDocs/24593.pdf

例えばページフォルトに続いてゼロ除算例外が起きた場合は問題ない（ページフォルトハンドラが呼び出される）が、一般保護例外に続いてゼロ除算例外が起きた場合はダブルフォルトが起きます。

この表を見れば、先程の質問のうち最初の３つに答えることができます。

1. ブレークポイント例外が発生して、対応するハンドラ関数がスワップアウトされている場合、**ページフォルト**が発生して**ページフォルトハンドラ**が呼び出される
2. ページフォルトが発生してページフォルトハンドラがスワップアウトされている場合、**ダブルフォルト**が発生してダブルフォルトハンドラが呼び出される
3. ゼロ除算ハンドラがブレークポイント例外を発生させた場合、CPUはブレークポイントハンドラを呼び出そうとする。もしブレークポイントハンドラがスワップアウトされている場合、**ページフォルト**が発生して**ページフォルトハンドラ**が呼び出される

実際、IDTにハンドラ関数ないときの例外の場合はこの体系に従っています。つまり、例外が発生したとき、CPUは対応するIDTエントリを読み込みにいきます。このエントリは0のため正しいIDTエントリではないので、_一般保護例外_が発生します。私達は一般保護例外のハンドラも定義していないので、新たな一般保護例外が発生します。表によるとこれはダブルフォルトを起こします。

### カーネルスタックオーバーフロー
４つ目の質問を見てみましょう

> カーネルがスタックをオーバーフローさせてガードページにヒットしたら？

ガードページはスタックの底にある特別なメモリページで、これによってスタックオーバーフローを検出することができます。このページはどの物理メモリにもマップされていないので、アクセスすることで静かに他のメモリを破壊するのではなくページフォルトが発生します。ブートローダーはカーネルスタックのためにガードページをセットアップするので、スタックオーバーフローが起きると**ページフォルト**が起きます。

ページフォルトが起きるととCPUはIDT内のページフォルトハンドラを探しにいき、[割り込みスタックフレーム]をスタック上にプッシュします。しかし、現在のスタックポインタはすでに存在しないガードページを指しています。結果、二度目のページフォルトが発生して、ダブルフォルトが起きます（上の表によれば）。

[割り込みスタックフレーム]: @/edition-2/posts/05-cpu-exceptions/index.md#the-interrupt-stack-frame

つまりここでCPUは**ダブルフォルトハンドラ**を呼びにいきます。しかし、ダブルフォルトでもCPUは例外スタックフレームをプッシュします。スタックポインタはまだガードページを指しているので、**三度目の**ページフォルトが起きて、**トリプルフォルト**を発生させシステムは再起動します。そのため、私達の今のダブルフォルトハンドラではこの場合でのトリプルフォルトを避けることができません。

実際にやってみましょう。カーネルスタックオーバーフローは無限に再帰する関数を呼び出すことによって簡単に引き起こせます。

```rust
// in src/main.rs

#[no_mangle] // この関数の名前修飾をしない
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    fn stack_overflow() {
        stack_overflow(); // 再帰呼び出しのために、リターンアドレスがプッシュされる
    }

    // スタックオーバーフローを起こす
    stack_overflow();

    […] // test_main(), println(…), and loop {}
}
```

これをQEMUで試すと、再び起動ループに入るのがわかります。

ではどうやったら私達はこの問題を避けられるでしょうか？例外スタックフレームをプッシュすることは、CPU自身が行ってしまうので、取り除くことはできません。つまりどうにかしてダブルフォルト例外が発生したときスタックが常に正常であることを確かにする必要があります。幸いにもx86_64アーキテクチャにはこの問題の解決策を持っています。

## Switching Stacks
## スタックを切り替える
The x86_64 architecture is able to switch to a predefined, known-good stack when an exception occurs. This switch happens at hardware level, so it can be performed before the CPU pushes the exception stack frame.
x86_64アーキテクチャは例外発生時に予め定義されている既知の正常なスタックに切り替えることができます。この切り替えはハードウェアレベルで発生するので、CPUが例外スタックフレームをプッシュする前に行うことができます。

The switching mechanism is implemented as an _Interrupt Stack Table_ (IST). The IST is a table of 7 pointers to known-good stacks. In Rust-like pseudo code:
切り替えの仕組みは**割り込みスタックテーブル**（IST）として実装されています。ISTは７つの既知の正常なポインタのテーブルです。Rust風の疑似コードで表すとこのようになります。

```rust
struct InterruptStackTable {
    stack_pointers: [Option<StackPointer>; 7],
}
```

For each exception handler, we can choose a stack from the IST through the `stack_pointers` field in the corresponding [IDT entry]. For example, we could use the first stack in the IST for our double fault handler. Then the CPU would automatically switch to this stack whenever a double fault occurs. This switch would happen before anything is pushed, so it would prevent the triple fault.
各例外ハンドラに対して、私達は対応する[IDTエントリ]の`stack_pointers`フィールドによってスタックをISTから選ぶことができます。例えば、IST中の最初のスタックをダブルフォルトハンドラのために使うことができます。

[IDTエントリ]: @/edition-2/posts/05-cpu-exceptions/index.md#the-interrupt-descriptor-table

### The IST and TSS
### ISTとTSS
The Interrupt Stack Table (IST) is part of an old legacy structure called _[Task State Segment]_ \(TSS). The TSS used to hold various information (e.g. processor register state) about a task in 32-bit mode and was for example used for [hardware context switching]. However, hardware context switching is no longer supported in 64-bit mode and the format of the TSS changed completely.
割り込みスタックテーブル（IST）は**[テーブルステートセグメント]**（TSS）という古いレガシーな構造体の一部です。TSSはかつては様々な32ビットモードでのタスクに関する情報（例：プロセッサのレジスタの状態）を保持していて、例えば[ハードウェアコンテキストスイッチング]に使われていました。しかし、ハードウェアコンテキストスイッチングは64ビットではサポートされなくなり、TSSのフォーマットは完全に変わりました。

[タスクステートセグメント]: https://en.wikipedia.org/wiki/Task_state_segment
[ハードウェアコンテキストスイッチング]: https://wiki.osdev.org/Context_Switching#Hardware_Context_Switching

On x86_64, the TSS no longer holds any task specific information at all. Instead, it holds two stack tables (the IST is one of them). The only common field between the 32-bit and 64-bit TSS is the pointer to the [I/O port permissions bitmap].
x86_64ではTSSはタスク固有の情報は全く持たなくなりました。代わりに、２つのスタックテーブル（ISTがその１つ）を持つようになりました。唯一32ビットと64ビットのTSSで共通のフィールドは[I/Oポート権限ビットマップ]へのポインタのみです。

[I/Oポート権限ビットマップ]: https://en.wikipedia.org/wiki/Task_state_segment#I.2FO_port_permissions

The 64-bit TSS has the following format:
64ビットのTSSは下記のようなフォーマットです。

Field  | Type
------ | ----------------
<span style="opacity: 0.5">(reserved)</span> | `u32`
Privilege Stack Table | `[u64; 3]`
<span style="opacity: 0.5">(reserved)</span> | `u64`
Interrupt Stack Table | `[u64; 7]`
<span style="opacity: 0.5">(reserved)</span> | `u64`
<span style="opacity: 0.5">(reserved)</span> | `u16`
I/O Map Base Address | `u16`

The _Privilege Stack Table_ is used by the CPU when the privilege level changes. For example, if an exception occurs while the CPU is in user mode (privilege level 3), the CPU normally switches to kernel mode (privilege level 0) before invoking the exception handler. In that case, the CPU would switch to the 0th stack in the Privilege Stack Table (since 0 is the target privilege level). We don't have any user mode programs yet, so we ignore this table for now.
**特権スタックテーブル**は特権レベルが変わったときにCPUに使われます。例えば、CPUがユーザーモード（特権レベル3）の時に例外が発生した場合、CPUは通常は例外ハンドラを呼び出す前にカーネルモード（特権レベル0）に切り替わります。この場合、CPUは特権レベルスタックテーブルの0番目のスタックに切り替わります。

### Creating a TSS
### TSSをつくる
Let's create a new TSS that contains a separate double fault stack in its interrupt stack table. For that we need a TSS struct. Fortunately, the `x86_64` crate already contains a [`TaskStateSegment` struct] that we can use.
割り込みスタックテーブルにダブルフォルト用の別のスタックを含めた新しいTSSをつくってみましょう。そのためにはTSS構造体が必要です。幸いにも、`x86_64`クレートにすでに[`TaskStateSegment`構造体]は含まれているので、これを使うことができます。

[`TaskStateSegment`構造体]: https://docs.rs/x86_64/0.12.1/x86_64/structures/tss/struct.TaskStateSegment.html

We create the TSS in a new `gdt` module (the name will make sense later):
新しい`gdt`モジュール内でTSSをつくります（名前の意味は後でわかるでしょう）。

```rust
// in src/lib.rs

pub mod gdt;

// in src/gdt.rs

use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;
use lazy_static::lazy_static;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}
```

We use `lazy_static` because Rust's const evaluator is not yet powerful enough to do this initialization at compile time. We define that the 0th IST entry is the double fault stack (any other IST index would work too). Then we write the top address of a double fault stack to the 0th entry. We write the top address because stacks on x86 grow downwards, i.e. from high addresses to low addresses.
Rustの定数評価機はこの初期化をコンパイル時に行うことがまだできないので`lazy_static`を使います。私達は0番目のISTエントリはダブルフォルト用のスタックだと定義します（他のISTのインデックスでも動くでしょう）。
そして、ダブルフォルト用スタックの先頭アドレスを0番目のエントリに書き込みます。先頭アドレスを書き込むのはx86のスタックは下、つまり高いアドレスから低いアドレスに向かって伸びていくからです。

We haven't implemented memory management yet, so we don't have a proper way to allocate a new stack. Instead, we use a `static mut` array as stack storage for now. The `unsafe` is required because the compiler can't guarantee race freedom when mutable statics are accessed. It is important that it is a `static mut` and not an immutable `static`, because otherwise the bootloader will map it to a read-only page. We will replace this with a proper stack allocation in a later post, then the `unsafe` will be no longer needed at this place.
私達はまだメモリ管理を実装していません。そのため、新しいスタックを確保する適切な方法がありません。その代わり今回は、スタックのストレージとして`static mut`な配列を使います。`unsafe`はコンパイラが変更可能な静的変数がアクセスされるとき競合がないことを保証できないため必要です。これが不変の`static`ではなく`static mut`であることは重要です。そうでなければブートローダーはこれをリードオンリーのページにマップしてしまうからです。私達は後の記事でこの部分を適切なスタック確保に置き換えます。そうしたらこの部分での`unsafe`は必要なくなります。

Note that this double fault stack has no guard page that protects against stack overflow. This means that we should not do anything stack intensive in our double fault handler because a stack overflow might corrupt the memory below the stack.
ちなみに、このダブルフォルトスタックはスタックオーバーフローに対する保護をするガードページを持ちません。これはつまり、スタックオーバーフローがスタックより下のメモリと衝突するかもしれないので、私達はダブルフォルトハンドラ内でスタックを多く使うようなことをするべきではないということです。

#### Loading the TSS
#### TSSを読み込む
Now that we created a new TSS, we need a way to tell the CPU that it should use it. Unfortunately this is a bit cumbersome, since the TSS uses the segmentation system (for historical reasons). Instead of loading the table directly, we need to add a new segment descriptor to the [Global Descriptor Table] \(GDT). Then we can load our TSS invoking the [`ltr` instruction] with the respective GDT index. (This is the reason why we named our module `gdt`.)
新しいTSSをつくったので、私達はCPUにそれを使うように教える方法が必要です。残念ながら、これはちょっと面倒くさいです。なぜならTSSは（歴史的な理由で）セグメンテーションシステムを使うためです。テーブルを直接読み込むのではなく、新しいセグメントディスクリプタを[Global Descriptor Table]（GDT）に追加する必要があります。そうすると各自のGDTインデックスで[`ltr`命令]を呼び出すことで私達のTSSを読み込むことができます。

[Global Descriptor Table]: https://web.archive.org/web/20190217233448/https://www.flingos.co.uk/docs/reference/Global-Descriptor-Table/
[`ltr`命令]: https://www.felixcloutier.com/x86/ltr

### The Global Descriptor Table
### グローバルディスクリプタテーブル
The Global Descriptor Table (GDT) is a relict that was used for [memory segmentation] before paging became the de facto standard. It is still needed in 64-bit mode for various things such as kernel/user mode configuration or TSS loading.
グローバルディスクリプタテーブル（GDT）はページングがデファクトスタンダードになる以前の[メモリセグメンテーション]のため使われていた遺物です。64ビットモードでもカーネル・ユーザーモードの設定やTSSの読み込みなど様々なことのため未だに必要です。

[メモリセグメンテーション]: https://en.wikipedia.org/wiki/X86_memory_segmentation

The GDT is a structure that contains the _segments_ of the program. It was used on older architectures to isolate programs from each other, before paging became the standard. For more information about segmentation check out the equally named chapter of the free [“Three Easy Pieces” book]. While segmentation is no longer supported in 64-bit mode, the GDT still exists. It is mostly used for two things: Switching between kernel space and user space, and loading a TSS structure.
GDTはプログラムの**セグメント**を含む構造です。ページングが標準になる以前に、プログラム同士を独立させるためにより古いアーキテクチャで使われていました。セグメンテーション

[“Three Easy Pieces” book]: http://pages.cs.wisc.edu/~remzi/OSTEP/

#### Creating a GDT
Let's create a static `GDT` that includes a segment for our `TSS` static:

```rust
// in src/gdt.rs

use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor};

lazy_static! {
    static ref GDT: GlobalDescriptorTable = {
        let mut gdt = GlobalDescriptorTable::new();
        gdt.add_entry(Descriptor::kernel_code_segment());
        gdt.add_entry(Descriptor::tss_segment(&TSS));
        gdt
    };
}
```

As before, we use `lazy_static` again. We create a new GDT with a code segment and a TSS segment.

#### Loading the GDT

To load our GDT we create a new `gdt::init` function, that we call from our `init` function:

```rust
// in src/gdt.rs

pub fn init() {
    GDT.load();
}

// in src/lib.rs

pub fn init() {
    gdt::init();
    interrupts::init_idt();
}
```

Now our GDT is loaded (since the `_start` function calls `init`), but we still see the boot loop on stack overflow.

### The final Steps

The problem is that the GDT segments are not yet active because the segment and TSS registers still contain the values from the old GDT. We also need to modify the double fault IDT entry so that it uses the new stack.

In summary, we need to do the following:

1. **Reload code segment register**: We changed our GDT, so we should reload `cs`, the code segment register. This is required since the old segment selector could point a different GDT descriptor now (e.g. a TSS descriptor).
2. **Load the TSS** : We loaded a GDT that contains a TSS selector, but we still need to tell the CPU that it should use that TSS.
3. **Update the IDT entry**: As soon as our TSS is loaded, the CPU has access to a valid interrupt stack table (IST). Then we can tell the CPU that it should use our new double fault stack by modifying our double fault IDT entry.

For the first two steps, we need access to the `code_selector` and `tss_selector` variables in our `gdt::init` function. We can achieve this by making them part of the static through a new `Selectors` struct:

```rust
// in src/gdt.rs

use x86_64::structures::gdt::SegmentSelector;

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors { code_selector, tss_selector })
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}
```

Now we can use the selectors to reload the `cs` segment register and load our `TSS`:

```rust
// in src/gdt.rs

pub fn init() {
    use x86_64::instructions::segmentation::set_cs;
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        set_cs(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}
```

We reload the code segment register using [`set_cs`] and to load the TSS using [`load_tss`]. The functions are marked as `unsafe`, so we need an `unsafe` block to invoke them. The reason is that it might be possible to break memory safety by loading invalid selectors.

[`set_cs`]: https://docs.rs/x86_64/0.12.1/x86_64/instructions/segmentation/fn.set_cs.html
[`load_tss`]: https://docs.rs/x86_64/0.12.1/x86_64/instructions/tables/fn.load_tss.html

Now that we loaded a valid TSS and interrupt stack table, we can set the stack index for our double fault handler in the IDT:

```rust
// in src/interrupts.rs

use crate::gdt;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX); // new
        }

        idt
    };
}
```

The `set_stack_index` method is unsafe because the the caller must ensure that the used index is valid and not already used for another exception.

That's it! Now the CPU should switch to the double fault stack whenever a double fault occurs. Thus, we are able to catch _all_ double faults, including kernel stack overflows:

![QEMU printing `EXCEPTION: DOUBLE FAULT` and a dump of the exception stack frame](qemu-double-fault-on-stack-overflow.png)

From now on we should never see a triple fault again! To ensure that we don't accidentally break the above, we should add a test for this.

## A Stack Overflow Test

To test our new `gdt` module and ensure that the double fault handler is correctly called on a stack overflow, we can add an integration test. The idea is to do provoke a double fault in the test function and verify that the double fault handler is called.

Let's start with a minimal skeleton:

```rust
// in tests/stack_overflow.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

Like our `panic_handler` test, the test will run [without a test harness]. The reason is that we can't continue execution after a double fault, so more than one test doesn't make sense. To disable, the test harness for the test, we add the following to our `Cargo.toml`:

```toml
# in Cargo.toml

[[test]]
name = "stack_overflow"
harness = false
```

[without a test harness]: @/edition-2/posts/04-testing/index.md#no-harness-tests

Now `cargo test --test stack_overflow` should compile successfully. The test fails of course, since the `unimplemented` macro panics.

### Implementing `_start`

The implementation of the `_start` function looks like this:

```rust
// in tests/stack_overflow.rs

use blog_os::serial_print;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    blog_os::gdt::init();
    init_test_idt();

    // trigger a stack overflow
    stack_overflow();

    panic!("Execution continued after stack overflow");
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow(); // for each recursion, the return address is pushed
    volatile::Volatile::new(0).read(); // prevent tail recursion optimizations
}
```

We call our `gdt::init` function to initialize a new GDT. Instead of calling our `interrupts::init_idt` function, we call a `init_test_idt` function that will be explained in a moment. The reason is that we want to register a custom double fault handler that does a `exit_qemu(QemuExitCode::Success)` instead of panicking.

The `stack_overflow` function is almost identical to the function in our `main.rs`. The only difference is that we do an additional [volatile] read at the end of the function using the [`Volatile`] type to prevent a compiler optimization called [_tail call elimination_]. Among other things, this optimization allows the compiler to transform a function whose last statement is a recursive function call into a normal loop. Thus, no additional stack frame is created for the function call, so that the stack usage does remain constant.

[volatile]: https://en.wikipedia.org/wiki/Volatile_(computer_programming)
[`Volatile`]: https://docs.rs/volatile/0.2.6/volatile/struct.Volatile.html
[_tail call elimination_]: https://en.wikipedia.org/wiki/Tail_call

In our case, however, we want that the stack overflow happens, so we add a dummy volatile read statement at the end of the function, which the compiler is not allowed to remove. Thus, the function is no longer _tail recursive_ and the transformation into a loop is prevented. We also add the `allow(unconditional_recursion)` attribute to silence the compiler warning that the function recurses endlessly.

### The Test IDT

As noted above, the test needs its own IDT with a custom double fault handler. The implementation looks like this:

```rust
// in tests/stack_overflow.rs

use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(blog_os::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}

pub fn init_test_idt() {
    TEST_IDT.load();
}
```

The implementation is very similar to our normal IDT in `interrupts.rs`. Like in the normal IDT, we set a stack index into the IST for the double fault handler in order to switch to a separate stack. The `init_test_idt` function loads the IDT on the CPU through the `load` method.

### The Double Fault Handler

The only missing piece is our double fault handler. It looks like this:

```rust
// in tests/stack_overflow.rs

use blog_os::{exit_qemu, QemuExitCode, serial_println};
use x86_64::structures::idt::InterruptStackFrame;

extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

When the double fault handler is called, we exit QEMU with a success exit code, which marks the test as passed. Since integration tests are completely separate executables, we need to set `#![feature(abi_x86_interrupt)]` attribute again at the top of our test file.

Now we can run our test through `cargo test --test stack_overflow` (or `cargo test` to run all tests). As expected, we see the `stack_overflow... [ok]` output in the console. Try to comment out the `set_stack_index` line: it should cause the test to fail.

## Summary
In this post we learned what a double fault is and under which conditions it occurs. We added a basic double fault handler that prints an error message and added an integration test for it.

We also enabled the hardware supported stack switching on double fault exceptions so that it also works on stack overflow. While implementing it, we learned about the task state segment (TSS), the contained interrupt stack table (IST), and the global descriptor table (GDT), which was used for segmentation on older architectures.

## What's next?
The next post explains how to handle interrupts from external devices such as timers, keyboards, or network controllers. These hardware interrupts are very similar to exceptions, e.g. they are also dispatched through the IDT. However, unlike exceptions, they don't arise directly on the CPU. Instead, an _interrupt controller_ aggregates these interrupts and forwards them to CPU depending on their priority. In the next we will explore the [Intel 8259] \(“PIC”) interrupt controller and learn how to implement keyboard support.

[Intel 8259]: https://en.wikipedia.org/wiki/Intel_8259
