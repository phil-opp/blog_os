+++
title = "Rustでつくる最小のカーネル"
weight = 2
path = "ja/minimal-rust-kernel"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "7212ffaa8383122b1eb07fe1854814f99d2e1af4"
# GitHub usernames of the people that translated this post
translators = ["woodyZootopia"]
+++

この記事では、Rustで最小限の64bitカーネルを作ります。前の記事で作った[<ruby>独立した<rp> (</rp><rt>freestanding</rt><rp>) </rp></ruby>Rustバイナリ][freestanding Rust binary]を下敷きにして、何かを画面に出力する、ブータブルディスクイメージを作ります。

[freestanding Rust binary]: @/second-edition/posts/01-freestanding-rust-binary/index.ja.md

<!-- more -->

このブログの内容は [GitHub] 上で公開・開発されています。何か問題や質問などがあれば issue をたててください。また[記事下部][at the bottom]にコメントを残すこともできます。この記事の完全なソースコードは[`post-02` ブランチ][post branch]にあります。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-02

<!-- toc -->

## <ruby>起動<rp> (</rp><rt>Boot</rt><rp>) </rp></ruby>のプロセス
コンピュータを起動すると、マザーボードの[ROM]に保存されたファームウェアのコードを実行し始めます。このコードは、[<ruby>起動時の自己テスト<rp> (</rp><rt>power-on self test</rt><rp>) </rp></ruby>][power-on self-test]を実行し、使用可能なRAMを検出し、CPUとハードウェアを<ruby>事前初期化<rp> (</rp><rt>pre-initialize</rt><rp>) </rp></ruby>します。その後、<ruby>ブータブル<rp> (</rp><rt>bootable</rt><rp>) </rp></ruby>ディスクを探し、オペレーティングシステムのカーネルを<ruby>起動<rp> (</rp><rt>boot</rt><rp>) </rp></ruby>します。

[ROM]: https://ja.wikipedia.org/wiki/Read_only_memory
[power-on self-test]: https://ja.wikipedia.org/wiki/Power_On_Self_Test

x86には2つのファームウェアの標準規格があります："Basic Input/Output System" (**[BIOS]**) と、より新しい "Unified Extensible Firmware Interface" (**[UEFI]**) です。BIOS規格は古く時代遅れですが、シンプルでありすべてのx86のマシンで1980年代からよくサポートされています。対して、UEFIはより現代的でずっと多くの機能を持っていますが、セットアップが複雑です（少なくとも私はそう思います）。

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[UEFI]: https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

今の所、このブログではBIOSしかサポートしていませんが、UEFIのサポートも計画中です。お手伝いいただける場合は、[GitHubのissue](https://github.com/phil-opp/blog_os/issues/349)をご覧ください。

### BIOSの起動
ほぼすべてのx86システムがBIOSによる起動をサポートしています。これは近年のUEFIベースのマシンも例外ではなく、それらはエミュレートされたBIOSを使います。前世紀のすべてのマシンにも同じブートロジックが使えるなんて素晴らしいですね。しかし、この広い互換性は、BIOSによる起動の最大の欠点でもあるのです。というのもこれは、1980年代の化石のようなブートローダーを動かすために、CPUが[<ruby>リアルモード<rp> (</rp><rt>real mode</rt><rp>) </rp></ruby>][real mode]と呼ばれる16bit互換モードにされてしまうということを意味しているからです。

まあ順を追って見ていくこととしましょう。

コンピュータを起動すると、マザーボードにある特殊なフラッシュメモリからBIOSを読み込みます。BIOSは自己テストとハードウェアの初期化ルーチンを実行し、ブータブルディスクを探します。ディスクが見つかると、 **<ruby>ブートローダー<rp> (</rp><rt>bootloader</rt><rp>) </rp></ruby>** と呼ばれる、その先頭512バイトに保存された実行可能コードへと操作権が移ります。多くのブートローダーは512バイトより大きいですから、普通ブートローダーは、512バイトに収まる小さな最初のステージと、その最初のステージによって読み込まれる第2ステージに分けられています。

ブートローダーはディスク内のカーネルイメージの場所を特定し、メモリに読み込まなければなりません。また、CPUを16bitの[リアルモード][real mode]から32bitの[<ruby>プロテクトモード<rp> (</rp><rt>protected mode</rt><rp>) </rp></ruby>][protected mode]へ、そして64bitの[<ruby>ロングモード<rp> (</rp><rt>long mode</rt><rp>) </rp></ruby>][long mode]――64bitレジスタとすべてのメインメモリが利用可能になります――へと変更しなければなりません。3つ目の仕事は、特定の情報（例えばメモリーマップなどです）をBIOSから聞き出し、OSのカーネルに渡すことです。

[real mode]: https://ja.wikipedia.org/wiki/リアルモード
[protected mode]: https://ja.wikipedia.org/wiki/プロテクトモード
[long mode]: https://en.wikipedia.org/wiki/Long_mode
[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation

ブートローダーを書くのにはアセンブリ言語を必要とするうえ、「何も考えずにプロセッサーのこのレジスターにこの値を書き込んでください」みたいな勉強の役に立たない作業がたくさんあるので、ちょっと面倒くさいです。ですのでこの記事ではブートローダーの制作については飛ばして、代わりに[bootimage]という、自動でカーネルの前にブートローダを置いてくれるツールを使いましょう。

[bootimage]: https://github.com/rust-osdev/bootimage

自前のブートローダーを作ることに興味がある人もご期待下さい、これに関する記事も計画中です！

#### マルチブートの標準規格
すべてのオペレーティングシステムが、自前で一つのOSにしか対応していないブートローダーを実装するということを避けるために、[フリーソフトウェア財団][Free Software Foundation]が[Multiboot]というブートローダーの公開標準規格を策定しています。この標準規格では、ブートローダーとオペレーティングシステムのインターフェースが定義されており、どのMultibootに準拠したブートローダーでもすべてのMultibootに準拠したオペレーティングシステムが読み込めるようになっています。そのリファレンス実装として、Linuxシステムで一番人気のブートローダーである[GNU GRUB]があります。

[Free Software Foundation]: https://en.wikipedia.org/wiki/Free_Software_Foundation
[Multiboot]: https://wiki.osdev.org/Multiboot
[GNU GRUB]: https://en.wikipedia.org/wiki/GNU_GRUB

カーネルをMultibootに準拠させるには、カーネルファイルの先頭にいわゆる[Multiboot header]を挿入するだけでいいです。このおかげで、OSをGRUBで起動するのはとても簡単です。しかし、GRUBとMultiboot標準規格にはいくつか問題もあります：

[Multiboot header]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format

- これらは32bitプロテクトモードしかサポートしていません。そのため、64bitロングモードに変更するためのCPUの設定は依然行う必要があります。
- これらは、カーネルではなくブートローダーがシンプルになるように設計されています。例えば、カーネルは[通常とは異なるデフォルトページサイズ][adjusted default page size]でリンクされる必要があり、そうしないとGRUBはMultiboot headerを見つけることができません。他にも、カーネルに渡される[<ruby>ブート情報<rp> (</rp><rt>boot information</rt><rp>) </rp></ruby>][boot information]は、クリーンな抽象化を与えてくれず、たくさんのアーキテクチャ依存の構造を含んでいます。
- GRUBもMultiboot標準規格もドキュメントが充実していません。
- カーネルファイルからブータブルディスクイメージを作るには、ホストシステムにGRUBがインストールされている必要があります。これにより、MacとWindows上での開発は比較的難しくなっています。

[adjusted default page size]: https://wiki.osdev.org/Multiboot#Multiboot_2
[boot information]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format

これらの欠点を考慮し、私達はGRUBとMultiboot標準規格を使わないことに決めました。しかし、あなたのカーネルをGRUBシステム上で読み込めるように、私達の[bootimage]ツールにMultibootのサポートを追加することも計画しています。Multiboot準拠なカーネルを書きたい場合は、このブログシリーズの[第1版][first edition]をご覧ください。

[first edition]: @/first-edition/_index.md

### UEFI

（今の所UEFIのサポートは提供していませんが、ぜひともしたいと思っています！お手伝いいただける場合は、 [Github issue](https://github.com/phil-opp/blog_os/issues/349)で教えてください）

## 最小のカーネル
どのようにコンピュータが起動するのかについてざっくりと理解できたので、自前で最小のカーネルを書いてみましょう。目標は、起動したら画面に"Hello, World!"と出力するようなディスクイメージを作ることです。そのためには、前の記事の[<ruby>独立した<rp> (</rp><rt>freestanding</rt><rp>) </rp></ruby>Rustバイナリ][freestanding Rust binary]を基礎として作ります。

覚えていますか、この独立したバイナリは`cargo`を使ってビルドしましたが、オペレーティングシステムに依って異なるエントリポイント名とコンパイルフラグが必要なのでした。これは`cargo`は標準では **ホストシステム**（あなたの使っているシステム）向けにビルドするからです。例えばWindows上で走るカーネルなんてあまり意味がありませんから、これは私達の望む動作ではありません。代わりに、明確に定義された **ターゲットシステム** 向けにコンパイルしたいです。

### RustのNightly版をインストールする
Rustには**stable**、**beta**、**nightly**の3つのリリースチャンネルがあります。Rust Bookはこれらの3つのチャンネルの違いをとても良く説明しているので、一度[確認してみてください](https://doc.rust-jp.rs/book-ja/appendix-07-nightly-rust.html)。オペレーティングシステムをビルドするには、nightlyチャンネルでしか利用できないいくつかの実験的機能を使う必要があるので、RustのNightly版をインストールする必要があります。

Rustの実行環境を管理するのには、[rustup]を強くおすすめします。Nightly、beta、stable版のコンパイラをそれぞれインストールすることができますし、アップデートするのも簡単です。現在のディレクトリにnightlyコンパイラを使うようにするには、`rustup override set nightly`と実行してください。もしくは、`rust-toolchain`というファイルに`nightly`と記入してプロジェクトのルートディレクトリに置くのでも良いです。Nightly版を使っていることは、`rustc --version`と実行することで確かめられます。表示されるバージョン名の末尾に`-nightly`とあるはずです。

[rustup]: https://www.rustup.rs/

nightlyコンパイラでは、いわゆる**feature flag**をファイルの先頭につけることで、いろいろな実験的機能を使うことを選択することができます。例えば、`#![feature(asm)]`を`main.rs`の先頭につけることで、インラインアセンブリのための実験的な[`asm!`マクロ][`asm!` macro]を有効化することができます。ただし、これらの実験的機能は全くもって<ruby>不安定<rp> (</rp><rt>unstable</rt><rp>) </rp></ruby>であり、将来のRustの版においては事前の警告なく変更されたり取り除かれたりすることがあることに注意してください。このため、絶対に必要なときにのみこれらを使うことにします。

[`asm!` macro]: https://doc.rust-lang.org/unstable-book/library-features/asm.html

### ターゲットの仕様
Cargoは`--target`パラメータを使ってさまざまなターゲットをサポートします。ターゲットはいわゆる[target <ruby>triple<rp> (</rp><rt>3つ組</rt><rp>) </rp></ruby>][target triple]によって表されます。これはCPUアーキテクチャ、製造元、オペレーティングシステム、そして[ABI]を表します。例えば、`x86_64-unknown-linux-gnu`というtarget tripleは、`x86_64`のCPU、製造元不明、GNU ABIのLinuxオペレーティングシステム向けのシステムを表します。Rustは[多くの種類のtarget triple][platform-support]をサポートしており、その中にはAndroidのための`arm-linux-androideabi`や[WebAssemblyのための`wasm32-unknown-unknown`](https://www.hellorust.com/setup/wasm-target/)があります。

[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[ABI]: https://stackoverflow.com/a/2456882
[platform-support]: https://forge.rust-lang.org/release/platform-support.html
[custom-targets]: https://doc.rust-lang.org/nightly/rustc/targets/custom.html

しかしながら、私達のターゲットシステムには、いくつか特殊な設定パラメータが必要になります（例えば、その下ではOSが走っていない、など）。なので、[既存のtarget triple][platform-support]はどれも当てはまりません。ありがたいことに、RustではJSONファイルを使って[独自のターゲット][custom-targets]を定義できます。例えば、`x86_64-unknown-linux-gnu`というターゲットを表すJSONファイルはこんな感じです。

```json
{
    "llvm-target": "x86_64-unknown-linux-gnu",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "linux",
    "executables": true,
    "linker-flavor": "gcc",
    "pre-link-args": ["-m64"],
    "morestack": false
}
```

ほとんどのフィールドはLLVMがそのプラットフォーム向けのコードを生成するために必要なものです。例えば、[`data-layout`]フィールドは種々の整数、浮動小数点数、ポインタ型の大きさを定義しています。次に、`target-pointer-width`のような、条件付きコンパイルに用いられるフィールドがあります。第3の種類のフィールドはクレートがどのようにビルドされるべきかを定義します。例えば、`pre-link-args`フィールドは[<ruby>リンカ<rp> (</rp><rt>linker</rt><rp>) </rp></ruby>][linker]に渡される引数を指定しています。

[`data-layout`]: https://llvm.org/docs/LangRef.html#data-layout
[linker]: https://en.wikipedia.org/wiki/Linker_(computing)

私達のカーネルも`x86_64`のシステムをターゲットとするので、私達のターゲット仕様も上のものと非常によく似たものになるでしょう。`x86_64-blog_os.json`というファイル（お好きな名前を選んでください）を作り、共通する要素を埋めるところから始めましょう。

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executables": true
}
```

ベアメタル環境で実行するので、`llvm-target`のOSを変え、`os`フィールドを`none`にしたことに注目してください。

以下の、ビルドに関係する項目を追加します。


```json
"linker-flavor": "ld.lld",
"linker": "rust-lld",
```

私達のカーネルをリンクするのに、プラットフォーム標準の（Linuxターゲットをサポートしていないかもしれない）リンカではなく、Rustに付属しているクロスプラットフォームの[LLD]リンカを使用します。

[LLD]: https://lld.llvm.org/

```json
"panic-strategy": "abort",
```

この設定は、ターゲットが<ruby>パニック<rp> (</rp><rt>panic</rt><rp>) </rp></ruby>時の[stack unwinding]をサポートしていないので、プログラムは代わりに直接<ruby>中断<rp> (</rp><rt>abort</rt><rp>) </rp></ruby>しなければならないということを指定しています。これは、Cargo.tomlに`panic = "abort"`という設定を書くのに等しいですから、そこからそれを取り除いて良いです（このターゲット設定は、Cargo.tomlの設定と異なり、このあとやる`core`ライブラリの再コンパイルにも適用されます。ですので、Cargo.tomlに設定するほうが好みだったとしても、この設定を追加するようにしてください）。

[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php

```json
"disable-redzone": true,
```

カーネルを書いているので、いつかの時点で<ruby>割り込み<rp> (</rp><rt>interrupt</rt><rp>) </rp></ruby>を処理しなければならなくなるでしょう。これを安全に行うために、 **"red zone"** と呼ばれる、ある種のスタックポインタ最適化を無効化する必要があります。こうしないと、スタックの<ruby>破損<rp> (</rp><rt>corruption</rt><rp>) </rp></ruby>を引き起こしてしまうだろうからです。より詳しくは、[red zoneの無効化][disabling the red zone]という別記事をご覧ください。

[disabling the red zone]: @/second-edition/posts/02-minimal-rust-kernel/disable-red-zone/index.md

```json
"features": "-mmx,-sse,+soft-float",
```

`features`フィールドは、ターゲットの<ruby>機能<rp> (</rp><rt>features</rt><rp>) </rp></ruby>を有効化/無効化します。マイナスを前につけることで`mmx`と`sse`という機能を無効化し、プラスを前につけることで`soft-float`という機能を有効化しています。それぞれのフラグの間にスペースは入れてはならず、もしそうするとLLVMが機能文字列の解釈に失敗していまうということに注意してください。

`mmx`と`sse`という機能は、[Single Instruction Multiple Data (SIMD)]命令をサポートするかを決定します。この命令は、しばしばプログラムを著しく速くしてくれます。しかし、大きなSIMDレジスタをOSカーネルで使うことは性能上の問題に繋がります。 その理由は、カーネルは、割り込まれたプログラムを再開する前に、すべてのレジスタを元に戻さないといけないからです。これは、カーネルがSIMDの状態のすべてを、システムコールやハードウェア割り込みがあるたびにメインメモリに保存しないといけないということを意味します。SIMDの状態情報はとても巨大（512〜1600 bytes）で、割り込みは非常に頻繁に起こるかもしれないので、保存・復元の操作がこのように追加されるのは性能にかなりの悪影響を及ぼします。これを避けるために、（カーネルの上で走っているアプリケーションではなく！）カーネルではSIMDを無効化するのです。

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

SIMDを無効化することによる問題に、`x86_64`における浮動小数点演算は標準ではSIMDレジスタを必要とするということがあります。この問題を解決するため、`soft-float`機能を追加します。これは、すべての浮動小数点演算を通常の整数に基づいたソフトウェア上の関数を使ってエミュレートするというものです。

より詳しくは、[SIMDを無効化する](@/second-edition/posts/02-minimal-rust-kernel/disable-simd/index.md)ことに関する私達の記事を読んでください。

#### まとめると
私達のターゲット仕様ファイルはこんなふうになっているはずです。

```json
{
  "llvm-target": "x86_64-unknown-none",
  "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
  "arch": "x86_64",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "os": "none",
  "executables": true,
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
  "panic-strategy": "abort",
  "disable-redzone": true,
  "features": "-mmx,-sse,+soft-float"
}
```

### カーネルをビルドする
私達の新しいターゲットのコンパイルにはLinuxの慣習を使います（理由は知りません、LLVMの既定ってだけじゃないでしょうか）。つまり、[前の記事][previous post]で説明したように`_start`という名前のエントリポイントが要るということです。

[previous post]: @/second-edition/posts/01-freestanding-rust-binary/index.ja.md

```rust
// src/main.rs

#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}
```

あなたのホストOSがなんであるかにかかわらず、エントリポイントは`_start`という名前である必要があることに注意してください。

これで、私達の新しいターゲットのためのカーネルを、JSONファイル名を`--target`として渡すことでビルドできるようになりました。

```
> cargo build --target x86_64-blog_os.json

error[E0463]: can't find crate for `core`
```

失敗しましたね！エラーはRustコンパイラが[`core`ライブラリ][`core` library]を見つけられなくなったと言っています。このライブラリは、`Result`、`Option`、イテレータのような基本的なRustの型を持っており、暗黙のうちにすべての`no_std`なクレートにリンクされています。

[`core` library]: https://doc.rust-lang.org/nightly/core/index.html

問題は、coreライブラリはRustコンパイラと一緒に<ruby>コンパイル済み<rp> (</rp><rt>precompiled</rt><rp>) </rp></ruby>ライブラリとして配布されているということです。そのため、これは、私達独自のターゲットではなく、サポートされたhost triple（例えば `x86_64-unknown-linux-gnu`）にのみ使えるのです。他のターゲットのためにコードをコンパイルしようと思ったら、`core`をそれらのターゲットに向けて再コンパイルする必要があります。

#### `build-std`オプション

ここでcargoの[`build-std`機能][`build-std` feature]の出番です。これを使うと`core`やその他の標準ライブラリクレートについて、Rustインストール時に一緒についてくるコンパイル済みバージョンを使う代わりに、必要に応じて再コンパイルすることができます。この機能はとても新しく、まだ完成していないので、「<ruby>不安定<rp> (</rp><rt>unstable</rt><rp>) </rp></ruby>」マークが付いており、[nightly Rustコンパイラ][nightly Rust compilers]でのみ利用可能です。

[`build-std` feature]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std
[nightly Rust compilers]: #installing-rust-nightly

この機能を使うためには、[cargoの設定][cargo configuration]ファイルを`.cargo/config.toml`に作り、次の内容を書きましょう。

```toml
# in .cargo/config.toml

[unstable]
build-std = ["core", "compiler_builtins"]
```

これはcargoに`core`と`compiler_builtins`ライブラリを再コンパイルするよう命令します。後者は必要なのは`core`がこれに依存しているためです。 これらのライブラリを再コンパイルするためには、cargoはRustのソースコードにアクセスできる必要があります。これは`rustup component add rust-src`でインストールできます。

<div class="note">

**注意:** `unstable.build-std`設定キーを使うには、少なくとも2020-07-15以降のRust nightlyが必要です。

</div>

`unstable.build-std`設定キーをセットし、`rust-src`コンポーネントをインストールしたなら、ビルドコマンドをもう一度実行しましょう。

```
> cargo build --target x86_64-blog_os.json
   Compiling core v0.0.0 (/…/rust/src/libcore)
   Compiling rustc-std-workspace-core v1.99.0 (/…/rust/src/tools/rustc-std-workspace-core)
   Compiling compiler_builtins v0.1.32
   Compiling blog_os v0.1.0 (/…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

今回は、`cargo build`が`core`、`rustc-std-workspace-core` (`compiler_builtins`の依存です)、そして `compiler_builtins`を私達のカスタムターゲット向けに再コンパイルしているということがわかります。

#### Memory-Related Intrinsics

The Rust compiler assumes that a certain set of built-in functions is available for all systems. Most of these functions are provided by the `compiler_builtins` crate that we just recompiled. However, there are some memory-related functions in that crate that are not enabled by default because they are normally provided by the C library on the system. These functions include `memset`, which sets all bytes in a memory block to a given value, `memcpy`, which copies one memory block to another, and `memcmp`, which compares two memory blocks. While we didn't need any of these functions to compile our kernel right now, they will be required as soon as we add some more code to it (e.g. when copying structs around).

Since we can't link to the C library of the operating system, we need an alternative way to provide these functions to the compiler. One possible approach for this could be to implement our own `memset` etc. functions and apply the `#[no_mangle]` attribute to them (to avoid the automatic renaming during compilation). However, this is dangerous since the slightest mistake in the implementation of these functions could lead to undefined behavior. For example, you might get an endless recursion when implementing `memcpy` using a `for` loop because `for` loops implicitly call the [`IntoIterator::into_iter`] trait method, which might call `memcpy` again. So it's a good idea to reuse existing well-tested implementations instead.

[`IntoIterator::into_iter`]: https://doc.rust-lang.org/stable/core/iter/trait.IntoIterator.html#tymethod.into_iter

Fortunately, the `compiler_builtins` crate already contains implementations for all the needed functions, they are just disabled by default to not collide with the implementations from the C library. We can enable them by setting cargo's [`build-std-features`] flag to `["compiler-builtins-mem"]`. Like the `build-std` flag, this flag can be either passed on the command line as `-Z` flag or configured in the `unstable` table in the `.cargo/config.toml` file. Since we always want to build with this flag, the config file option makes more sense for us:

[`build-std-features`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features

```toml
# in .cargo/config.toml

[unstable]
build-std-features = ["compiler-builtins-mem"]
```

(Support for the `compiler-builtins-mem` feature was only [added very recently](https://github.com/rust-lang/rust/pull/77284), so you need at least Rust nightly `2020-09-30` for it.)

Behind the scenes, this flag enables the [`mem` feature] of the `compiler_builtins` crate. The effect of this is that the `#[no_mangle]` attribute is applied to the [`memcpy` etc. implementations] of the crate, which makes them available to the linker. It's worth noting that these functions are [not optimized] right now, so their performance might not be the best, but at least they are correct. For `x86_64`, there is an open pull request to [optimize these functions using special assembly instructions][memcpy rep movsb].

[`mem` feature]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/Cargo.toml#L51-L52
[`memcpy` etc. implementations]: (https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/src/mem.rs#L12-L69)
[not optimized]: https://github.com/rust-lang/compiler-builtins/issues/339
[memcpy rep movsb]: https://github.com/rust-lang/compiler-builtins/pull/365

With this change, our kernel has valid implementations for all compiler-required functions, so it will continue to compile even if our code gets more complex.

#### Set a Default Target

To avoid passing the `--target` parameter on every invocation of `cargo build`, we can override the default target. To do this, we add the following to our [cargo configuration] file at `.cargo/config.toml`:

[cargo configuration]: https://doc.rust-lang.org/cargo/reference/config.html

```toml
# in .cargo/config.toml

[build]
target = "x86_64-blog_os.json"
```

This tells `cargo` to use our `x86_64-blog_os.json` target when no explicit `--target` argument is passed. This means that we can now build our kernel with a simple `cargo build`. For more information on cargo configuration options, check out the [official documentation][cargo configuration].

We are now able to build our kernel for a bare metal target with a simple `cargo build`. However, our `_start` entry point, which will be called by the boot loader, is still empty. It's time that we output something to screen from it.

### 画面に出力する
現在の段階で画面に文字を出力する最も簡単な方法は[VGAテキストバッファ][VGA text buffer]です。これは画面に出力されている内容を持っているVGAハードウェアにマップされた特殊なメモリです。通常、これは25行からなり、それぞれの行は80文字セルからなります。それぞれの文字セルは、背景色と前景色付きのASCII文字を表示します。画面出力はこんなふうに見えます：

[VGA text buffer]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode

![screen output for common ASCII characters](https://upload.wikimedia.org/wikipedia/commons/f/f8/Codepage-437.png)

次の記事では、VGAバッファの正確なレイアウトについて議論し、このためのちょっとしたドライバも書きます。"Hello World!"を出力するためには、バッファが<ruby>番地<rp> (</rp><rt>address</rt><rp>) </rp></ruby>`0xb8000`にあり、それぞれの文字セルはASCIIのバイトと色のバイトからなることだけ知っていればよいです。

実装はこんな感じになります：

```rust
static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}
```

まず、`0xb8000`という整数を[生ポインタ][raw pointer]にキャストします。次に[<ruby>静的<rp> (</rp><rt>static</rt><rp>) </rp></ruby>][static]な`HELLO`という[バイト列][byte string]変数の要素に対し[イテレート][iterate]します。[`enumerate`]メソッドを使って、ループと一緒に「走る」変数`i`を追加しています。ループの内部では、[`offset`]メソッドを使って文字列のバイトと対応する色のバイト（`0xb`は明るいシアン色）を書き込んでいます。

[iterate]: https://doc.rust-jp.rs/book-ja/ch13-02-iterators.html
[static]: https://doc.rust-jp.rs/book-ja/ch10-03-lifetime-syntax.html#静的ライフタイム
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate
[byte string]: https://doc.rust-lang.org/reference/tokens.html#byte-string-literals
[raw pointer]: https://doc.rust-jp.rs/book-ja/ch19-01-unsafe-rust.html#生ポインタを参照外しする
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

すべてのメモリへの書き込みの周りを、[<ruby>`unsafe`<rp> (</rp><rt>非安全</rt><rp>) </rp></ruby>][`unsafe`]ブロックが囲んでいることに注意してください。この理由は、私達の作った生ポインタが合法であることをRustコンパイラが証明できないからです。生ポインタはどんな場所でも指しうるので、データの破損につながるかもしれません。これらの操作を`unsafe`ブロックに入れることで、私達はこれが合法だと確信しているとコンパイラに伝えているのです。ただし、`unsafe`ブロックはRustの安全性チェックを消すわけではなく、[5つのことが追加でできるようになる][five additional things]だけということに注意してください。（訳注：翻訳時点では、リンク先のThe Rust book日本語版には「追加でできるようになること」は4つしか書かれていません）

[`unsafe`]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html
[five additional things]: https://doc.rust-jp.rs/book-ja/ch19-01-unsafe-rust.html#unsafeの強大な力superpower

強調しておきたいのですが、 **このような機能はRustでプログラミングするときには使いたくありません！** unsafeブロック内で生ポインタを扱うと非常にしくじりやすいです。たとえば、注意不足でバッファの終端のさらに奥に書き込みを行ってしまったりするかもしれません。

ですので、`unsafe`の使用はできるかぎり最小限にしたいです。これをするために、Rustでは安全な<ruby>abstraction<rp> (</rp><rt>抽象化されたもの</rt><rp>) </rp></ruby>を作ることができます。たとえば、VGAバッファ型を作り、この中にすべてのunsafeな操作を<ruby>包み<rp> (</rp><rt>encapsulate</rt><rp>) </rp></ruby>、外側から誤ったことをするのを**不可能**にすることができるでしょう。こうすれば、`unsafe`の量を最小限にすることができ、[メモリ安全性][memory safety]を侵していないことを確信することができます。そのような安全なVGAバッファの抽象化を次の記事で作ります。

[memory safety]: https://ja.wikipedia.org/wiki/メモリ安全性

## Running our Kernel

Now that we have an executable that does something perceptible, it is time to run it. First, we need to turn our compiled kernel into a bootable disk image by linking it with a bootloader. Then we can run the disk image in the [QEMU] virtual machine or boot it on real hardware using a USB stick.

### Creating a Bootimage

To turn our compiled kernel into a bootable disk image, we need to link it with a bootloader. As we learned in the [section about booting], the bootloader is responsible for initializing the CPU and loading our kernel.

[section about booting]: #the-boot-process

Instead of writing our own bootloader, which is a project on its own, we use the [`bootloader`] crate. This crate implements a basic BIOS bootloader without any C dependencies, just Rust and inline assembly. To use it for booting our kernel, we need to add a dependency on it:

[`bootloader`]: https://crates.io/crates/bootloader

```toml
# in Cargo.toml

[dependencies]
bootloader = "0.9.8"
```

Adding the bootloader as dependency is not enough to actually create a bootable disk image. The problem is that we need to link our kernel with the bootloader after compilation, but cargo has no support for [post-build scripts].

[post-build scripts]: https://github.com/rust-lang/cargo/issues/545

To solve this problem, we created a tool named `bootimage` that first compiles the kernel and bootloader, and then links them together to create a bootable disk image. To install the tool, execute the following command in your terminal:

```
cargo install bootimage
```

For running `bootimage` and building the bootloader, you need to have the `llvm-tools-preview` rustup component installed. You can do so by executing `rustup component add llvm-tools-preview`.

After installing `bootimage` and adding the `llvm-tools-preview` component, we can create a bootable disk image by executing:

```
> cargo bootimage
```

We see that the tool recompiles our kernel using `cargo build`, so it will automatically pick up any changes you make. Afterwards it compiles the bootloader, which might take a while. Like all crate dependencies it is only built once and then cached, so subsequent builds will be much faster. Finally, `bootimage` combines the bootloader and your kernel to a bootable disk image.

After executing the command, you should see a bootable disk image named `bootimage-blog_os.bin` in your `target/x86_64-blog_os/debug` directory. You can boot it in a virtual machine or copy it to an USB drive to boot it on real hardware. (Note that this is not a CD image, which have a different format, so burning it to a CD doesn't work).

#### How does it work?
The `bootimage` tool performs the following steps behind the scenes:

- It compiles our kernel to an [ELF] file.
- It compiles the bootloader dependency as a standalone executable.
- It links the bytes of the kernel ELF file to the bootloader.

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
[rust-osdev/bootloader]: https://github.com/rust-osdev/bootloader

When booted, the bootloader reads and parses the appended ELF file. It then maps the program segments to virtual addresses in the page tables, zeroes the `.bss` section, and sets up a stack. Finally, it reads the entry point address (our `_start` function) and jumps to it.

### Booting it in QEMU

We can now boot the disk image in a virtual machine. To boot it in [QEMU], execute the following command:

[QEMU]: https://www.qemu.org/

```
> qemu-system-x86_64 -drive format=raw,file=target/x86_64-blog_os/debug/bootimage-blog_os.bin
warning: TCG doesn't support requested feature: CPUID.01H:ECX.vmx [bit 5]
```

This opens a separate window with that looks like this:

![QEMU showing "Hello World!"](qemu.png)

We see that our "Hello World!" is visible on the screen.

### Real Machine

It is also possible to write it to an USB stick and boot it on a real machine:

```
> dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

Where `sdX` is the device name of your USB stick. **Be careful** to choose the correct device name, because everything on that device is overwritten.

After writing the image to the USB stick, you can run it on real hardware by booting from it. You probably need to use a special boot menu or change the boot order in your BIOS configuration to boot from the USB stick. Note that it currently doesn't work for UEFI machines, since the `bootloader` crate has no UEFI support yet.

### Using `cargo run`

To make it easier to run our kernel in QEMU, we can set the `runner` configuration key for cargo:

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```

The `target.'cfg(target_os = "none")'` table applies to all targets that have set the `"os"` field of their target configuration file to `"none"`. This includes our `x86_64-blog_os.json` target. The `runner` key specifies the command that should be invoked for `cargo run`. The command is run after a successful build with the executable path passed as first argument. See the [cargo documentation][cargo configuration] for more details.

The `bootimage runner` command is specifically designed to be usable as a `runner` executable. It links the given executable with the project's bootloader dependency and then launches QEMU. See the [Readme of `bootimage`] for more details and possible configuration options.

[Readme of `bootimage`]: https://github.com/rust-osdev/bootimage

Now we can use `cargo run` to compile our kernel and boot it in QEMU.

## What's next?

In the next post, we will explore the VGA text buffer in more detail and write a safe interface for it. We will also add support for the `println` macro.
