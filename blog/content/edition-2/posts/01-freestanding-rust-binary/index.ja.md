+++
title = "フリースタンディングな Rust バイナリ"
weight = 1
path = "ja/freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "6f1f87215892c2be12c6973a6f753c9a25c34b7e"
# GitHub usernames of the people that translated this post
translators = ["JohnTitor"]
+++

私達自身のオペレーティングシステム(以下、OS)カーネルを作っていく最初のステップは標準ライブラリとリンクしない Rust の実行可能ファイルをつくることです。これにより、基盤となる OS がない[ベアメタル][bare metal]上で Rust のコードを実行することができるようになります。

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

このブログの内容は [GitHub] 上で公開・開発されています。何か問題や質問などがあれば issue をたててください (訳注: リンクは原文(英語)のものになります)。また[こちら][comments]にコメントを残すこともできます。この記事の完全なソースコードは[`post-01` ブランチ][post branch]にあります。

[GitHub]: https://github.com/phil-opp/blog_os
[comments]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## 導入

OS カーネルを書くためには、いかなる OS の機能にも依存しないコードが必要となります。つまり、スレッドやヒープメモリ、ネットワーク、乱数、標準出力、その他 OS による抽象化や特定のハードウェアを必要とする機能は使えません。私達は自分自身で OS とそのドライバを書こうとしているので、これは理にかなっています。

これは [Rust の標準ライブラリ][Rust standard library]をほとんど使えないということを意味しますが、それでも私達が使うことのできる Rust の機能はたくさんあります。例えば、[イテレータ][iterators]や[クロージャ][closures]、[パターンマッチング][pattern matching]、 [`Option`][option] や [`Result`][result] 型に[文字列フォーマット][string formatting]、そしてもちろん[所有権システム][ownership system]を使うことができます。これらの機能により、[未定義動作][undefined behavior]や[メモリ安全性][memory safety]を気にせずに、高い水準で表現力豊かにカーネルを書くことができます。

[option]: https://doc.rust-lang.org/core/option/
[result]:https://doc.rust-lang.org/core/result/
[Rust standard library]: https://doc.rust-lang.org/std/
[iterators]: https://doc.rust-lang.org/book/ch13-02-iterators.html
[closures]: https://doc.rust-lang.org/book/ch13-01-closures.html
[pattern matching]: https://doc.rust-lang.org/book/ch06-00-enums.html
[string formatting]: https://doc.rust-lang.org/core/macro.write.html
[ownership system]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
[undefined behavior]: https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs
[memory safety]: https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention

Rust で OS カーネルを書くには、基盤となる OS なしで動く実行環境をつくる必要があります。そのような実行環境はフリースタンディング環境やベアメタルのように呼ばれます。

この記事では、フリースタンディングな Rust のバイナリをつくるために必要なステップを紹介し、なぜそれが必要なのかを説明します。もし最小限の説明だけを読みたいのであれば **[概要](#概要)** まで飛ばしてください。

## 標準ライブラリの無効化

デフォルトでは、全ての Rust クレートは[標準ライブラリ][standard library]にリンクされています。標準ライブラリはスレッドやファイル、ネットワークのような OS の機能に依存しています。また OS と密接な関係にある C の標準ライブラリ(`libc`)にも依存しています。私達の目的は OS を書くことなので、 OS 依存のライブラリを使うことはできません。そのため、 [`no_std` attribute] を使って標準ライブラリが自動的にリンクされるのを無効にします。

[standard library]: https://doc.rust-lang.org/std/
[`no_std` attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

新しい Cargo プロジェクトをつくるところから始めましょう。もっとも簡単なやり方はコマンドラインで以下を実行することです。

```bash
cargo new blog_os --bin --edition 2018
```

プロジェクト名を `blog_os` としましたが、もちろんお好きな名前をつけていただいても大丈夫です。`--bin`フラグは実行可能バイナリを作成することを、 `--edition 2018` は[2018エディション][2018 edition]を使用することを明示的に指定します。コマンドを実行すると、 Cargoは以下のようなディレクトリ構造を作成します:

[2018 edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html

```bash
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

`Cargo.toml` にはクレートの名前や作者名、[セマンティックバージョニング][semantic version]に基づくバージョンナンバーや依存関係などが書かれています。`src/main.rs` には私達のクレートのルートモジュールと `main` 関数が含まれています。`cargo build` コマンドでこのクレートをコンパイルして、 `target/debug` ディレクトリの中にあるコンパイルされた `blog_os` バイナリを実行することができます。

[semantic version]: https://semver.org/

### `no_std` Attribute

今のところ私達のクレートは暗黙のうちに標準ライブラリをリンクしています。[`no_std` attribute]を追加してこれを無効にしてみましょう:

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

(`cargo build` を実行して)ビルドしようとすると、次のようなエラーが発生します:

```bash
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

これは [`println` マクロ][`println` macro]が標準ライブラリに含まれているためです。`no_std` で標準ライブラリを無効にしたので、何かをプリントすることはできなくなりました。`println` は標準出力に書き込むのでこれは理にかなっています。[標準出力][standard output]は OS によって提供される特別なファイル記述子であるためです。

[`println` macro]: https://doc.rust-lang.org/std/macro.println.html
[standard output]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

では、 `println` を削除し `main` 関数を空にしてもう一度ビルドしてみましょう:

```rust
// main.rs

#![no_std]

fn main() {}
```

```bash
> cargo build
error: `#[panic_handler]` function required, but not found
error: language item required, but not found: `eh_personality`
```

この状態では `#[panic_handler]` 関数と `language item` がないというエラーが発生します。

## Panic の実装

`panic_handler` attribute は[パニック]が発生したときにコンパイラが呼び出す関数を定義します。標準ライブラリには独自のパニックハンドラー関数がありますが、 `no_std` 環境では私達の手でそれを実装する必要があります:

[パニック]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// in main.rs

use core::panic::PanicInfo;

/// この関数はパニック時に呼ばれる
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

[`PanicInfo` パラメータ]には、パニックが発生したファイルと行、およびオプションでパニックメッセージが含まれます。この関数は戻り値を取るべきではないので、]"never" 型(`!`)][“never” type]を返すことで[発散する関数][diverging function]となります。今のところこの関数でできることは多くないので、無限にループするだけです。

[`PanicInfo` パラメータ]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[“never” type]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## `eh_personality` Language Item

language item はコンパイラによって内部的に必要とされる特別な関数や型です。例えば、[`Copy`] トレイトはどの型が[コピーセマンティクス][`Copy`]を持っているかをコンパイラに伝える language item です。[実装][copy code]を見てみると、 language item として定義されている特別な `#[lang = "copy"]` attribute を持っていることが分かります。

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

独自に language item を実装することもできますが、これは最終手段として行われるべきでしょう。というのも、language item は非常に不安定な実装であり型検査も行われないからです(なので、コンパイラは関数が正しい引数の型を取っているかさえ検査しません)。幸い、上記の language item のエラーを修正するためのより安定した方法があります。

[`eh_personality` language item] は[スタックアンワインド][stack unwinding] を実装するための関数を定義します。デフォルトでは、パニックが起きた場合には Rust はアンワインドを使用してすべてのスタックにある変数のデストラクタを実行します。これにより、使用されている全てのメモリが確実に解放され、親スレッドはパニックを検知して実行を継続できます。しかしアンワインドは複雑であり、いくつかの OS 特有のライブラリ(例えば、Linux では [libunwind] 、Windows では[構造化例外][structured exception handling])を必要とするので、私達の OS には使いたくありません。

[`eh_personality` language item]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: https://www.nongnu.org/libunwind/
[structured exception handling]:  https://docs.microsoft.com/en-us/windows/win32/debug/structured-exception-handling

### アンワインドの無効化

他にもアンワインドが望ましくないユースケースがあります。そのため、Rust には代わりに[パニックで中止する][abort on panic]オプションがあります。これにより、アンワインドのシンボル情報の生成が無効になり、バイナリサイズが大幅に削減されます。アンワインドを無効にする方法は複数あります。もっとも簡単な方法は、`Cargo.toml` に次の行を追加することです:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

これは dev プロファイル(`cargo build` に使用される)と release プロファイル(`cargo build --release` に使用される)の両方でパニックで中止するようにするための設定です。これで `eh_personality` language item が不要になりました。

[abort on panic]: https://github.com/rust-lang/rust/pull/32900

これで上の2つのエラーを修正しました。しかし、コンパイルしようとすると別のエラーが発生します:

```bash
> cargo build
error: requires `start` lang_item
```

私達のプログラムにはエントリポイントを定義する `start` language item がありません。

## `start` attribute

`main` 関数はプログラムを実行したときに最初に呼び出される関数であると思うかもしれません。しかし、ほとんどの言語には[ランタイムシステム][runtime system]があり、これはガベージコレクション(Java など)やソフトウェアスレッド(Go のゴルーチン)などを処理します。ランタイムは自身を初期化する必要があるため、`main` 関数の前に呼び出す必要があります。これにはスタック領域の作成と正しいレジスタへの引数の配置が含まれます。

[runtime system]: https://en.wikipedia.org/wiki/Runtime_system

標準ライブラリをリンクする一般的な Rust バイナリでは、`crt0` ("C runtime zero")と呼ばれる C のランタイムライブラリで実行が開始され、C アプリケーションの環境が設定されます。その後 C ランタイムは、`start` language item で定義されている [Rust ランタイムのエントリポイント][rt::lang_start]を呼び出します。Rust にはごくわずかなランタイムしかありません。これは、スタックオーバーフローを防ぐ設定やパニック時のバックトレースの表示など、いくつかの小さな処理を行います。最後に、ランタイムは `main` 関数を呼び出します。

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

私達のフリースタンディングな実行可能ファイルは今のところ Rust ランタイムと `crt0` へアクセスできません。なので、私達は自身でエントリポイントを定義する必要があります。`start` language item を実装することは `crt0` を必要とするのでここではできません。代わりに `crt0` エントリポイントを直接上書きしなければなりません。

### エントリポイントの上書き

Rust コンパイラに通常のエントリポイントを使いたくないことを伝えるために、`#![no_main]` attribute を追加します。

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

`main` 関数を削除したことに気付いたかもしれません。`main` 関数を呼び出す基盤となるランタイムなしには置いていても意味がないからです。代わりに、OS のエントリポイントを独自の `_start` 関数で上書きしていきます:

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

Rust コンパイラが `_start` という名前の関数を実際に出力するように、`#[no_mangle]` attributeを用いて[名前修飾][name mangling]を無効にします。この attribute がないと、コンパイラはすべての関数にユニークな名前をつけるために、 `_ZN3blog_os4_start7hb173fedf945531caE` のようなシンボルを生成します。次のステップでエントリポイントとなる関数の名前をリンカに伝えるため、この属性が必要となります。

また、(指定されていない Rust の呼び出し規約の代わりに)この関数に [C の呼び出し規約][C calling convention]を使用するようコンパイラに伝えるために、関数を `extern "C"` として定義する必要があります。`_start`という名前をつける理由は、これがほとんどのシステムのデフォルトのエントリポイント名だからです。

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[C calling convention]: https://en.wikipedia.org/wiki/Calling_convention

戻り値の型である `!` は関数が発散している、つまり値を返すことができないことを意味しています。エントリポイントはどの関数からも呼び出されず、OS またはブートローダから直接呼び出されるので、これは必須です。なので、値を返す代わりに、エントリポイントは例えば OS の [`exit` システムコール][`exit` system call]を呼び出します。今回はフリースタンディングなバイナリが返されたときマシンをシャットダウンするようにすると良いでしょう。今のところ、私達は無限ループを起こすことで要件を満たします。

[`exit` system call]: https://en.wikipedia.org/wiki/Exit_(system_call)

`cargo build` を実行すると、見づらいリンカエラーが発生します。

## リンカエラー

リンカは、生成されたコードを実行可能ファイルに紐付けるプログラムです。実行可能ファイルの形式は Linux や Windows、macOS でそれぞれ異なるため、各システムにはそれぞれ異なるエラーを発生させる独自のリンカがあります。エラーの根本的な原因は同じです。リンカのデフォルト設定では、プログラムが C ランタイムに依存していると仮定していますが、実際にはしていません。

エラーを回避するためにはリンカに C ランタイムに依存しないことを伝える必要があります。これはリンカに一連の引数を渡すか、ベアメタルターゲット用にビルドすることで可能となります。

### ベアメタルターゲット用にビルドする

デフォルトでは、Rust は現在のシステム環境に合った実行可能ファイルをビルドしようとします。例えば、`x86_64` で Windows を使用している場合、Rust は `x86_64` 用の `.exe` Windows 実行可能ファイルをビルドしようとします。このような環境は「ホスト」システムと呼ばれます。

様々な環境を表現するために、Rust は [_target triple_] という文字列を使います。`rustc --version --verbose` を実行すると、ホストシステムの target triple を確認できます:

[_target triple_]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple

```bash
rustc 1.35.0-nightly (474e7a648 2019-04-07)
binary: rustc
commit-hash: 474e7a6486758ea6fc761893b1a49cd9076fb0ab
commit-date: 2019-04-07
host: x86_64-unknown-linux-gnu
release: 1.35.0-nightly
LLVM version: 8.0
```

上記の出力は `x86_64` の Linux によるものです。`host` は `x86_64-unknown-linux-gnu` です。これには CPU アーキテクチャ(`x86_64`)、ベンダー(`unknown`)、OS(`Linux`)、そして [ABI] (`gnu`)が含まれています。

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

ホストの triple 用にコンパイルすることで、Rust コンパイラとリンカは、デフォルトで C ランタイムを使用する Linux や Windows のような基盤となる OS があると想定し、それによってリンカエラーが発生します。なのでリンカエラーを回避するために、基盤となる OS を使用せずに異なる環境用にコンパイルします。

このようなベアメタル環境の例としては、`thumbv7em-none-eabihf` target triple があります。これは、[組込みシステム][embedded]を表しています。詳細は省きますが、重要なのは `none` という文字列からわかるように、 この target triple に基盤となる OS がないことです。このターゲット用にコンパイルできるようにするには、 rustup にこれを追加する必要があります:

[embedded]: https://en.wikipedia.org/wiki/Embedded_system

```bash
rustup target add thumbv7em-none-eabihf
```

これにより、この target triple 用の標準(およびコア)ライブラリのコピーがダウンロードされます。これで、このターゲット用にフリースタンディングな実行可能ファイルをビルドできます:

```bash
cargo build --target thumbv7em-none-eabihf
```

`--target` 引数を渡すことで、ベアメタルターゲット用に実行可能ファイルを[クロスコンパイル][cross compile]します。このターゲットシステムには OS がないため、リンカは C ランタイムをリンクしようとせず、ビルドはリンカエラーなしで成功します。

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

これが私達の OS カーネルを書くために使うアプローチです。`thumbv7em-none-eabihf` の代わりに、`x86_64` のベアメタル環境を表す[カスタムターゲット][custom target]を使用することもできます。詳細は次のセクションで説明します。

[custom target]: https://doc.rust-lang.org/rustc/targets/custom.html

### リンカへの引数

ベアメタルターゲット用にコンパイルする代わりに、特定の引数のセットをリンカにわたすことでリンカエラーを回避することもできます。これは私達がカーネルに使用するアプローチではありません。したがって、このセクションはオプションであり、選択肢を増やすために書かれています。表示するには以下の「リンカへの引数」をクリックしてください。

<details>

<summary>リンカへの引数</summary>

このセクションでは、Linux、Windows、および macOS で発生するリンカエラーについてと、リンカに追加の引数を渡すことによってそれらを解決する方法を説明します。実行可能ファイルの形式とリンカは OS によって異なるため、システムごとに異なる引数のセットが必要です。

#### Linux

Linux では以下のようなエラーが発生します(抜粋):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: /usr/lib/gcc/../x86_64-linux-gnu/Scrt1.o: In function `_start':
          (.text+0x12): undefined reference to `__libc_csu_fini'
          /usr/lib/gcc/../x86_64-linux-gnu/Scrt1.o: In function `_start':
          (.text+0x19): undefined reference to `__libc_csu_init'
          /usr/lib/gcc/../x86_64-linux-gnu/Scrt1.o: In function `_start':
          (.text+0x25): undefined reference to `__libc_start_main'
          collect2: error: ld returned 1 exit status
```

問題は、デフォルトで C ランタイムの起動ルーチンがリンカに含まれていることです。これは `_start` とも呼ばれます。`no_std` attribute により、C 標準ライブラリ `libc` のいくつかのシンボルが必要となります。なので、リンカはこれらの参照を解決できません。これを解決するために、リンカに `-nostartfiles` フラグを渡して、C の起動ルーチンをリンクしないようにします。

Cargo を通してリンカの attribute を渡す方法の一つに、`cargo rustc` コマンドがあります。このコマンドは `cargo build` と全く同じように動作しますが、基本となる Rust コンパイラである `rustc` にオプションを渡すことができます。`rustc` にはリンカに引数を渡す `-C link-arg` フラグがあります。新しいビルドコマンドは次のようになります:

```bash
cargo rustc -- -C link-arg=-nostartfiles
```

これで crate を Linux 上で独立した実行ファイルとしてビルドできます！

リンカはデフォルトで `_start` という名前の関数を探すので、エントリポイントとなる関数の名前を明示的に指定する必要はありません。

#### Windows

Windows では別のリンカエラーが発生します(抜粋):

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

"entry point must be defined" というエラーは、リンカがエントリポイントを見つけられていないことを意味します。Windows では、デフォルトのエントリポイント名は[使用するサブシステム][windows-subsystems]によって異なります。`CONSOLE` サブシステムの場合、リンカは `mainCRTStartup` という名前の関数を探し、`WINDOWS` サブシステムの場合は、`WinMainCRTStartup` という名前の関数を探します。デフォルトの動作を無効にし、代わりに `_start` 関数を探すようにリンカに指示するには、`/ENTRY` 引数をリンカに渡します:

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```bash
cargo rustc -- -C link-arg=/ENTRY:_start
```

引数の形式が異なることから、Windows のリンカは Linux のリンカとは全く異なるプログラムであることが分かります。

これにより、別のリンカエラーが発生します:

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

このエラーは Windows での実行可能ファイルが異なる [subsystems][windows-subsystems] を使用することができるために発生します。通常のプログラムでは、エントリポイント名に基づいて推定されます。エントリポイントが `main` という名前の場合は `CONSOLE` サブシステムが使用され、エントリポイント名が `WinMain` である場合には `WINDOWS` サブシステムが使用されます。`_start` 関数は別の名前を持っているので、サブシステムを明示的に指定する必要があります:

This error occurs because Windows executables can use different [subsystems][windows-subsystems]. For normal programs they are inferred depending on the entry point name: If the entry point is named `main`, the `CONSOLE` subsystem is used, and if the entry point is named `WinMain`, the `WINDOWS` subsystem is used. Since our `_start` function has a different name, we need to specify the subsystem explicitly:

```bash
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

ここでは `CONSOLE` サブシステムを使用しますが、`WINDOWS` サブシステムを使うこともできます。`-C link-arg` を複数渡す代わりに、スペースで区切られたリストを引数として取る `-C link-args` を渡します。

このコマンドで、実行可能ファイルが Windows 上で正しくビルドされます。

#### macOS

macOS では次のようなリンカエラーが発生します(抜粋):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

このエラーメッセージは、リンカがデフォルト名が `main` (いくつかの理由で、macOS 上ではすべての関数の前には `_` が付きます) であるエントリポイントとなる関数を見つけられないことを示しています。`_start` 関数をエントリポイントとして設定するには、`-e` というリンカ引数を渡します:

```bash
cargo rustc -- -C link-args="-e __start"
```

`-e` というフラグでエントリポイントとなる関数の名前を指定できます。macOS 上では全ての関数には `_` というプレフィックスが追加されるので、`_start` ではなく `__start` にエントリポイントを設定する必要があります。

これにより、次のようなリンカエラーが発生します:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS は[正式には静的にリンクされたバイナリをサポートしておらず][does not officially support statically linked binaries]、プログラムはデフォルトで `libSystem` ライブラリにリンクされる必要があります。これを無効にして静的バイナリをリンクするには、`-static` フラグをリンカに渡します:

[does not officially support statically linked binaries]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```bash
cargo rustc -- -C link-args="-e __start -static"
```

これでもまだ十分ではありません、3つ目のリンカエラーが発生します:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

このエラーは、macOS 上のプログラムがデフォルトで `crt0` ("C runtime zero") にリンクされるために発生します。これは Linux 上で起きたエラーと似ており、`-nostartfiles` というリンカ引数を追加することで解決できます:

```bash
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

これで 私達のプログラムを macOS 上で正しくビルドできます。

#### ビルドコマンドの統一

現時点では、ホストプラットフォームによって異なるビルドコマンドを使っていますが、これは理想的ではありません。これを回避するために、プラットフォーム固有の引数を含む `.cargo/config.toml` というファイルを作成します:

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

`rustflags` には `rustc` を呼び出すたびに自動的に追加される引数が含まれています。`.cargo/config.toml` についての詳細は[公式のドキュメント][official documentation]を確認してください。

[official documentation]: https://doc.rust-lang.org/cargo/reference/config.html

これで私達のプログラムは3つすべてのプラットフォーム上で、シンプルに `cargo build` のみでビルドすることができるようになります。

#### 私達はこれをすべきですか？

これらの手順で Linux、Windows および macOS 用の独立した実行可能ファイルをビルドすることはできますが、おそらく良い方法ではありません。その理由は、例えば `_start` 関数が呼ばれたときにスタックが初期化されるなど、まだ色々なことを前提としているからです。C ランタイムがなければ、これらの要件のうちいくつかが満たされない可能性があり、セグメンテーション違反(segfault)などによってプログラムが失敗する可能性があります。

もし既存の OS 上で動作する最小限のバイナリを作成したいなら、`libc` を使って `#[start]` attribute を[ここ][no-stdlib]で説明するとおりに設定するのが良いでしょう。

[no-stdlib]: https://doc.rust-lang.org/1.16.0/book/no-stdlib.html

</details>

## 概要

最小限の独立した Rust バイナリは次のようになります:

`src/main.rs`:

```rust
#![no_std] // Rust の標準ライブラリにリンクしない
#![no_main] // 全ての Rust レベルのエントリポイントを無効にする

use core::panic::PanicInfo;

#[no_mangle] // この関数の名前修飾をしない
pub extern "C" fn _start() -> ! {
    // リンカはデフォルトで `_start` という名前の関数を探すので、
    // この関数がエントリポイントとなる
    loop {}
}

/// この関数はパニック時に呼ばれる
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

`Cargo.toml`:

```toml
[package]
name = "crate_name"
version = "0.1.0"
authors = ["Author Name <author@example.com>"]

# the profile used for `cargo build`
[profile.dev]
panic = "abort" # disable stack unwinding on panic

# the profile used for `cargo build --release`
[profile.release]
panic = "abort" # disable stack unwinding on panic
```

このバイナリをビルドするために、`thumbv7em-none-eabihf` のようなベアメタルターゲット用にコンパイルする必要があります:

```bash
cargo build --target thumbv7em-none-eabihf
```

あるいは、追加のリンカ引数を渡してホストシステム用にコンパイルすることもできます:

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

これは独立した Rust バイナリの最小の例にすぎないことに注意してください。このバイナリは `_start` 関数が呼び出されたときにスタックが初期化されるなど、さまざまなことを前提としています。**そのため、このようなバイナリを実際に使用するには、より多くの手順が必要となります**。

## 次は？

[次の記事][next post]では、この独立したバイナリを最小限の OS カーネルにするために必要なステップを説明しています。カスタムターゲットの作成、実行可能ファイルとブートローダの組み合わせ、画面に何か文字を表示する方法について説明しています。

[next post]: @/edition-2/posts/02-minimal-rust-kernel/index.ja.md
