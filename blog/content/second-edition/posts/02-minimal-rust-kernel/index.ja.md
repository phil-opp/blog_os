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

この記事では、Rustで最小限の64bitカーネルを作ります。前の記事で作った[フリースタンディングなRustバイナリ][freestanding Rust binary]を下敷きにして、何かを画面に出力する、ブータブルディスクイメージを作ります。

[freestanding Rust binary]: @/second-edition/posts/01-freestanding-rust-binary/index.ja.md

<!-- more -->

このブログの内容は [GitHub] 上で公開・開発されています。何か問題や質問などがあれば issue をたててください。また[記事下部][at the bottom]にコメントを残すこともできます。この記事の完全なソースコードは[`post-02` ブランチ][post branch]にあります。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-02

<!-- toc -->

## <ruby>起動<rp> (</rp><rt>Boot</rt><rp>) </rp></ruby>のプロセス
コンピュータを起動すると、マザーボードの [ROM] に保存されたファームウェアのコードを実行し始めます。このコードは、[<ruby>起動時の自己テスト<rp> (</rp><rt>power-on self test</rt><rp>) </rp></ruby>][power-on self-test]を実行し、使用可能なRAMを検出し、CPUとハードウェアを<ruby>事前初期化<rp> (</rp><rt>pre-initialize</rt><rp>) </rp></ruby>します。その後、<ruby>ブータブル<rp> (</rp><rt>bootable</rt><rp>) </rp></ruby>ディスクを探し、オペレーティングシステムのカーネルを<ruby>起動<rp> (</rp><rt>boot</rt><rp>) </rp></ruby>します。

[ROM]: https://ja.wikipedia.org/wiki/Read_only_memory
[power-on self-test]: https://ja.wikipedia.org/wiki/Power_On_Self_Test

x86には2つのファームウェアの標準規格があります："Basic Input/Output System" (**[BIOS]**) と、より新しい "Unified Extensible Firmware Interface" (**[UEFI]**) です。BIOS規格は古く時代遅れですが、シンプルでありすべてのx86のマシンで1980年代からよくサポートされています。対して、UEFIはより現代的でずっと多くの機能を持っていますが、セットアップが複雑です（少なくとも私はそう思います）。

[BIOS]: https://ja.wikipedia.org/wiki/Basic_Input/Output_System
[UEFI]: https://ja.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

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

[Free Software Foundation]: https://ja.wikipedia.org/wiki/フリーソフトウェア財団
[Multiboot]: https://wiki.osdev.org/Multiboot
[GNU GRUB]: https://ja.wikipedia.org/wiki/GNU_GRUB

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
[linker]: https://ja.wikipedia.org/wiki/リンケージエディタ

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

<ruby>ベアメタル<rp> (</rp><rt>bare metal</rt><rp>) </rp></ruby>環境で実行するので、`llvm-target`のOSを変え、`os`フィールドを`none`にしたことに注目してください。

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

[Single Instruction Multiple Data (SIMD)]: https://ja.wikipedia.org/wiki/SIMD

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

問題は、coreライブラリはRustコンパイラと一緒に<ruby>コンパイル済み<rp> (</rp><rt>precompiled</rt><rp>) </rp></ruby>ライブラリとして配布されているということです。そのため、これは、私達独自のターゲットではなく、サポートされているhost triple（例えば `x86_64-unknown-linux-gnu`）にのみ使えるのです。他のターゲットのためにコードをコンパイルしようと思ったら、`core`をそれらのターゲットに向けて再コンパイルする必要があります。

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

#### メモリ関係の<ruby>組み込み<rp> (</rp><rt>intrinsic</rt><rp>) </rp></ruby>関数

Rustコンパイラは、すべてのシステムにおいて、特定の組み込み関数が利用可能であるということを前提にしています。それらの関数の多くは、私達がちょうど再コンパイルした`compiler_builtins`クレートによって提供されています。しかしながら、通常システムのCライブラリに提供されているので標準では有効化されていない、メモリ関係の関数がいくつかあります。それらの関数には、メモリブロック内のすべてのバイトを与えられた値にセットする`memset`、メモリーブロックを他のブロックへとコピーする`memcpy`、2つのメモリーブロックを比較する`memcmp`などがあります。これらの関数はどれも、現在の段階で我々のカーネルをコンパイルするのに必要というわけではありませんが、コードを追加していくとすぐに必要になるでしょう（たとえば、構造体をコピーしたりとか）。

オペレーティングシステムのCライブラリにリンクすることはできませんので、これらの関数をコンパイラに与えてやる別の方法が必要になります。このための方法として考えられるものの一つが、自前で`memset`を実装し、（コンパイル中の自動リネームを防ぐため）`#[no_mangle]`アトリビュートをこれらに適用することでしょう。しかし、こうすると、これらの関数の実装のちょっとしたミスが未定義動作に繋がりうるため危険です。たとえば、`for`ループを使って`memcpy`を実装すると無限再帰を起こしてしまうかもしれません。なぜなら、`for`ループは暗黙のうちに[`IntoIterator::into_iter`]トレイトメソッドを呼び出しており、これは`memcpy`を再び呼び出しているかもしれないからです。なので、代わりに、既存のよくテストされた実装を再利用するのが良いでしょう。

[`IntoIterator::into_iter`]: https://doc.rust-lang.org/stable/core/iter/trait.IntoIterator.html#tymethod.into_iter

ありがたいことに、`compiler_builtins`クレートにはこれらの必要な関数すべての実装が含まれており、標準ではCライブラリの実装と競合しないように無効化されているだけなのです。有効化するにはcargoの[`build-std-features`]フラグを`["computer-builtins-mem"]`にセットすればいいです。`build-std`フラグと同じように、このフラグはコマンドラインで`-Z`フラグとして渡すこともできれば、`.cargo/config.toml`ファイルの`unstable`テーブルで設定することもできます。ビルド時は常にこのフラグをセットしたいので、設定ファイルを使うほうが良いでしょう：

[`build-std-features`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features

```toml
# in .cargo/config.toml

[unstable]
build-std-features = ["compiler-builtins-mem"]
```

（`compiler-builtins-mem`機能のサポートが追加されたのは[すごく最近](https://github.com/rust-lang/rust/pull/77284)なので、`2019-09-30`以降のRust nightlyが必要です）

このとき、裏で`compiler_builtins`クレートの[`mem`機能][`mem` feature]が有効化されています。これにより、このクレートの[`memcpy`などの実装][`memcpy` etc. implementations]に`#[no_mangle]`アトリビュートが適用され、リンカがこれらを利用できるようになっています。これらの関数は今のところ[最適化されておらず][not optimized]、性能は最高ではないかもしれないということは知っておく価値があるでしょう。ですが、少なくともこれらは正しいです。`x86_64`については、[これらの関数を特殊なアセンブリ命令を使って最適化する][memcpy rep movsb]プルリクエストがopenされています。

[`mem` feature]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/Cargo.toml#L51-L52
[`memcpy` etc. implementations]: (https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/src/mem.rs#L12-L69)
[not optimized]: https://github.com/rust-lang/compiler-builtins/issues/339
[memcpy rep movsb]: https://github.com/rust-lang/compiler-builtins/pull/365

この変更をもって、私達のカーネルの実装はすべてのコンパイラに必要とされている関数の有効な実装を手に入れたので、私達のコードがもっと複雑になっても変わらずコンパイルできるでしょう。

#### 標準のターゲットをセットする

`cargo build`を呼び出すたびに`--target`パラメータを渡すのを避けるために、標準のターゲットを書き換えることができます。これをするには、以下を`.cargo/config.toml`の[cargo設定][cargo configuration]ファイルに付け加えます:

[cargo configuration]: https://doc.rust-lang.org/cargo/reference/config.html

```toml
# in .cargo/config.toml

[build]
target = "x86_64-blog_os.json"
```

これは、明示的に`--target`引数が渡されていないときは、`x86_64-blog_os.json`ターゲットを使うように`cargo`に命令します。つまり、私達はカーネルをシンプルな`cargo build`コマンドでビルドできるということです。cargoの設定のオプションについてより詳しく知るには、[公式のドキュメント][cargo configuration]を読んでください。

今や、シンプルな`cargo build`コマンドで、ベアメタルのターゲットに私達のカーネルをビルドできるようになりました。しかし、ブートローダーによって呼び出される私達の`_start`エントリポイントはまだ空っぽです。こいつから何かを画面に出力してみましょう。

### 画面に出力する
現在の段階で画面に文字を出力する最も簡単な方法は[VGAテキストバッファ][VGA text buffer]です。これは画面に出力されている内容を保持しているVGAハードウェアにマップされた特殊なメモリです。通常、これは25行からなり、それぞれの行は80文字セルからなります。それぞれの文字セルは、背景色と前景色付きのASCII文字を表示します。画面出力はこんなふうに見えます：

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

すべてのメモリへの書き込みの周りを、[<ruby>`unsafe`<rp> (</rp><rt>安全でない</rt><rp>) </rp></ruby>][`unsafe`]ブロックが囲んでいることに注意してください。この理由は、私達の作った生ポインタが合法であることをRustコンパイラが証明できないからです。生ポインタはどんな場所でも指しうるので、データの破損につながるかもしれません。これらの操作を`unsafe`ブロックに入れることで、私達はこれが合法だと確信しているとコンパイラに伝えているのです。ただし、`unsafe`ブロックはRustの安全性チェックを消すわけではなく、[5つのことが追加でできるようになる][five additional things]だけということに注意してください。

<div class="note">

**訳注:**  翻訳時点(2020-10-20)では、リンク先のThe Rust book日本語版には「追加でできるようになること」は4つしか書かれていません。

</div>

[`unsafe`]: https://doc.rust-jp.rs/book-ja/ch19-01-unsafe-rust.html
[five additional things]: https://doc.rust-jp.rs/book-ja/ch19-01-unsafe-rust.html#unsafeの強大な力superpower

強調しておきたいのですが、 **このような機能はRustでプログラミングするときに使いたいものではありません！** unsafeブロック内で生ポインタを扱うと非常にしくじりやすいです。たとえば、注意不足でバッファの終端のさらに奥に書き込みを行ってしまったりするかもしれません。

ですので、`unsafe`の使用はできるかぎり最小限にしたいです。これをするために、Rustでは安全な<ruby>abstraction<rp> (</rp><rt>抽象化されたもの</rt><rp>) </rp></ruby>を作ることができます。たとえば、VGAバッファ型を作り、この中にすべてのunsafeな操作を包みこみ（<ruby>カプセル化し<rp> (</rp><rt>encapsulate</rt><rp>) </rp></ruby>）、外側から誤ったことをするのを**不可能**にすることができるでしょう。こうすれば、`unsafe`の量を最小限にすることができ、[メモリ安全性][memory safety]を侵していないことを確信することができます。そのような安全なVGAバッファの抽象化を次の記事で作ります。

[memory safety]: https://ja.wikipedia.org/wiki/メモリ安全性

## カーネルを実行する

目に見えることを行ってくれる実行可能ファイルを手に入れたので、実行してみましょう。まず、コンパイルした私達のカーネルを、ブートローダーとリンクすることによってブータブルディスクイメージにする必要があります。すると、そのディスクイメージを、[QEMU]バーチャルマシン内や、USBメモリを使って実際のハードウェア上で実行できます。

### ブートイメージを作る

コンパイルされた私達のカーネルをブータブルディスクイメージに変えるには、ブートローダーとリンクする必要があります。[起動のプロセスの節][section about booting]で学んだように、ブートローダーはCPUを初期化しカーネルをロードする役割があります。

[section about booting]: #the-boot-process

自前のブートローダーを書くと、それだけで1つのプロジェクトになってしまうので、代わりに[`bootloader`]クレートを使いましょう。このクレートは、Cに依存せず、Rustとインラインアセンブリだけで基本的なBIOSブートローダーを実装しています。私達のカーネルを起動するためにこれを使うには、それへの依存関係を追記する必要があります：

[`bootloader`]: https://crates.io/crates/bootloader

```toml
# in Cargo.toml

[dependencies]
bootloader = "0.9.8"
```

bootloaderを依存として加えることだけでブータブルディスクイメージが実際に作れるわけではありません。問題は、私達のカーネルをコンパイル後にブートローダーにリンクしなければならないのに、cargoは[<ruby>ビルド後<rp> (</rp><rt>post-build</rt><rp>) </rp></ruby>にスクリプトを走らせること][post-build scripts]に対応していないのです。

[post-build scripts]: https://github.com/rust-lang/cargo/issues/545

この問題を解決するため、私達は`bootimage`というツールを作りました。これは、まずカーネルとブートローダーをコンパイルし、そしてこれらをリンクしてブータブルディスクイメージを作ります。このツールをインストールするには、以下のコマンドをターミナルで実行してください：

```
cargo install bootimage
```

`bootimage`を実行しブートローダをビルドするには、`llvm-tools-preview`というrustupコンポーネントをインストールする必要があります。これは`rustup component add llvm-tools-preview`と実行することでできます。

`bootimage`をインストールし、`llvm-tools-preview`を追加したら、以下のように実行することでブータブルディスクイメージを作れます：

```
> cargo bootimage
```

このツールが私達のカーネルを`cargo build`を使って再コンパイルしていることがわかります。そのため、あなたの行った変更を自動で検知してくれます。その後、bootloaderをビルドします。これには少し時間がかかるかもしれません。他の依存クレートと同じように、ビルドは一度しか行われず、その時キャッシュされるので、以降のビルドはもっと早くなります。最終的に、`bootimage`はbootloaderとあなたのカーネルを合体させ、ブータブルディスクイメージにします。

このコマンドを実行したら、`target/x86_64-blog_os/debug`ディレクトリ内に`bootimage-blog_os.bin`という名前のブータブルディスクイメージがあるはずです。これをバーチャルマシン内で起動してもいいですし、実際のハードウェア上で起動するためにUSBメモリにコピーしてもいいでしょう（ただし、これはCDイメージではありません。CDイメージは異なるフォーマットを持つので、これをCDに焼いてもうまくいきません）。

#### どういう仕組みなの？
`bootimage`ツールは、裏で以下のステップを行っています：

- 私達のカーネルを[ELF]ファイルにコンパイルする。
- 依存であるbootloaderをスタンドアロンの実行ファイルとしてコンパイルする。
- カーネルのELFファイルのバイト列をブートローダーにリンクする。

[ELF]: https://ja.wikipedia.org/wiki/Executable_and_Linkable_Format
[rust-osdev/bootloader]: https://github.com/rust-osdev/bootloader

起動時、ブートローダーは付け足されたELFファイルを読み、解釈します。次にプログラム部を<ruby>ページテーブル<rp> (</rp><rt>page table</rt><rp>) </rp></ruby>の<ruby>仮想アドレス<rp> (</rp><rt>virtual address</rt><rp>) </rp></ruby>にマップし、`.bss`部をゼロにし、スタックをセットアップします。最後に、エントリポイントの番地（私達の`_start`関数）を読み、そこに<ruby>飛び<rp> (</rp><rt>jump</rt><rp>) </rp></ruby>ます。

### QEMUで起動する

今や、ディスクイメージを仮想マシンで起動できます。[QEMU]でこれを起動するには、以下のコマンドを実行してください：

[QEMU]: https://www.qemu.org/

```
> qemu-system-x86_64 -drive format=raw,file=target/x86_64-blog_os/debug/bootimage-blog_os.bin
warning: TCG doesn't support requested feature: CPUID.01H:ECX.vmx [bit 5]
```

これにより、以下のような見た目の別のウィンドウが開きます：

![QEMU showing "Hello World!"](qemu.png)

私達の書いた"Hello World!"が画面に見えますね。

### 実際のマシン

USBメモリにこれを書き込んで実際のマシン上で起動することも可能です：

```
> dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

ただし、`sdX`はあなたのUSBメモリのデバイス名です。 **正しいデバイス名を選んでいるのかよく確認してください** 、そのデバイス上のすべてのデータが上書きされてしまいますので。

イメージをUSBメモリに書き込んだあとは、それから起動することによって実際のハードウェア上で走らせることができます。特殊なブートメニューを使ったり、BIOS設定で起動時の優先順位を変え、USBメモリから起動することを選択する必要があるでしょう。ただし、`bootloader`クレートはUEFIをサポートしていないので、UEFI機に対してはうまく行かないということに注意してください。

### `cargo run`を使う

QEMU内で私達のカーネルを走らせるのを簡単にするために、cargoの`runner`設定が使えます。

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```
`target.'cfg(target_os = "none")'`テーブルは、`"os"`フィールドが`"none"`であるようなすべてのターゲットに適用されます。私達の`x86_64-blog_os.json`ターゲットもその1つです。`runner`キーは`caargo run`のときに発動されるべきコマンドを指定しています。このコマンドは、ビルドが成功した後に、実行可能ファイルのパスを第一引数として実行されます。詳しくは、[cargoのドキュメント][cargo configuration]を読んでください。

`bootimage runner`コマンドは、`runner`で実行するために設計されています。このコマンドは、与えられた実行ファイルをプロジェクトの依存するbootloaderとリンクして、QEMUを立ち上げます。より詳しく知りたいときや、設定オプションについては[`bootimage`のReadme][Readme of `bootimage`]を読んでください。

[Readme of `bootimage`]: https://github.com/rust-osdev/bootimage

これで、`cargo run`を使ってカーネルをコンパイルしQEMU内で起動することができます。

## 次は？

次の記事では、VGAテキストバッファをより詳しく学び、その安全なインターフェースを書きます。`println`マクロが使えるようにもします。
