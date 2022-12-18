+++
title = "ヒープ割り当て"
weight = 10
path = "ja/heap-allocation"
date = 2019-06-26

[extra]
chapter = "Memory Management"
# Please update this when updating the translation
translation_based_on_commit = "afeed7477bb19a29d94a96b8b0620fd241b0d55f"
# GitHub usernames of the people that translated this post
translators = ["woodyZootopia", "garasubo"]
+++

この記事では、私たちのカーネルにヒープ<ruby>割り当て<rp> (</rp><rt>アロケーション</rt><rp>) </rp></ruby>の機能を追加します。まず動的メモリの基礎を説明し、どのようにして借用チェッカがありがちなアロケーションエラーを防いでくれるのかを示します。その後Rustの基本的なアロケーションインターフェースを実装し、ヒープメモリ領域を作成し、アロケータクレートを設定します。この記事を終える頃には、Rustに組み込みの`alloc`クレートのすべてのアロケーション・コレクション型が私たちのカーネルで利用可能になっているでしょう。

<!-- more -->

このブログの内容は [GitHub] 上で公開・開発されています。何か問題や質問などがあれば issue をたててください (訳注: リンクは原文(英語)のものになります)。また[こちら][at the bottom]にコメントを残すこともできます。この記事の完全なソースコードは[`post-10` ブランチ][post branch]にあります。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-10

<!-- toc -->

## <ruby>局所<rp> (</rp><rt>ローカル</rt><rp>) </rp></ruby>変数と<ruby>静的<rp> (</rp><rt>スタティック</rt><rp>) </rp></ruby>変数

私たちのカーネルでは現在二種類の変数が使用されています：局所変数と`static`変数です。局所変数は[コールスタック][call stack]に格納されており、変数の定義された関数がリターンするまでの間のみ有効です。静的変数はメモリ上の固定された場所に格納されており、プログラムのライフタイム全体で常に生存しています。

### 局所変数

局所変数は[コールスタック][call stack]に格納されています。これはプッシュ (`push`) とポップ (`pop`) という命令をサポートする[スタックというデータ構造][stack data structure]です。関数に入るたびに、パラメータ、リターンアドレス、呼び出された関数の局所変数がコンパイラによってプッシュされます：

[call stack]: https://ja.wikipedia.org/wiki/%E3%82%B3%E3%83%BC%E3%83%AB%E3%82%B9%E3%82%BF%E3%83%83%E3%82%AF
[stack data structure]: https://ja.wikipedia.org/wiki/%E3%82%B9%E3%82%BF%E3%83%83%E3%82%AF

![outer()とinner(i: usize)関数。両方が局所変数を持っています。outerはinner(1)を呼びます。コールスタックには順に以下の領域があります：outerの局所変数、引数i=1、リターンアドレス、そしてinnerの局所変数。](call-stack.svg)

上の例は、`outer`関数が`inner`関数を呼び出した後のコールスタックを示しています。コールスタックは`outer`の局所変数を先に持っていることが分かります。`inner`を呼び出すと、パラメータ`1`とこの関数のリターンアドレスがプッシュされます。そこで制御は`inner`へと移り、`inner`は自身の局所変数をプッシュします。

`inner`関数がリターンすると、コールスタックのこの関数に対応する部分がポップされ、`outer`の局所変数のみが残ります：

![outerの局所変数しか持っていないコールスタック](call-stack-return.svg)

`inner`関数の局所変数はリターンまでしか生存していないことが分かります。Rustコンパイラはこの<ruby>生存期間<rp> (</rp><rt>ライフタイム</rt><rp>) </rp></ruby>を強制し、私たちが値を長く使いすぎてしまうとエラーを投げます。例えば、局所変数への参照を返そうとしたときがそうです：

```rust
fn inner(i: usize) -> &'static u32 {
    let z = [1, 2, 3];
    &z[i]
}
```

([この例をplaygroundで実行する](https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=6186a0f3a54f468e1de8894996d12819))

上の例の場合、参照を返すことには意味がありませんが、変数に関数よりも長く生存して欲しいというケースは存在します。すでに私たちのカーネルでそのようなケースに遭遇しています。それは[割り込み記述子表 (IDT) を読み込][load an interrupt descriptor table]もうとしたときで、ライフタイムを延ばすために`static`変数を使う必要がありました。

[load an interrupt descriptor table]: @/edition-2/posts/05-cpu-exceptions/index.ja.md#idtwodu-miip-mu

### 静的変数

静的変数は、スタックとは別の固定されたメモリ位置に格納されます。このメモリ位置はコンパイル時にリンカによって指定され、実行可能ファイルにエンコードされています。静的変数はプログラムの実行中ずっと生存するため、`'static`ライフタイムを持っており、局所変数によっていつでも参照することができます。

![同じouter/innerの例ですが、innerが`static Z: [u32; 3] = [1,2,3];`を持っており、参照`&Z[i]`を返します](call-stack-static.svg)

上の例で`inner`関数がリターンするとき、それに対応するコールスタックは破棄されます。（しかし）静的変数は絶対に破棄されない別のメモリ領域にあるため、参照`&Z[1]`はリターン後も有効です。

`'static`ライフタイムの他にも静的変数には利点があります。それらは位置がコンパイル時に分かるため、アクセスするために参照が必要ないのです。この特性を私たちの`println`マクロを作る際に利用しました：[静的な`Writer`][static `Writer`]をその内部で使うことで、マクロを呼び出す際に`&mut Writer`参照が必要でなくなります。これは他の変数にアクセスできない[例外処理関数][exception handlers]においてとても有用です。

[static `Writer`]: @/edition-2/posts/03-vga-text-buffer/index.ja.md#da-yu-de-global-naintahuesu
[exception handlers]: @/edition-2/posts/05-cpu-exceptions/index.ja.md#shi-zhuang

しかし、静的変数のこの特性には重大な欠点がついてきます：デフォルトでは読み込み専用なのです。Rustがこのルールを強制するのは、例えば二つのスレッドがある静的変数を同時に変更した場合[データ競合][data race]が発生するためです。静的変数を変更する唯一の方法は、それを[`Mutex`]型にカプセル化し、あらゆる時刻において`&mut`参照が一つしか存在しないことを保証することです。`Mutex`は[VGAバッファへの静的な`Writer`][vga mutex]を作ったときにすでに使いました。

[data race]: https://doc.rust-jp.rs/rust-nomicon-ja/races.html
[`Mutex`]: https://docs.rs/spin/0.5.2/spin/struct.Mutex.html
[vga mutex]: @/edition-2/posts/03-vga-text-buffer/index.ja.md#supinrotuku

## <ruby>動的<rp> (</rp><rt>ダイナミック</rt><rp>) </rp></ruby>メモリ

局所変数と静的変数を組み合わせれば、それら自体とても強力であり、ほとんどのユースケースを満足します。しかし、どちらにも制限が存在することも見てきました：

- 局所変数はそれを定義する関数やブロックが終わるまでしか生存しません。なぜなら、これらはコールスタックに存在し、関数がリターンした段階で破棄されるからです。
- 静的変数はプログラムの実行中常に生存するため、必要なくなったときでもメモリを取り戻したり再利用したりする方法がありません。また、所有権のセマンティクスが不明瞭であり、すべての関数からアクセスできてしまうため、変更しようと思ったときには[`Mutex`]で保護してやらないといけません。

局所変数・静的変数の制約としてもう一つ、固定サイズであることが挙げられます。従ってこれらは要素が追加されたときに動的に大きくなるコレクションを格納することができません（Rustにおいて動的サイズの局所変数を可能にする[unsized rvalues]の提案が行われていますが、これはいくつかの特定のケースでしかうまく動きません）。

[unsized rvalues]: https://github.com/rust-lang/rust/issues/48055

これらの欠点を回避するために、プログラミング言語はしばしば、変数を格納するための第三の領域である**ヒープ**をサポートします。ヒープは、`allocate`と`deallocate`という二つの関数を通じて、実行時の**動的メモリ割り当て**をサポートします。仕組みとしては以下のようになります：`allocate`関数は、変数を格納するのに使える、指定されたサイズの解放されたメモリの塊を返します。変数への参照を引数に`deallocate`関数を呼び出すことによってその変数を解放するまで、この変数は生存します。

例を使って見てみましょう：

![inner関数は`allocate(size_of([u32; 3]))`を呼び、`z.write([1,2,3]);`で書き込みを行い、`(z as *mut u32).offset(i)`を返します。outer関数は返された値`y`に対して`deallocate(y, size_of(u32))`を行います。](call-stack-heap.svg)

ここで`inner`関数は`z`を格納するために静的変数ではなくヒープメモリを使っています。まず要求されたサイズのメモリブロックを割り当て、`*mut u32`の[生ポインタ][raw pointer]を受け取ります。その後で[`ptr::write`]メソッドを使ってこれに配列`[1,2,3]`を書き込みます。最後のステップとして、[`offset`]関数を使って`i`番目の要素へのポインタを計算しそれを返します（簡単のため、必要なキャストやunsafeブロックをいくつか省略しました）。

[raw pointer]: https://doc.rust-jp.rs/book-ja/ch19-01-unsafe-rust.html#%E7%94%9F%E3%83%9D%E3%82%A4%E3%83%B3%E3%82%BF%E3%82%92%E5%8F%82%E7%85%A7%E5%A4%96%E3%81%97%E3%81%99%E3%82%8B
[`ptr::write`]: https://doc.rust-lang.org/core/ptr/fn.write.html
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

割り当てられたメモリは`deallocate`の呼び出しによって明示的に解放されるまで生存します。したがって、返されたポインタは、`inner`がリターンしコールスタックの対応する部分が破棄された後も有効です。スタティックメモリと比較したときのヒープメモリの長所は、解放（`outer`内の`deallocate`呼び出しでまさにこれを行っています）後に再利用できるということです。この呼び出しの後、状況は以下のようになります。

![コールスタックはouterの局所変数を持っており、ヒープはz[0]とz[2]を持っているが、z[1]はもう持っていない。](call-stack-heap-freed.svg)

`z[1]`スロットが解放され、次の`allocate`呼び出しで再利用できることが分かります。しかし、`z[0]`と`z[2]`は永久にdeallocateされず、したがって永久に解放されないことも分かります。このようなバグは**メモリリーク**と呼ばれており、しばしばプログラムの過剰なメモリ消費を引き起こします（`inner`をループで何度も呼び出したらどんなことになるか、想像してみてください）。これ自体良くないことに思われるかもしれませんが、動的割り当てはもっと危険性の高いバグを発生させうるのです。

### よくあるミス

メモリリークは困りものですが、プログラムを攻撃者に対して脆弱にはしません。しかしこのほかに、より深刻な結果を招く二種類のバグが存在します：

- もし変数に対して`deallocate`を呼んだ後にも間違ってそれを使い続けたら、いわゆる<ruby>use-after-free<rp> (</rp><rt>メモリ解放後に使用</rt><rp>) </rp></ruby>脆弱性が発生します。このようなバグは未定義動作を引き起こし、しばしば攻撃者が任意コードを実行するのに利用されます。
- 間違ってある変数を二度解放したら、<ruby>double-free<rp> (</rp><rt>二重解放</rt><rp>) </rp></ruby>脆弱性が発生します。これが問題になるのは、最初の`deallocate`呼び出しの後に同じ場所にallocateされた別の割り当てを解放してしまうかもしれないからです。従って、これもまたuse-after-free脆弱性につながりかねません。

これらの脆弱性は広く知られているため、回避する方法も解明されているはずだとお思いになるかもしれません。しかし答えはいいえで、このような脆弱性は未だ散見され、例えば最近でも任意コード実行を許す[Linuxのuse-after-free脆弱性][linux vulnerability]が存在しました。このことは、最高のプログラマーであっても、複雑なプロジェクトにおいて常に正しく動的メモリを扱えはしないということを示しています。

[linux vulnerability]: https://securityboulevard.com/2019/02/linux-use-after-free-vulnerability-found-in-linux-2-6-through-4-20-11/

これらの問題を回避するため、JavaやPythonといった多くの言語では[**ガベージコレクション**][_garbage collection_]という技術を使って自動的に動的メモリを管理しています。発想としては、プログラマが絶対に自分の手で`deallocate`を呼び出すことがないようにするというものです。代わりに、プログラムが定期的に一時停止されてスキャンされ、未使用のヒープ変数が見つかったら自動的にdeallocateされるのです。従って、上のような脆弱性は絶対に発生し得ません。欠点としては，定期的にスキャンすることによる性能のオーバーヘッドが発生することと、一時停止の時間が長くなりがちであることが挙げられます。

[_garbage collection_]: https://ja.wikipedia.org/wiki/%E3%82%AC%E3%83%99%E3%83%BC%E3%82%B8%E3%82%B3%E3%83%AC%E3%82%AF%E3%82%B7%E3%83%A7%E3%83%B3

Rustはこの問題に対して別のアプローチを取ります：[**所有権**][_ownership_]と呼ばれる概念を使って、動的メモリの操作の正確性をコンパイル時にチェックするのです。従って前述の脆弱性を回避するためのガベージコレクションの必要がなく、性能のオーバーヘッドが存在しません。このアプローチのもう一つの利点として、CやC++と同様、プログラマが動的メモリの使用に関して精緻な制御を行うことができるということが挙げられます。

[_ownership_]: https://doc.rust-jp.rs/book-ja/ch04-01-what-is-ownership.html

### Rustにおける割り当て

プログラマーに自分の手で`allocate`と`deallocate`を呼ばせる代わりに、Rustの標準ライブラリはこれらの関数を暗黙の内に呼ぶ抽象型を提供しています。最も重要な型は[**`Box`**]で、これはヒープに割り当てられた値の抽象化です。これは[`Box::new`]コンストラクタ関数を提供しており、これは値を引数として、その値のサイズを引数に`allocate`を呼び出し、ヒープ上に新しく割り当てられたスロットにその値を<ruby>移動<rp> (</rp><rt>ムーブ</rt><rp>) </rp></ruby>します。ヒープメモリを解放するために、スコープから出た際に`deallocate`を呼ぶような[`Drop`トレイト][`Drop` trait]を`Box`型は実装しています。

[**`Box`**]: https://doc.rust-lang.org/std/boxed/index.html
[`Box::new`]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html#method.new
[`Drop` trait]: https://doc.rust-jp.rs/book-ja/ch15-03-drop.html

```rust
{
    let z = Box::new([1,2,3]);
    […]
} // zがスコープから出たので`deallocate`が呼ばれる
```

このような記法のパターンは[リソース取得は初期化である][_resource acquisition is initialization_]（resource acquisition is initialization、略してRAII）という奇妙な名前を持っています。C++で[`std::unique_ptr`]という同じような抽象型を実装するのに使われたのが始まりです。

[_resource acquisition is initialization_]: https://ja.wikipedia.org/wiki/RAII
[`std::unique_ptr`]: https://en.cppreference.com/w/cpp/memory/unique_ptr

このような型自体ではすべてのuse-after-freeバグを防ぐのに十分ではありません。なぜなら、プログラマは、`Box`がスコープ外に出て対応するヒープメモリスロットがdeallocateされた後でも参照を利用し続けることができてしまうからです：

```rust
let x = {
    let z = Box::new([1,2,3]);
    &z[1]
}; // zがスコープから出たので`deallocate`が呼ばれる
println!("{}", x);
```

ここでRustの所有権の出番です。所有権システムは、参照が有効なスコープを表す抽象[ライフタイム][lifetime]をそれぞれの参照に指定します。上の例では、参照`x`は配列`z`から取られているので、`z`がスコープ外に出ると無効になります。[上の例をplaygroundで実行する][playground-2]と、確かにRustコンパイラがエラーを投げるのが分かります：

[lifetime]: https://doc.rust-jp.rs/book-ja/ch10-03-lifetime-syntax.html
[playground-2]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=28180d8de7b62c6b4a681a7b1f745a48

```
error[E0597]: `z[_]` does not live long enough
 --> src/main.rs:4:9
  |
2 |     let x = {
  |         - borrow later stored here
3 |         let z = Box::new([1,2,3]);
4 |         &z[1]
  |         ^^^^^ borrowed value does not live long enough
5 |     }; // z goes out of scope and `deallocate` is called
  |     - `z[_]` dropped here while still borrowed
```

ここで使われている用語は初見では少しわかりにくいかもしれません。値の参照を取ることは値を借用する (borrow) と呼ばれています。これは現実での借用と似ているためです：オブジェクトに一時的にアクセスできるようになりますが、それをいつか返さなければならず、また破壊することも許されません。オブジェクトが破壊される前にすべての借用が終了することを確かめることにより、Rustコンパイラはuse-after-freeが起こりえないことを保証できるのです。

Rustの所有権システムはさらに突き詰められており、use-after-freeバグを防ぐだけでなく、JavaやPythonのようなガベージコレクション型言語と同じ完全な[メモリ<ruby>安全性<rp> (</rp><rt>セーフティ</rt><rp>) </rp></ruby>][_memory safety_]を提供しています。さらに[スレッド<ruby>安全性<rp> (</rp><rt>セーフティ</rt><rp>) </rp></ruby>][_thread safety_]も保証されており、マルチスレッドのプログラムにおいてはこれらの言語よりもさらに安全です。さらに最も重要なことに、これらのチェックは全てコンパイル時に行われるため、C言語で手書きされたメモリ管理と比べても実行時のオーバーヘッドはありません。

[_memory safety_]: https://ja.wikipedia.org/wiki/%E3%83%A1%E3%83%A2%E3%83%AA%E5%AE%89%E5%85%A8%E6%80%A7
[_thread safety_]: https://ja.wikipedia.org/wiki/%E3%82%B9%E3%83%AC%E3%83%83%E3%83%89%E3%82%BB%E3%83%BC%E3%83%95

### 使用例

Rustにおける動的メモリ割り当ての基礎を学んだわけですが、これをいつ使えば良いのでしょうか？私たちのカーネルは動的メモリ割り当てなしにこれだけやってこられたのに、どうして今になってこれが必要なのでしょうか？

まず覚えておいて欲しいのは、割り当てを行うたびにヒープから空いているスロットを探してこないといけないので、動的メモリ割り当てには少しだけ性能オーバーヘッドがあるということです。このため、特に性能が重要となるカーネルのプログラムにおいては、一般に局所変数の方が好ましいです。しかし、動的メモリ割り当てが最良の選択肢であるようなケースも存在するのです。

基本的なルールとして、動的メモリは動的なライフタイムや可変サイズを持つような変数に必要とされます。動的なライフタイムを持つ最も重要な型は[**`Rc`**]で、これはラップされた値に対する参照を数えておき、すべての参照がスコープから外れたらそれをdeallocateするというものです。可変サイズを持つ型の例には、[**`Vec`**]、[**`String`**]、その他の[コレクション型][collection types]といった、要素が追加されたときに動的に大きくなるような型が挙げられます。これらの型は、容量が一杯になると、より大きい量のメモリを割り当て、すべての要素をコピーし、古い割り当てをdeallocateすることにより対処します。

[**`Rc`**]: https://doc.rust-lang.org/alloc/rc/index.html
[**`Vec`**]: https://doc.rust-lang.org/alloc/vec/index.html
[**`String`**]: https://doc.rust-lang.org/alloc/string/index.html
[collection types]: https://doc.rust-lang.org/alloc/collections/index.html

私たちのカーネルでは主にコレクション型を必要とし、例えば、将来の記事でマルチタスキングを実行するときにアクティブなタスクのリストを格納するために使います。

## アロケータインターフェース

ヒープアロケータを実装するための最初のステップは、組み込みの[`alloc`]クレートへの依存関係を追加することです。[`core`]クレートと同様、これは標準ライブラリのサブセットであり、アロケーション型やコレクション型を含んでいます。`alloc`への依存関係を追加するために、以下を`lib.rs`に追加します：

[`alloc`]: https://doc.rust-lang.org/alloc/
[`core`]: https://doc.rust-lang.org/core/

```rust
// in src/lib.rs

extern crate alloc;
```

通常の依存関係と異なり`Cargo.toml`を修正する必要はありません。その理由は、`alloc`クレートは標準ライブラリの一部としてRustコンパイラに同梱されているため、コンパイラはすでにこのクレートのことを知っているからです。この`extern crate`宣言を追加することで、コンパイラにこれをインクルードしようと試みるよう指定しています（昔はすべての依存関係が`extern crate`宣言を必要としていたのですが、いまは任意です）。

<div class="note">

**訳者注：** 詳しくは[edition guideの対応するページ](https://doc.rust-jp.rs/edition-guide/rust-2018/path-changes.html#%E3%81%95%E3%82%88%E3%81%86%E3%81%AA%E3%82%89extern-crate)をご覧ください。

</div>

カスタムターゲット向けにコンパイルしようとしているので、Rustインストール時に同梱されていたコンパイル済みの`alloc`を使用することはできません。代わりにcargoにこのクレートをソースから再コンパイルするよう命令する必要があります。これは、配列`unstable.build-std`を`.cargo/config.toml`ファイルに追加することで行えます。

```toml
# in .cargo/config.toml

[unstable]
build-std = ["core", "compiler_builtins", "alloc"]
````

これでコンパイラは`alloc`クレートを再コンパイルして私たちのカーネルにインクルードしてくれます。

`alloc`クレートが`#[no_std]`なクレートで標準では無効化されている理由は、これが追加の要件を持っているからです。今私たちのプロジェクトをコンパイルしようとすると、その要件をエラーとして目にすることになります：

```
error: no global memory allocator found but one is required; link to std or add
       #[global_allocator] to a static item that implements the GlobalAlloc trait.
（エラー：グローバルメモリアロケータが見つかりませんが、一つ必要です。
　stdをリンクするか、GlobalAllocトレイトを実装する静的な要素に#[global_allocator]を付けてください。）

error: `#[alloc_error_handler]` function required, but not found
（エラー：`#[alloc_error_handler]`関数が必要ですが、見つかりません）
```

最初のエラーは、`alloc`クレートが、ヒープアロケータという`allocate`と`deallocate`関数を提供するオブジェクトを必要とするために発生します。Rustにおいては、ヒープアロケータ（の満たすべき性質）は[`GlobalAlloc`]トレイトによって記述されており、エラーメッセージでもそのことについて触れられています。クレートのヒープアロケータを設定するためには、`#[global_allocator]`属性を`GlobalAlloc`トレイトを実装する何らかの`static`変数に適用する必要があります。

二つ目のエラーは、（主にメモリが不足している場合）`allocate`の呼び出しが失敗しうるために発生します。私たちのプログラムはこのケースに対処できるようになっている必要があり、そのために使われる関数が`#[alloc_error_handler]`なのです。

[`GlobalAlloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html

次のセクションでこのトレイトと属性について説明します。

### `GlobalAlloc`トレイト

[`GlobalAlloc`]トレイトはヒープアロケータの提供しなければならない関数を定義します。このトレイトは、プログラマが絶対に直接使わないという点において特別です。代わりに、`alloc`のアロケーション・コレクション型を使うときに、コンパイラがトレイトメソッドへの適切な呼び出しを自動的に挿入します。

このトレイトを私たちのアロケータ型全てに実装しなければならないので、その宣言は詳しく見ておく価値があるでしょう：

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

このトレイトは[`alloc`]と[`dealloc`]という必須メソッドを定義しており、これは上の例で使った`allocate`と`deallocate`関数に相当します：
- [`alloc`]メソッドは[`Layout`]インスタンス（割り当てられたメモリの持つべきサイズとアラインメントを記述する）を引数として取ります。メソッドは割り当てられたメモリブロックの最初のバイトへの[生ポインタ][raw pointer]を返します。割り当てエラーが起きたことを示す際は、明示的なエラー値を返す代わりにヌルポインタを返します。このやり方は（Rustの）慣習とはやや外れていますが、同じ慣習に従っている既存のシステムのアロケータをラップするのが簡単になるという利点があります。
- [`dealloc`]はその対で、メモリブロックを開放する役割を持ちます。このメソッドは、`alloc`によって返されたポインタと割り当ての際に使われた`Layout`という二つの引数を取ります。

[`alloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.alloc
[`dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc
[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html

このトレイトは[`alloc_zeroed`]と[`realloc`]という二つのデフォルト実装付きメソッドも定義しています。

- [`alloc_zeroed`]メソッドは`alloc`を呼んでから割り当てられたメモリブロックの値を0にするのに等しく、デフォルト実装でもまさに同じことをしています。もし、より効率的なカスタム実装があるならば、デフォルト実装を上書きすることもできます。
- [`realloc`]メソッドは割り当てたメモリを拡大したり縮小したりすることができます。デフォルト実装では、要求されたサイズの新しいメモリブロックを割り当て、以前のアロケーションから中身を全てコピーします。同じく、アロケータの実装によってはこのメソッドをより効率的に実装することができるかもしれません。例えば、可能な場合はその場でアロケーションを拡大・縮小するなど。

[`alloc_zeroed`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#method.alloc_zeroed
[`realloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#method.realloc

#### Unsafe

トレイト自体とすべてのトレイトメソッドが`unsafe`として宣言されていることに気をつけましょう：

- トレイトを`unsafe`として宣言する理由は、プログラマがアロケータ型のトレイト実装が正しいことを保証しなければならないからです。例えば、`alloc`メソッドは他のどこかですでに使用されているメモリブロックを決して返してはならず、もしそうすると未定義動作が発生してしまいます。
- 同様に、メソッドが`unsafe`である理由は、メソッドを呼び出す際に呼び出し元がいくつかの不変条件を保証しなければならないからです。例えば、`alloc`に渡される`Layout`の指定するサイズが非ゼロであることなどです。実際にはこれは大して重要ではなく、というのもこれらのメソッドはコンパイラによって直接呼び出されるため、これらの要件が満たされていることは保証されているからです。

### `DummyAllocator`

アロケータ型が何を提供しないといけないかを理解したので、シンプルな<ruby>ダミー<rp> (</rp><rt>ハリボテ</rt><rp>) </rp></ruby>のアロケータを作ることができます。そのためまず新しく`allocator`モジュールを作りましょう：

```rust
// in src/lib.rs

pub mod allocator;
```

私たちのダミーアロケータでは、トレイトを実装するための最小限のことしかせず、`alloc`が呼び出されたら常にエラーを返すようにします。以下のようになります：

```rust
// in src/allocator.rs

use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should be never called")
    }
}
```

この構造体はフィールドを必要としないので、[サイズがゼロの型][zero sized type]として作成します。上で述べたように、`alloc`は常に割り当てエラーに相当するヌルポインタを返すようにします。アロケータがメモリを返すことは絶対に起きないのだから、`dealloc`の呼び出しも絶対に起きないはずです。このため`dealloc`メソッドでは単にpanicすることにします。`alloc_zeroed`と`realloc`メソッドにはデフォルト実装があるので、これらを実装する必要はありません。

[zero sized type]: https://doc.rust-jp.rs/rust-nomicon-ja/exotic-sizes.html#%E3%82%B5%E3%82%A4%E3%82%BA%E3%81%8C-0-%E3%81%AE%E5%9E%8Bzst-zero-sized-type

こうして単純なアロケータを手に入れたわけですが、さらにRustコンパイラにこのアロケータを使うよう指示しないといけません。ここで`#[global_allocator]`属性の出番です。

### `#[global_allocator]`属性

`#[global_allocator]`属性は、どのアロケータインスタンスをグローバルヒープアロケータとして使うべきかをRustコンパイラに指示します。この属性は`GlobalAlloc`トレイトを実装する`static`にのみ適用できます。私たちの`Dummy`アロケータのインスタンスをグローバルアロケータとして登録してみましょう：

```rust
// in src/allocator.rs

#[global_allocator]
static ALLOCATOR: Dummy = Dummy;
```

`Dummy`アロケータは[サイズがゼロの型][zero sized type]なので、初期化式でフィールドを指定する必要はありません。

これをコンパイルしようとすると、最初のエラーは消えているはずです。残っている二つ目のエラーを修正しましょう：

```
error: `#[alloc_error_handler]` function required, but not found
```

### `#[alloc_error_handler]`属性

`GlobalAlloc`トレイトについて議論したときに学んだように、`alloc`関数はヌルポインタを返すことによって割り当てエラーを示します。ここで生じる疑問は、そのように割り当てが失敗したときRustランタイムはどう対処するべきなのかということです。ここで`#[alloc_error_handler]`属性の出番です。この属性は、パニックが起こったときにパニックハンドラが呼ばれるのと同じように、割り当てエラーが起こったときに呼ばれる関数を指定するのです。

コンパイルエラーを修正するためにそのような関数を追加してみましょう：

```rust
// in src/lib.rs

#![feature(alloc_error_handler)] // ファイルの先頭に書く

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
```

`alloc_error_handler`関数はまだunstableなので、feature gateによってこれを有効化する必要があります。この関数は引数を一つ取ります：割り当てエラーが起こったとき`alloc`関数に渡されていた`Layout`のインスタンスです。割り当ての失敗を解決するためにできることはないので、`Layout`インスタンスを含めたメッセージを表示してただpanicすることにしましょう。

この関数を追加したことで、コンパイルエラーは修正されたはずです。これで`alloc`のアロケーション・コレクション型を使えるようになりました。例えば、[`Box`]を使ってヒープに値を割り当てることができます：

[`Box`]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html

```rust
// in src/main.rs

extern crate alloc;

use alloc::boxed::Box;

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] "Hello World!"を表示, `init`の呼び出し, `mapper`と`frame_allocator`を作成

    let x = Box::new(41);

    // […] テストモードでは`test_main`を呼ぶ

    println!("It did not crash!");
    blog_os::hlt_loop();
}

```

`main.rs`においても`extern crate alloc`文を指定する必要があることに注意してください。`lib.rs`と`main.rs`は別のクレートとして取り扱われているためです。しかしながら、グローバルアロケータはプロジェクト内のすべてのクレートに適用されるため、`#[global_allocator]`静的変数をもう一つ作る必要はありません。実際、別のクレートで新しいアロケータを指定するとエラーになります。

上のコードを実行すると、`alloc_error_handler`関数が呼ばれるのが分かります：

![QEMUが"panicked at `allocation error: Layout { size_: 4, align_: 4 }, src/lib.rs:89:5"と出力している。](qemu-dummy-output.png)

`Box::new`関数は暗黙のうちにグローバルアロケータの`alloc`関数を呼び出すため、エラーハンドラが呼ばれました。私たちのダミーアロケータは常にヌルポインタを返すので、あらゆる割り当てが失敗するのです。これを修正するためには、使用可能なメモリを実際に返すアロケータを作る必要があります。

## Creating a Kernel Heap

適切なアロケータを作りたいですが、その前にまず、そのアロケータがメモリを割り当てるためのヒープメモリ領域を作らないといけません。このために、ヒープ領域のための仮想メモリ範囲を定義し、その領域を物理フレームに対応付ける必要があります。仮想メモリとページテーブルの概要については、[ページング入門][_"Introduction To Paging"_]の記事を読んでください。

[_"Introduction To Paging"_]: @/edition-2/posts/08-paging-introduction/index.ja.md

最初のステップはヒープのための仮想メモリ領域を定義することです。他のメモリ領域に使われていない限り、どんな仮想アドレス範囲でも構いません。ここでは、あとからそこがヒープポインタだと簡単に分かるよう、`0x_4444_4444_0000`から始まるメモリとしましょう。

```rust
// in src/allocator.rs

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB
```

今のところヒープの大きさは100 KiBとします。将来より多くの領域が必要になったら大きくすれば良いです。

今このヒープ領域を使おうとすると、仮想メモリ領域が物理メモリにまだ対応付けられていないためページフォルトが発生します。これを解決するために、[ページング入門][_"Paging Implementation"_]の記事で導入した[`Mapper` API]を使ってヒープページを対応付ける関数`init_heap`を作ります：

[`Mapper` API]: @/edition-2/posts/09-paging-implementation/index.ja.md#offsetpagetablewoshi-u
[_"Paging Implementation"_]: @/edition-2/posts/09-paging-implementation/index.ja.md

```rust
// in src/allocator.rs

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        };
    }

    Ok(())
}
```

この関数は[`Mapper`]と[`FrameAllocator`]への可変参照を取ります。これらはどちらも[`Size4KiB`]をジェネリックパラメータとすることで4KiBページのみに制限しています。この関数の戻り値は[`Result`]で、成功ヴァリアントが`()`、失敗ヴァリアントが（[`Mapper::map_to`]メソッドによって失敗時に返されるエラー型である）[`MapToError`]です。この関数における主なエラーの原因は`map_to`メソッドであるため、このエラー型を流用するのは理にかなっています。

[`Mapper`]:https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html
[`FrameAllocator`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.FrameAllocator.html
[`Size4KiB`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/enum.Size4KiB.html
[`Result`]: https://doc.rust-lang.org/core/result/enum.Result.html
[`MapToError`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/enum.MapToError.html
[`Mapper::map_to`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html#method.map_to

実装内容は以下の二つに分けられます：

- **ページ範囲の作成:** 対応付けたいページ領域を作成するために、ポインタ`HEAP_START`を[`VirtAddr`]型に変換します。つぎに`HEAP_SIZE`を足すことによってヒープの終端アドレスを計算します。<ruby>端が含まれる境界<rp> (</rp><rt>インクルーシブレンジ</rt><rp>) </rp></ruby>にしたい（ヒープの最後のバイトのアドレスとしたい）ので1を引きます。次に、これらのアドレスを[`containing_address`]関数を使って[`Page`]型に変換します。最後に、[`Page::range_inclusive`]関数を使って最初と最後のページからページ範囲を作成します。

- **ページの<ruby>対応付け<rp> (</rp><rt>マッピング</rt><rp>) </rp></ruby>:** 二つ目のステップは、今作ったページ範囲のすべてのページに対して対応付けを行うことです。これを行うため、`for`ループを使ってこのページ範囲に対して繰り返し処理を行います。それぞれのページに対して以下を行います：

	- [`FrameAllocator::allocate_frame`]メソッドを使って、ページのマップされるべき物理フレームを割り当てます。このメソッドはもうフレームが残っていないとき[`None`]を返します。このケースに対処するため、[`Option::ok_or`]メソッドを使ってこれを[`MapToError::FrameAllocationFailed`]に変換し、エラーの場合は[`?`演算子][question mark operator]を使って早期リターンしています。

	- このページに対し、必要となる`PRESENT`フラグと`WRITABLE`フラグをセットします。これらのフラグにより読み書きのアクセスが許可されますが、これはヒープメモリとして理にかなっています。

	- [`Mapper::map_to`]メソッドを使ってアクティブなページテーブルに対応付けを作成します。このメソッドは失敗しうるので、同様に[`?`演算子][question mark operator]を使ってエラーを呼び出し元に受け渡します。成功時には、このメソッドは[`MapperFlush`]インスタンスを返しますが、これを使って[`flush`]メソッドを呼ぶことで[**トランスレーション・ルックアサイド・バッファ**][_translation lookaside buffer_]を更新することができます。

[`VirtAddr`]: https://docs.rs/x86_64/0.14.2/x86_64/addr/struct.VirtAddr.html
[`Page`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/struct.Page.html
[`containing_address`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/struct.Page.html#method.containing_address
[`Page::range_inclusive`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/struct.Page.html#method.range_inclusive
[`FrameAllocator::allocate_frame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.FrameAllocator.html#tymethod.allocate_frame
[`None`]: https://doc.rust-lang.org/core/option/enum.Option.html#variant.None
[`MapToError::FrameAllocationFailed`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/enum.MapToError.html#variant.FrameAllocationFailed
[`Option::ok_or`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.ok_or
[question mark operator]: https://doc.rust-jp.rs/book-ja/ch09-02-recoverable-errors-with-result.html#%E3%82%A8%E3%83%A9%E3%83%BC%E5%A7%94%E8%AD%B2%E3%81%AE%E3%82%B7%E3%83%A7%E3%83%BC%E3%83%88%E3%82%AB%E3%83%83%E3%83%88-%E6%BC%94%E7%AE%97%E5%AD%90
[`MapperFlush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html
[_translation lookaside buffer_]: @/edition-2/posts/08-paging-introduction/index.ja.md#toransuresiyonrutukuasaidobatuhua
[`flush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html#method.flush

最後のステップは、この関数を`kernel_main`から呼び出すことです：

```rust
// in src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::allocator; // 新しいインポート
    use blog_os::memory::{self, BootInfoFrameAllocator};

    println!("Hello World{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    // ここを追加
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    let x = Box::new(41);

    // […] テストモードでは`test_main`を呼ぶ

    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

ここで、文脈が分かるよう関数の全体を示しています。（しかし）新しい行は`blog_os::allocator`のインポートと`allocator::init_heap`の呼び出しだけです。`init_heap`関数がエラーを返した場合、これを処理する良い方法は今のところないため、[`Result::expect`]メソッドを使ってパニックします。

[`Result::expect`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.expect

これで、使用する準備のできた、対応付けられたヒープメモリ領域を手に入れました。`Box::new`の呼び出しはまだ私たちの古い`Dummy`アロケータを使っているので、実行しても依然として「メモリ不足」のエラーを見ることになるでしょう。適切なアロケータを使うようにして、このエラーを修正してみましょう。

## アロケータクレートを使う

アロケータを実装するのは少々複雑なので、まずは既製のアロケータを使うことにしましょう。アロケータを自作する方法については次の記事で学びます。

`no_std`のアプリケーションのためのシンプルなアロケータのひとつに[`linked_list_allocator`]クレートがあります。この名前は、割り当てられていないメモリ領域を連結リストを使って管理しているところから来ています。この手法のより詳しい説明については次の記事を読んでください。

このクレートを使うためには、まず依存関係を`Cargo.toml`に追加する必要があります：

[`linked_list_allocator`]: https://github.com/phil-opp/linked-list-allocator/

```toml
# in Cargo.toml

[dependencies]
linked_list_allocator = "0.9.0"
```

次に私たちのダミーアロケータをこのクレートによって提供されるアロケータで置き換えます：

```rust
// in src/allocator.rs

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();
```

この構造体は同期のために`spinning_top::Spinlock`型を使うため`LockedHeap`という名前が付いています。これが必要なのは、`ALLOCATOR`静的変数に複数のスレッドが同時にアクセスすることがありえるからです。スピンロックやmutexを使うときはいつもそうであるように、誤ってデッドロックを起こさないように注意する必要があります。これが意味するのは、我々は割り込みハンドラ内で一切アロケーションを行ってはいけないと言うことです。なぜなら、割り込みハンドラはどんなタイミングでも走る可能性があるため、進行中のアロケーションに割り込んでいることがあるからです。

[`spinning_top::Spinlock`]: https://docs.rs/spinning_top/0.1.0/spinning_top/type.Spinlock.html

`LockedHeap`をグローバルアロケータとして設定するだけでは十分ではありません。いま[`empty`]コンストラクタ関数を使っていますが、この関数はメモリを与えることなくアロケータを作るからです。私たちのダミーアロケータと同じく、これ（今の状態の`LockedHeap`）は`alloc`を行うと常にエラーを返します。この問題を修正するため、ヒープを作った後でアロケータを初期化する必要があります：

[`empty`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.LockedHeap.html#method.empty

```rust
// in src/allocator.rs

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // […] すべてのヒープページを物理フレームにマップする

    // new
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
```

`LockedHeap`型の内部のスピンロックの[`lock`]メソッドを呼ぶことで、ラップされた[`Heap`]インスタンスへの排他参照を得て、これの[`init`]メソッドをヒープの境界を引数として呼んでいます。`init`関数自体がヒープメモリに書き込もうとするので、ヒープページを対応付けた **後に** ヒープを初期化することが重要です。

[`lock`]: https://docs.rs/lock_api/0.3.3/lock_api/struct.Mutex.html#method.lock
[`Heap`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html
[`init`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.init

ヒープを初期化できたら、組み込みの[`alloc`]クレートのあらゆるアロケーション・コレクション型がエラーなく使用できます：

```rust
// in src/main.rs

use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] initialize interrupts, mapper, frame_allocator, heap

    // ヒープに数字をアロケートする
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    // 動的サイズのベクタを作成する
    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    // 参照カウントされたベクタを作成する -> カウントが0になると解放される
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!("current reference count is {}", Rc::strong_count(&cloned_reference));
    core::mem::drop(reference_counted);
    println!("reference count is {} now", Rc::strong_count(&cloned_reference));

    // […] テストでは `test_main` を呼ぶ
    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

このコード例では[`Box`], [`Vec`], [`Rc`]型を使ってみました。`Box`型と`Vec`型については対応するヒープポインタを[`{:p}`フォーマット指定子][`{:p}` formatting specifier]を使って出力しています。`Rc`についての例を示すために、参照カウントされたヒープ値を作成し、インスタンスを（[`core::mem::drop`]を使って）ドロップする前と後に[`Rc::strong_count`]関数を使って現在の参照カウントを出力しています。

[`Vec`]: https://doc.rust-lang.org/alloc/vec/
[`Rc`]: https://doc.rust-lang.org/alloc/rc/
[`{:p}` formatting specifier]: https://doc.rust-lang.org/core/fmt/trait.Pointer.html
[`Rc::strong_count`]: https://doc.rust-lang.org/alloc/rc/struct.Rc.html#method.strong_count
[`core::mem::drop`]: https://doc.rust-lang.org/core/mem/fn.drop.html

実行すると、以下のような結果を得ます：

![QEMUが`
heap_value at 0x444444440000
vec at 0x4444444408000
current reference count is 2
reference count is 1 now
`と出力している](qemu-alloc-showcase.png)

ポインタが`0x_4444_4444_*`で始まることから、`Box`と`Vec`の値は想定通りヒープ上にあることが分かります。参照カウントされた値も期待したとおり振る舞っており、`clone`呼び出しの後では参照カウントは2になり、インスタンスの一方がドロップされた後では再び1になっています。

ベクタがヒープメモリの先頭から`0x800`だけずれた場所から始まるのは、Box内の値が`0x800`バイトの大きさがあるためではなく、ベクタが容量を増やさなければならないときに発生する[<ruby>再割り当て<rp> (</rp><rt>リアロケーション</rt><rp>) </rp></ruby>][reallocations]のためです。例えば、ベクタの容量が32の際に次の要素を追加しようとすると、ベクタは内部で容量64の配列を新たに割り当て、すべての要素をコピーします。その後古い割り当てを解放しています。

[reallocations]: https://doc.rust-lang.org/alloc/vec/struct.Vec.html#capacity-and-reallocation

もちろん`alloc`クレートにはもっと多くのアロケーション・コレクション型があり、今やそれらのすべてを私たちのカーネルで使うことができます。それには以下が含まれます：

- スレッドセーフな参照カウントポインタ[`Arc`]
- 文字列を所有する型[`String`]と[`format!`]マクロ
- [`LinkedList`]
- 必要に応じてサイズを大きくできるリングバッファ[`VecDeque`]
- プライオリティキューである[`BinaryHeap`]
- [`BTreeMap`]と[`BTreeSet`]

[`Arc`]: https://doc.rust-lang.org/alloc/sync/struct.Arc.html
[`String`]: https://doc.rust-lang.org/alloc/string/struct.String.html
[`format!`]: https://doc.rust-lang.org/alloc/macro.format.html
[`LinkedList`]: https://doc.rust-lang.org/alloc/collections/linked_list/struct.LinkedList.html
[`VecDeque`]: https://doc.rust-lang.org/alloc/collections/vec_deque/struct.VecDeque.html
[`BinaryHeap`]: https://doc.rust-lang.org/alloc/collections/binary_heap/struct.BinaryHeap.html
[`BTreeMap`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html
[`BTreeSet`]: https://doc.rust-lang.org/alloc/collections/btree_set/struct.BTreeSet.html

これらの型は、スレッドリスト、スケジュールキュー、async/awaitのサポートを実装しようとするときにとても有用になります。

## テストを追加する

いま新しく作ったアロケーションコードを間違って壊してしまうことがないことを保証するために、<ruby>結合<rp> (</rp><rt>インテグレーション</rt><rp>) </rp></ruby>テストを追加するべきでしょう。まず、次のような内容のファイル`tests/heap_allocation.rs`を作成します。

```rust
// in tests/heap_allocation.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

`lib.rs`の`test_runner`関数と`test_panic_handler`関数を再利用します。私たちはアロケーションをテストしたいので、`extern crate alloc`宣言を使って`alloc`クレートを有効化します。テストに共通する定型部については[テスト][_Testing_]の記事を読んでください。

[_Testing_]: @/edition-2/posts/04-testing/index.ja.md

`main`関数の実装は以下のようになります：

```rust
// in tests/heap_allocation.rs

fn main(boot_info: &'static BootInfo) -> ! {
    use blog_os::allocator;
    use blog_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    blog_os::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    test_main();
    loop {}
}
```

私たちの`main.rs`内の`kernel_main`関数によく似ていますが、`println`を呼び出さず、例示のため行ったアロケーションも行わず、また`test_main`を無条件で呼び出しているという違いがあります。

これでテストケースを追加する準備ができました。まず、[`Box`]を使って単純な<ruby>割り当て<rp> (</rp><rt>アロケーション</rt><rp>) </rp></ruby>を行い、割り当てられた値を確かめることで基本的なアロケーションがうまくいっていることを確かめるテストを追加しましょう：

```rust
// in tests/heap_allocation.rs
use alloc::boxed::Box;

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}
```

最も重要なのは、このテストはアロケーションエラーが起きないことを検証してくれるということです。

次に、反復によって少しずつ大きなベクタを作ることで、大きな割り当てと（再割り当てによる）複数回の割り当ての両方をテストしましょう：

```rust
// in tests/heap_allocation.rs

use alloc::vec::Vec;

#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}
```

このベクタの和を[n次部分和][n-th partial sum]の公式と比較することで検証しています。これにより、割り当てられた値はすべて正しいことをある程度保証できます。

[n-th partial sum]: https://ja.wikipedia.org/wiki/1%2B2%2B3%2B4%2B%E2%80%A6

3つ目のテストとして、10000回次々にアロケーションを行います：

```rust
// in tests/heap_allocation.rs

use blog_os::allocator::HEAP_SIZE;

#[test_case]
fn many_boxes() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}
```

このテストではアロケータが解放されたメモリを次の割り当てで再利用していることを保証してくれます。もしそうなっていなければメモリ不足が起きるでしょう。こんなことアロケータにとって当たり前の要件だと思われるかもしれませんが、これを行わないようなアロケータの設計も存在するのです。その例として、次の記事で説明するbump allocatorがあります。

では、私たちの新しい結合テストを実行してみましょう：

```
> cargo test --test heap_allocation
[…]
Running 3 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
```

すべてのテストが成功しました！`cargo test`コマンドを（`--test`引数なしに）呼ぶことで、すべての結合テストを実行することもできます。

## まとめ

この記事では動的メモリに入門し、なぜ、そしていつそれが必要になるのかを説明しました。Rustの借用チェッカがどのようにしてよくある脆弱性を防ぐのか、そしてRustのアロケーションAPIがどのような仕組みなのかを理解しました。

ダミーアロケータでRustのアロケータインターフェースの最小限の実装を作成した後、私たちのカーネル用の適切なヒープメモリ領域を作成しました。これを行うために、ヒープ用の仮想アドレス範囲を定義し、前の記事で説明した`Mapper`と`FrameAllocator`を使ってその範囲のすべてのページを物理フレームに対応付けました。

最後に、`linked_list_allocator`クレートへの依存関係を追加し、適切なアロケータを私たちのカーネルに追加しました。このアロケータのおかげで、`alloc`クレートに含まれる`Box`、`Vec`、その他のアロケーション・コレクション型を使えるようになりました。

## 次は？

この記事ではヒープ割り当て機能のサポートを追加しましたが、ほとんどの仕事は`linked_list_allocator`クレートに任せてしまっています。次の記事では、アロケータをゼロから実装する方法を詳細にお伝えします。可能なアロケータの設計を複数提示し、それらを単純化したものを実装する方法を示し、それらの利点と欠点を説明します。
