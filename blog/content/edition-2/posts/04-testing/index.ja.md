+++
title = "テスト"
weight = 4
path = "ja/testing"
date = 2019-04-27

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "dce5c9825bd4e7ea6c9530e999c9d58f80c585cc"
# GitHub usernames of the people that translated this post
translators = ["woodyZootopia", "JohnTitor"]
+++

この記事では、`no_std`な実行環境における<ruby>単体テスト<rp> (</rp><rt>unit test</rt><rp>) </rp></ruby>と<ruby>結合テスト<rp> (</rp><rt>integration test</rt><rp>) </rp></ruby>について学びます。Rustではカスタムテストフレームワークがサポートされているので、これを使ってカーネルの中でテスト関数を実行します。QEMUの外へとテストの結果を通知するため、QEMUと`bootimage`の様々な機能を使います。

<!-- more -->

このブログの内容は [GitHub] 上で公開・開発されています。何か問題や質問などがあれば issue をたててください (訳注: リンクは原文(英語)のものになります)。また[こちら][at the bottom]にコメントを残すこともできます。この記事の完全なソースコードは[`post-04` ブランチ][post branch]にあります。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-04

<!-- toc -->

## この記事を読む前に

この記事は、（古い版の）[単体テスト][_Unit Testing_]と[結合テスト][_Integration Tests_]の記事を置き換えるものです。この記事は、あなたが[最小のカーネル][_A Minimal Rust Kernel_]の記事を2019-04-27以降に読んだことを前提にしています。主に、あなたの`.cargo/config.toml`ファイルが[標準のターゲットを設定して][sets a default target]おり、[ランナー実行ファイルを定義している][defines a runner executable]ことが条件となります。

<div class="note">

**訳注:**  [最小のカーネル][_A Minimal Rust Kernel_]の記事が日本語に翻訳されたのはこの日より後なので、あなたがこのサイトを日本語で閲覧している場合は特に問題はありません。

</div>

[_Unit Testing_]: @/edition-2/posts/deprecated/04-unit-testing/index.md
[_Integration Tests_]: @/edition-2/posts/deprecated/05-integration-tests/index.md
[_A Minimal Rust Kernel_]: @/edition-2/posts/02-minimal-rust-kernel/index.ja.md
[sets a default target]: @/edition-2/posts/02-minimal-rust-kernel/index.ja.md#biao-zhun-notagetutowosetutosuru
[defines a runner executable]: @/edition-2/posts/02-minimal-rust-kernel/index.ja.md#cargo-runwoshi-u

## Rustにおけるテスト

Rustには[テストフレームワークが組み込まれて][built-in test framework]おり、特別な設定なしに単体テストを走らせることができます。何らかの結果をアサーションを使って確認する関数を作り、その関数のヘッダに`#[test]`属性をつけるだけです。その上で`cargo test`を実行すると、あなたのクレートのすべてのテスト関数を自動で見つけて実行してくれます。

[built-in test framework]: https://doc.rust-jp.rs/book-ja/ch11-00-testing.html

残念なことに、私達のカーネルのような`no_std`のアプリケーションにとっては、テストは少しややこしくなります。問題なのは、Rustのテストフレームワークは組み込みの[`test`]ライブラリを内部で使っており、これは標準ライブラリに依存しているということです。つまり、私達の`#[no_std]`のカーネルには標準のテストフレームワークは使えないのです。

[`test`]: https://doc.rust-lang.org/test/index.html

私達のプロジェクト内で`cargo test`を実行しようとすればそれがわかります：

```
> cargo test
   Compiling blog_os v0.1.0 (/…/blog_os)
error[E0463]: can't find crate for `test`
```
`test`クレートは標準ライブラリに依存しているので、私達のベアメタルのターゲットでは使えません。`test`クレートを`#[no_std]`環境に持ってくるということは[不可能ではない][utest]のですが、非常に不安定であり、また`panic`マクロの再定義といった<ruby>技巧<rp> (</rp><rt>ハック</rt><rp>) </rp></ruby>が必要になってしまいます。

[utest]: https://github.com/japaric/utest

### 独自のテストフレームワーク

ありがたいことに、Rustでは、不安定な[<ruby>`custom_test_frameworks`<rp> (</rp><rt>独自のテストフレームワーク</rt><rp>) </rp></ruby>][`custom_test_frameworks`]機能を使えば標準のテストフレームワークを置き換えることができます。この機能には外部ライブラリは必要なく、したがって`#[no_std]`環境でも動きます。これは、`#[test_case]`属性をつけられたすべての関数のリストを引数としてユーザの指定した実行関数を呼び出すことで働きます。こうすることで、（実行関数の）実装内容によってテストプロセスを最大限コントロールできるようにしているのです。

[`custom_test_frameworks`]: https://doc.rust-lang.org/unstable-book/language-features/custom-test-frameworks.html

標準のテストフレームワークと比べた欠点は、[`should_panic`テスト][`should_panic` tests]のような多くの高度な機能が利用できないということです。それらの機能が必要なら、自分で実装して提供してください、というわけです。これは私達にとって全く申し分のないことで、というのも、私達の非常に特殊な実行環境では、それらの高度な機能の標準の実装はいずれにせようまく働かないだろうからです。例えば、`#[should_panic]`属性はパニックを検知するためにスタックアンワインドを使いますが、これは私達のカーネルでは無効化しています。

[`should_panic` tests]: https://doc.rust-jp.rs/book-ja/ch11-01-writing-tests.html#should_panicでパニックを確認する

私達のカーネルのための独自テストフレームワークを実装するため、以下を`main.rs`に追記します：

```rust
// in src/main.rs

#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}
```

このランナーは短いデバッグメッセージを表示し、リスト内のそれぞれの関数を呼び出すだけです。引数の型である`&[&dyn Fn()]`は、[Fn()][_Fn()_]トレイトの[トレイトオブジェクト][_trait object_]参照の[スライス][_slice_]です。これは要するに、関数のように呼び出せる型への参照のリストです。この (test_runner) 関数はテストでない実行のときには意味がないので、`#[cfg(test)]`属性を使って、テスト時にのみこれがインクルードされるようにします。

[_slice_]: https://doc.rust-lang.org/std/primitive.slice.html
[_trait object_]: https://doc.rust-jp.rs/book-ja/ch17-02-trait-objects.html
[_Fn()_]: https://doc.rust-lang.org/std/ops/trait.Fn.html

`cargo test`を実行すると、今度は成功しているはずです（もし失敗したなら、下の補足を読んでください）。しかし、依然として、`test_runner`からのメッセージではなく "Hello World" が表示されてしまっています。この理由は、`_start`関数がまだエントリポイントとして使われているからです。「独自のテストフレームワーク」機能は`test_runner`を呼び出す`main`関数を生成するのですが、私達は`#[no_main]`属性を使っており、独自のエントリポイントを与えてしまっているため、このmain関数は無視されてしまうのです。

<div class = "warning">

**補足:** 現在、cargoには`cargo test`を実行すると、いくらかのケースにおいて "duplicate lang item" エラーになってしまうバグが存在します。これは、`Cargo.toml`内のプロファイルにおいて`panic = "abort"`を設定していたときに起こります。これを取り除けば`cargo test`はうまくいくはずです。これについて、より詳しく知りたい場合は[cargoのissue](https://github.com/rust-lang/cargo/issues/7359)を読んでください。

</div>

これを修正するために、まず生成される関数の名前を`reexport_test_harness_main`属性を使って`main`とは違うものに変える必要があります。そして、その<ruby>改名<rp> (</rp><rt>リネーム</rt><rp>) </rp></ruby>された関数を`_start`関数から呼び出せばよいです。

```rust
// in src/main.rs

#![reexport_test_harness_main = "test_main"]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}
```

テストフレームワークのエントリ関数の名前を`test_main`に設定し、私達の`_start`エントリポイントから呼び出しています。`test_main`関数は通常の実行時には生成されていないので、[条件付きコンパイル][conditional compilation]を用いて、テスト時にのみこの関数への呼び出しが追記されるようにしています。

`cargo test`を実行すると、 `test_runner`からの "Running 0 tests" というメッセージが画面に表示されます。これで、テスト関数を作り始める準備ができました：

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... "); // "些末なアサーション……"
    assert_eq!(1, 1);
    println!("[ok]");
}
```

`cargo test`を実行すると、以下の出力を得ます：

![QEMU printing "Hello World!", "Running 1 tests", and "trivial assertion... [ok]"](qemu-test-runner-output.png)

今、`test_runner`関数に渡される`test`のスライスは、`trivial_assertion`関数への参照を保持しています。`trivial assertion... [ok]`という画面の出力から、テストが呼び出され成功したことがわかります。

テストを実行したあとは、`test_runner`から`test_main`関数へとリターンし、さらに`_start`エントリポイント関数へとリターンします。エントリポイント関数がリターンすることは認められていないので、`_start`の最後では無限ループに入ります。しかし、`cargo test`にはすべてのテストを実行し終わった後に終了してほしいので、これは問題です。

## QEMUを終了する

今の所、`_start`関数の最後で無限ループがあるので、`cargo test`を実行するたびにQEMUを手動で終了しないといけません。ユーザによる入力などのないスクリプトでも`cargo test`を実行したいので、これは不都合です。これに対する綺麗な解決法はOSをシャットダウンする適切な方法を実装することでしょう。これは[APM]か[ACPI]というパワーマネジメント標準規格へのサポートを実装する必要があるので、残念なことに比較的複雑です。

[APM]: https://wiki.osdev.org/APM
[ACPI]: https://wiki.osdev.org/ACPI

しかし嬉しいことに、ある「脱出口」があるのです。QEMUは特殊な`isa-debug-exit`デバイスをサポートしており、これを使うとゲストシステムから簡単にQEMUを終了できます。これを有効化するためには、QEMUに`-device`引数を渡す必要があります。これは`Cargo.toml`に`package.metadata.bootimage.test-args`設定キーを追加することで行えます。

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
```

`bootimage runner`は、`test-args`をすべてのテスト実行可能ファイルの標準QEMUコマンドに追加します。通常の`cargo run`のとき、これらの引数は無視されます。

デバイス名 (`isa-debug-exit`) に加え、カーネルからそのデバイスにたどり着くための **I/Oポート** を指定する`iobase`と`iosize`という2つのパラメータを渡しています。

### I/Oポート

CPUと<ruby>周辺機器<rp> (</rp><rt>ペリフェラル</rt><rp>) </rp></ruby>が通信するやり方には、 **<ruby>memory-mapped<rp> (</rp><rt>メモリマップされた</rt><rp>) </rp></ruby> I/O** と **<ruby>port-mapped<rp> (</rp><rt>ポートマップされた</rt><rp>) </rp></ruby> I/O** の2つがあります。memory-mapped I/Oについては、すでに[VGAテキストバッファ][VGA text buffer]にメモリアドレス`0xb8000`を使ってアクセスしたときに使っています。このアドレスはRAMではなく、VGAデバイス上にあるメモリにマップされているのです。

[VGA text buffer]: @/edition-2/posts/03-vga-text-buffer/index.ja.md

一方、port-mapped I/Oは通信に別個のI/Oバスを使います。接続されたそれぞれの周辺機器は1つ以上のポート番号を持っています。それらのI/Oポートと通信するために、`in`と`out`という特別なCPU命令があり、これらはポート番号と1バイトのデータを受け取ります（`u16`や`u32`を送信できる、これらの亜種も存在します）。

`isa-debug-exit`はこのport-mapped I/Oを使います。`iobase`パラメータはどのポートにこのデバイスが繋がれているのか（`0xf4`はx86のI/Oバスにおいて[普通使われない][list of x86 I/O ports]ポートです）を、`iosize`はポートの大きさ（`0x04`は4バイトを意味します）を指定します。

[list of x86 I/O ports]: https://wiki.osdev.org/I/O_Ports#The_list

### 「終了デバイス」を使う

`isa-debug-exit`の機能は非常に単純です。値`value`が`iobase`により指定されたI/Oポートに書き込まれたら、QEMUは[終了ステータス][exit status]を`(value << 1) | 1`にして終了します。なので、このポートに`0`を書き込むと、QEMUは終了ステータス`(0 << 1) | 1 = 1`で、`1`を書き込むと終了ステータス`(1 << 1) | 1 = 3`で終了します。

[exit status]: https://ja.wikipedia.org/wiki/終了ステータス

`in`と`out`のアセンブリ命令を手動で呼び出す代わりに、[`x86_64`]クレートによって提供される<ruby>abstraction<rp> (</rp><rt>抽象化されたもの</rt><rp>) </rp></ruby>を使います。このクレートへの依存を追加するため、`Cargo.toml`の`dependencies`セクションにこれを追加しましょう：

[`x86_64`]: https://docs.rs/x86_64/0.12.1/x86_64/

```toml
# in Cargo.toml

[dependencies]
x86_64 = "0.12.1"
```

これで、このクレートによって提供される[`Port`]型を使って`exit_qemu`関数を作ることができます。

[`Port`]: https://docs.rs/x86_64/0.12.1/x86_64/instructions/port/struct.Port.html

```rust
// in src/main.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

この関数は新しい[`Port`]を`0xf4`（`isa-debug-exit`デバイスの`iobase`です）に作ります。そして、渡された終了コードをポートに書き込みます。`isa-device-exit`デバイスの`iosize`に4バイトを指定していたので、`u32`を使うことにします。I/Oポートへの書き込みは一般にあらゆる振る舞いを引き起こしうるので、これらの命令は両方unsafeです。

終了ステータスを指定するために、`QemuExitCode`enumを作ります。成功したら成功（`Success`）の終了コードで、そうでなければ失敗（`Failed`）の終了コードで終了しようというわけです。enumは`#[repr(u32)]`をつけることで、それぞれのヴァリアントが`u32`の整数として表されるようにしています。終了コード`0x10`を成功に、`0x11`を失敗に使います。終了コードの実際の値は、QEMUの標準の終了コードと被ってしまわない限りはなんでも構いません。例えば、成功の終了コードに`0`を使うと、変換後`(0 << 1) | 1 = 1`になってしまい、これはQEMUが実行に失敗したときの標準終了コードなのでよくありません。QEMUのエラーとテスト実行の成功が区別できなくなります。

というわけで、`test_runner`を更新して、すべてのテストが実行されたあとでQEMUを終了するようにできますね：

```rust
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    /// new
    exit_qemu(QemuExitCode::Success);
}
```

`cargo test`を実行すると、QEMUはテスト実行後即座に閉じるのがわかります。しかし、問題は、`Success`の終了コードを渡したのに、`cargo test`はテストが失敗したと解釈することです：

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.03s
     Running target/x86_64-blog_os/debug/deps/blog_os-5804fc7d2dd4c9be
Building bootloader
   Compiling bootloader v0.5.3 (/home/philipp/Documents/bootloader)
    Finished release [optimized + debuginfo] target(s) in 1.07s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-5804fc7d2dd4c9be.bin -device isa-debug-exit,iobase=0xf4,
    iosize=0x04`
error: test failed, to rerun pass '--bin blog_os'
```

問題は、`cargo test`が`0`でないすべてのエラーコードを失敗と解釈してしまうことです。

### 成功の終了コード

これを解決するために、`bootimage`は指定された終了コードを`0`へとマップする設定キー、`test-success-exit-code`を提供しています：

```toml
[package.metadata.bootimage]
test-args = […]
test-success-exit-code = 33         # (0x10 << 1) | 1
```

この設定を使うと、`bootimage`は私達の出した成功の終了コードを、終了コード0へとマップするので、`cargo test`は正しく成功を認識し、テストを失敗したと見做さなくなります。


これで私達のテストランナーは、自動でQEMUを閉じ、結果を報告するようになりました。しかし、QEMUの画面が非常に短い時間開くのは見えますが、短すぎて結果が読めません。QEMUが終了したあともテストの結果が見られるように、コンソールに出力できたら良さそうです。

## コンソールに出力する

テストの結果をコンソールで見るためには、カーネルからホストシステムにどうにかしてデータを送る必要があります。これを達成する方法は色々あり、例えばTCPネットワークインターフェースを通じてデータを送るというのが考えられます。しかし、ネットワークスタックを設定するのは非常に複雑なタスクなので、より簡単な解決策を取ることにしましょう。

### シリアルポート

データを送る簡単な方法とは、[シリアルポート][serial port]という、最近のコンピュータにはもはや見られない古いインターフェース標準を使うことです。これはプログラムするのが簡単で、QEMUはシリアルを通じて送られたデータをホストの標準出力やファイルにリダイレクトすることができます。

[serial port]: https://ja.wikipedia.org/wiki/シリアルポート

シリアルインターフェースを実装しているチップは[UART][UARTs]と呼ばれています。x86には[多くのUARTのモデルがありますが][lots of UART models]、幸運なことに、それらの違いは私達の必要としないような高度な機能だけです。今日よく見られるUARTはすべて[16550 UART]に互換性があるので、このモデルを私達のテストフレームワークに使いましょう。

[UARTs]: https://ja.wikipedia.org/wiki/UART
[lots of UART models]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter#UART_models
[16550 UART]: https://ja.wikipedia.org/wiki/16550_UART

[`uart_16550`]クレートを使ってUARTを初期化しデータをシリアルポートを使って送信しましょう。これを依存先として追加するため、`Cargo.toml`と`main.rs`を書き換えます：

[`uart_16550`]: https://docs.rs/uart_16550

```toml
# in Cargo.toml

[dependencies]
uart_16550 = "0.2.0"
```

`uart_16550`クレートにはUARTレジスタを表現する`SerialPort`構造体が含まれていますが、これのインスタンスは私達自身で作らなくてはいけません。そのため、以下の内容で新しい`serial`モジュールを作りましょう：

```rust
// in src/main.rs

mod serial;
```

```rust
// in src/serial.rs

use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}
```

[VGAテキストバッファ][vga lazy-static]のときのように、`lazy_static`とスピンロックを使って`static`なwriterインスタンスを作ります。`lazy_static`を使うことで、`init`メソッドが初回使用時にのみ呼び出されることを保証できます。

`isa-debug-exit`デバイスのときと同じように、UARTはport I/Oを使ってプログラムされています。UARTはより複雑で、様々なデバイスレジスタ群をプログラムするために複数のI/Oポートを使います。unsafeな`SerialPort::new`関数はUARTの最初のI/Oポートを引数とします。この引数から、すべての必要なポートのアドレスを計算することができます。ポートアドレス`0x3F8`を渡していますが、これは最初のシリアルインターフェースの標準のポート番号です。

[vga lazy-static]: @/edition-2/posts/03-vga-text-buffer/index.md#lazy-statics

シリアルポートを簡単に使えるようにするために、`serial_print!`と`serial_println!`マクロを追加します：

```rust
#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
}

/// シリアルインターフェースを通じてホストに出力する。
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// シリアルインターフェースを通じてホストに出力し、改行を末尾に追加する。
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
```

この実装は私達の`print`および`println`マクロとよく似ています。`SerialPort`型はすでに[`fmt::Write`]トレイトを実装しているので、自前の実装を提供する必要はありません。

[`fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

これで、テストコードにおいてVGAテキストバッファの代わりにシリアルインターフェースに出力することができます：

```rust
// in src/main.rs

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    […]
}

#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

`#[macro_export]`属性を使うことで、`serial_println`マクロはルート<ruby>名前空間<rp> (</rp><rt>namespace</rt><rp>) </rp></ruby>の直下に置かれるので、`use crate::serial::serial_println`とインポートするとうまくいかないということに注意してください。

### QEMUの引数

QEMUからのシリアル出力を見るために、出力を標準出力にリダイレクトしたいので、`-serial`引数を使う必要があります。

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio"
]
```

これで`cargo test`を実行すると、テスト出力がコンソールに直接出力されているのが見えるでしょう：

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [ok]
```

しかし、テストが失敗したときは、私達のパニックハンドラはまだ`println`を使っているので、出力がQEMUの中に出てしまいます。これをシミュレートするには、`trivial_assertion`テストの中のアサーションを`assert_eq!(0, 1)`に変えればよいです：

![QEMU printing "Hello World!" and "panicked at 'assertion failed: `(left == right)`
    left: `0`, right: `1`', src/main.rs:55:5](qemu-failed-test.png)

他のテスト出力がシリアルポートに出力されている一方、パニックメッセージはまだVGAバッファに出力されているのがわかります。このパニックメッセージは非常に役に立つので、コンソールでこのメッセージも見られたら非常に便利でしょう。

### パニック時のエラーメッセージを出力する

パニック時にQEMUをエラーメッセージとともに終了するためには、[条件付きコンパイル][conditional compilation]を使うことで、テスト時に異なるパニックハンドラを使うことができます：

[conditional compilation]: https://doc.rust-lang.org/1.30.0/book/first-edition/conditional-compilation.html

```rust
// 前からあるパニックハンドラ
#[cfg(not(test))] // 新しく追加した属性
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

// テストモードで使うパニックハンドラ
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}
```

テストパニックハンドラには`println`の代わりに`serial_println`を使い、そのあと失敗の終了コードでQEMUを終了します。コンパイラには、`exit_qemu`の呼び出しのあと`isa-debug-exit`デバイスがプログラムを終了させているということはわからないので、やはり最後に無限ループを入れないといけないことに注意してください。

これでQEMUはテストが失敗したときも終了し、コンソールに役に立つエラーメッセージを表示するようになります：

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [failed]

Error: panicked at 'assertion failed: `(left == right)`
  left: `0`,
 right: `1`', src/main.rs:65:5
```

これですべてのテスト出力がコンソールで見られるようになったので、一瞬出てくるQEMUウィンドウはもはや必要ありません。ですので、これを完全に見えなくしてしまいましょう。

### QEMUを隠す

すべてのテスト結果を`isa-debug-exit`デバイスとシリアルポートを使って通知できるので、QEMUのウィンドウはもはや必要ありません。これは、QEMUに`-display none`引数を渡すことで隠すことができます：

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio",
    "-display", "none"
]
```

これでQEMUは完全にバックグラウンドで実行するようになり、ウィンドウはもう開きません。これで、ジャマが減っただけでなく、私達のテストフレームワークがグラフィカルユーザーインターフェースのない環境――たとえばCIサービスや[SSH]接続――でも使えるようになりました。

[SSH]: https://ja.wikipedia.org/wiki/Secure_Shell

### タイムアウト

`cargo test`はテストランナーが終了するまで待つので、絶対に終了しないテストがあるとテストランナーを永遠にブロックしかねません。これは悲しいですが、普通<ruby>エンドレス<rp> (</rp><rt>終了しない</rt><rp>) </rp></ruby>ループを回避するのは簡単なので、実際は大きな問題ではありません。しかしながら、私達のケースでは、様々な状況でエンドレスループが発生しうるのです：

- ブートローダーが私達のカーネルを読み込むのに失敗し、これによりシステムが延々と再起動し続ける。
- BIOS/UEFIファームウェアがブートローダーの読み込みに失敗し、同様に延々と再起動し続ける。
- 私達の関数のどれかの最後で、CPUが`loop {}`文に入ってしまう（例えば、QEMU終了デバイスがうまく動かなかったなどの理由で）。
- CPU例外（今後説明します）がうまく捕捉されなかった場合などに、ハードウェアがシステムリセットを行う。

エンドレスループは非常に多くの状況で発生しうるので、`bootimage`はそれぞれのテスト実行ファイルに対し標準で5分のタイムアウトを設定しています。テストがこの時間内に終了しなかった場合は失敗したとみなされ、"Timed Out" エラーがコンソールに出力されます。この機能により、エンドレスループで詰まったテストが`cargo test`を永遠にブロックしてしまうことがないことが保証されます。

これを自分で試すこともできます。`trivial_assertion`テストに`loop {}`文を追加してください。`cargo test`を実行すると、5分後にテストがタイムアウトしたことが表示されるでしょう。タイムアウトまでの時間は`Cargo.toml`の`test-timeout`キーで[設定可能][bootimage config]です：

[bootimage config]: https://github.com/rust-osdev/bootimage#configuration

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-timeout = 300          # (単位は秒)
```

`trivial_assertion`テストがタイムアウトするのを待ちたくない場合は、上の値を一時的に下げても良いでしょう。

### 出力機能を自動で挿入する

現在、私達の`trivial_assertion`テストは、自分のステータス情報を`serial_print!`/`serial_println!`を使って出力する必要があります：

```rust
#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

私達の書くすべてのテストにこれらのprint文を手動で追加するのは煩わしいので、私達の`test_runner`を変更して、これらのメッセージを自動で出力するようにしましょう。そうするためには、`Testable`トレイトを作る必要があります：

```rust
// in src/main.rs

pub trait Testable {
    fn run(&self) -> ();
}
```

ここで、[`Fn()`トレイト][`Fn()` trait]を持つ型`T`すべてにこのトレイトを実装してやるのがミソです：

[`Fn()` trait]: https://doc.rust-lang.org/stable/core/ops/trait.Fn.html

```rust
// in src/main.rs

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}
```

`run`関数を実装するに当たり、まず[`any::type_name`]を使って関数の名前を出力します。この関数はコンパイラの中に直接実装されており、すべての型の文字列による説明を返すことができます。関数の型はその名前なので、今回の場合まさに私達のやりたいことができています。文字`\t`は[タブ文字][tab character]であり、メッセージ`[ok]`の前にちょっとしたアラインメント（幅を整えるための空白）をつけます。

[`any::type_name`]: https://doc.rust-lang.org/stable/core/any/fn.type_name.html
[tab character]: https://ja.wikipedia.org/wiki/タブキー#タブ文字

関数名を出力したあとは、テスト関数を`self()`を使って呼び出します。これは、`self`が`Fn()`トレイトを実装していることが要求されているからこそ可能です。テスト関数がリターンしたら、`[ok]`を出力してこの関数がパニックしなかったことを示します。

最後に、`test_runner`をこの`Testable`トレイトを使うように更新します：

```rust
// in src/main.rs

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run(); // ここを変更
    }
    exit_qemu(QemuExitCode::Success);
}
```

変更点は2つだけで、`tests`引数の型を`&[&dyn Fn()]`から`&[&dyn Testable]`に変えたことと、`test()`の変わりに`test.run()`を呼ぶようにしたことです。

また、`trivial_assertion`のprint文は今や自動で出力されるようになったので、これを取り除きましょう：

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
```

これで`cargo test`の出力は以下のようになるはずです：

```
Running 1 tests
blog_os::trivial_assertion...	[ok]
```

いま、関数名には関数までのフルパスが含まれていますが、これは異なるモジュールのテスト関数が同じ名前を持っているときに便利です。それ以外の点において出力は前と同じですが、もう手動でテストにprint文を付け加える必要はありません。

## VGAバッファをテストする

私達のテストフレームワークがうまく動くようになったので、私達のVGAバッファに関する実装のテストをいくつか作ってみましょう。まず、`println`がパニックすることなく成功することを確かめる非常に単純なテストを作ります：

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}
```

このテストは、適当な文字列をVGAバッファにただ出力するだけです。このテストがパニックすることなく終了したなら、`println`の呼び出しもまたパニックしなかったということです。

たくさんの行が出力され、行がスクリーンから押し出されたとしてもパニックが起きないことを確かめるために、もう一つテストを作ってみましょう：

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}
```

出力された行が本当に画面に映っているのかを確かめるテスト関数も作ることができます：

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_output() {
    let s = "Some test string that fits on a single line";
    println!("{}", s);
    for (i, c) in s.chars().enumerate() {
        let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
        assert_eq!(char::from(screen_char.ascii_character), c);
    }
}
```

この関数はテスト用文字列を定義し、`println`を使って出力し、静的な`WRITER`――VGAテキストバッファを表現しています――上の表示文字を<ruby>走査<rp> (</rp><rt>イテレート</rt><rp>) </rp></ruby>しています。`println`は最後に出力された行につづけて出力し、即座に改行するので、`BUFFER_HEIGHT - 2`行目にこの文字列は現れるはずです。

[`enumerate`]を使うことで、変数`i`によって反復の回数を数え、これを`c`に対応する画面上の文字を読み込むのに使っています。画面の文字の`ascii_character`を`c`と比較することで、文字列のそれぞれの文字がVGAテキストバッファに確実に現れていることを確かめることができます。

[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate

ご想像の通り、もっとたくさんテストを作っても良いです。例えば、非常に長い行を出力しても、うまく折り返され、パニックしないことをテストする関数や、改行・出力不可能な文字・非ユニコード文字などが適切に処理されることを確かめるような関数を作ることもできます。

ですが、この記事の残りでは、 **結合テスト** を作って、異なる<ruby>構成要素<rp> (</rp><rt>コンポーネント</rt><rp>) </rp></ruby>の相互作用をテストする方法を説明しましょう。

## 結合テスト
Rustにおける[結合テスト][integration tests]では、慣習としてプロジェクトのルートにおいた`tests`ディレクトリ (つまり`src`ディレクトリと同じ階層ですね) にテストプログラムを入れます。標準のテストフレームワークも、独自のテストフレームワークも、自動的にこのディレクトリにあるすべてのテストを実行します。

[integration tests]: https://doc.rust-jp.rs/book-ja/ch11-03-test-organization.html#結合テスト

すべての結合テストは、独自の実行可能ファイルを持っており、私達の`main.rs`とは完全に独立しています。つまり、それぞれのテストに独自のエントリポイント関数を定義しないといけないということです。どのような仕組みになっているのかを詳しく見るために、`basic_boot`という名前で試しに結合テストを作ってみましょう：

```rust
// in tests/basic_boot.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

#[no_mangle] // この関数の名前を変えない
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

fn test_runner(tests: &[&dyn Fn()]) {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
```

結合テストは独立した実行ファイルであるので、クレート属性（`no_std`、`no_main`、`test_runner`など）をすべてもう一度与えないといけません。また、新しいエントリポイント関数`_start`も作らないといけません。これはテストエントリポイント関数`test_main`を呼び出します。結合テストの実行可能ファイルは、テストモードでないときはビルドされないので、`cfg(test)`属性は必要ありません。

今のところ、`test_runner`関数の中身として、常にパニックする[`unimplemented`]マクロを代わりに入れており、そして`panic`ハンドラにはただの`loop`を入れています。本当は、`serial_println`マクロと`exit_qemu`関数を使って、これらの関数を`main.rs`と全く同じように実装したいです。しかし問題は、テストが私達の`main.rs`実行ファイルとは完全に別にビルドされているので、これらの関数にアクセスすることができないということです。

[`unimplemented`]: https://doc.rust-lang.org/core/macro.unimplemented.html

この段階で`cargo test`を実行したら、パニックハンドラによってエンドレスループに入ってしまうでしょう。QEMUを終了するキーボードショートカットである`Ctrl+c`を使わないといけません。

### ライブラリを作る

結合テストに必要な関数を利用できるようにするために、`main.rs`からライブラリを分離してやる必要があります。こうすると、他のクレートや結合テスト実行ファイルがこれをインクルードできるようになります。これをするために、新しい`src/lib.rs`ファイルを作りましょう：

```rust
// src/lib.rs

#![no_std]

```

`main.rs`と同じく、`lib.rs`は自動的にcargoに認識される特別なファイルです。ライブラリは別のコンパイル単位なので、`#![no_std]`属性を再び指定する必要があります。

`cargo test`がライブラリにも使えるようにするために、テストのための関数や属性を`main.rs`から`lib.rs`へと移す必要もあります。

```rust
// in src/lib.rs

#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

/// `cargo test`のときのエントリポイント
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
```

`test_runner`を（`main.rs`の）実行可能ファイルと結合テストの両方から利用可能にするために、`cfg(test)`属性をこれに適用せず、また、publicにします。パニックハンドラの実装もpublicな`test_panic_handler`関数へと分離することで、実行可能ファイルからも使えるようにしています。

`lib.rs`は`main.rs`とは独立にコンパイルされるので、ライブラリがテストモードでコンパイルされるときは`_start`エントリポイントとパニックハンドラを追加する必要があります。このような場合、[`cfg_attr`]クレート属性を使うことで、`no_main`属性を条件付きで有効化することができます。

[`cfg_attr`]: https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute

`QemuExitCode`enumと`exit_qemu`関数も移動し、publicにします：

```rust
// in src/lib.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

これで、実行ファイルも結合テストもこれらの関数をライブラリからインポートでき、自前の実装を定義する必要はありません。`println`と`serial_println`も利用可能にするために、モジュールの宣言も移動させましょう：

```rust
// in src/lib.rs

pub mod serial;
pub mod vga_buffer;
```

モジュールをpublicにすることで、ライブラリの外からも使えるようにしています。`println`と`serial_println`マクロは、これらのモジュールの`_print`関数を使っているため、これらのマクロを使うためにも、この変更は必要です。

では、`main.rs`をこのライブラリを使うように更新しましょう：

```rust
// src/main.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use blog_os::println;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}

/// この関数はパニック時に呼ばれる。
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

ライブラリは通常の外部クレートと同じように使うことができます。名前は、私達のクレート名――今回なら`blog_os`――になります。上のコードでは、`blog_os::test_runner`関数を`test_runner`属性で、`blog_os::test_panic_handler`関数を`cfg(test)`のパニックハンドラで使っています。また、`println`マクロをインポートすることで、`_start`と`panic`関数で使えるようにもしています。

この時点で、`cargo run`と`cargo test`は再びうまく実行できるようになっているはずです。もちろん、`cargo test`は依然エンドレスループするはずですが（`ctrl+c`で終了できます）。結合テストに必要な関数を使ってこれを修正しましょう。

### 結合テストを完成させる

`src/main.rs`と同じように、`tests/basic_boot.rs`実行ファイルは新しいライブラリから型をインポートできます。これで、テストを完成させるのに足りない要素をインポートすることができます。

```rust
// in tests/basic_boot.rs

#![test_runner(blog_os::test_runner)]

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

テストランナーを再実装することはせず、ライブラリの`test_runner`関数を使います。`panic`ハンドラとしては、`main.rs`でやったように`blog_os::test_panic_handler`関数を呼びます。

これで、`cargo test`は再び通常通り終了するはずです。実行すると、`lib.rs`、`main.rs`、そして`basic_boot.rs`を順にそれぞれビルドし、テストを実行するのが見えるはずです。`main.rs`と`basic_boot`結合テストに関しては、これらには`#[test_case]`のつけられた関数はないため、"Running 0 tests"と報告されるはずです。

これで、`basic_boot.rs`にテストを追加していくことができます。例えば、`println`がパニックすることなくうまく行くことを、VGAバッファのときのようにテストすることができます：

```rust
// in tests/basic_boot.rs

use blog_os::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}
```

`cargo test`を実行すると、テスト関数を見つけ出して実行しているのがわかるでしょう。

このテストは、VGAバッファのテストとほとんど同じであるため、今のところあまり意味がないように思われるかもしれません。しかし、将来的に`main.rs`の`_start`関数と`lib.rs`はどんどん大きくなり、`test_main`関数を実行する前に様々な初期化ルーチンを呼ぶようになるかもしれないので、これらの2つのテストは全然違う環境で実行されるようになるかもしれないのです。

`println`を`basic_boot`環境で（`_start`で初期化ルーチンを一切呼ぶことなく）テストすることにより、起動の直後に`println`が使えることが保証されます。私達は、例えばパニックメッセージの出力などを`println`に依存しているので、これは重要です。

### 今後のテスト

結合テストの魅力は、これらが完全に独立した実行ファイルとして扱われることです。これにより、実行環境を完全にコントロールすることができるので、コードがCPUやハードウェアデバイスと正しく相互作用していることをテストすることができるのです。

`basic_boot`テストは結合テストの非常に簡単な例でした。今後、私達のカーネルは機能がより豊富になり、そして様々な方法でハードウェアと相互作用するようになります。結合テストを追加することにより、それらの相互作用が期待通り動く（また、期待通り動きつづけている）ことを確かめることができるのです。今後追加できるテストの例としては、以下があります：

- **CPU<ruby>例外<rp> (</rp><rt>exception</rt><rp>) </rp></ruby>**: プログラムが不正な操作（例えばゼロで割るなど）を行った場合、CPUは例外を投げます（訳注：例外を発することを、英語でthrow an exceptionというのにちなんで、慣例的に「投げる」と表現します）。カーネルはそのような例外に対するハンドラ関数を登録しておくことができます。結合テストで、CPU例外が起こったときに、例外ハンドラが呼ばれていることや、例外が解決可能だった場合に実行が継続することを確かめることができるでしょう。
- **ページテーブル**: ページテーブルは、どのメモリ領域が有効でアクセスできるかを定義しています。例えばプログラムを立ち上げるとき、このページテーブルを変更することで、新しいメモリ領域を割り当てることが可能です。結合テストで、ページテーブルに`_start`関数内で何らかの変更を施して、その変更が期待通りの効果を起こしているかを`#[test_case]`関数で確かめることができるでしょう。
- **ユーザー<ruby>空間<rp> (</rp><rt>スペース</rt><rp>) </rp></ruby>プログラム**: ユーザー空間プログラムは、システムの<ruby>資源<rp> (</rp><rt>リソース</rt><rp>) </rp></ruby>に限られたアクセスしか持たないプログラムのことです。これらは例えば、カーネルのデータ構造や、他のプログラムのメモリにアクセスすることはできません。結合テストで、禁止された操作を実行するようなユーザー空間プログラムを起動し、カーネルがそれらをすべて防ぐことを確かめることができるでしょう。

ご想像のとおり、もっと多くのテストが可能です。このようなテストを追加することで、カーネルに新しい機能を追加したときや、コードをリファクタリングしたときに、これらを壊してしまっていないことを保証できます。これは、私達のカーネルがより大きく、より複雑になったときに特に重要になります。

### パニックしなければならないテスト

標準ライブラリのテストフレームワークは、[`#[should_panic]`属性][should_panic]をサポートしています。これを使うと、失敗しなければならないテストを作ることができます。これは、例えば、関数が無効な引数を渡されたときに失敗することを確かめる場合などに便利です。残念なことに、この機能は標準ライブラリのサポートを必要とするため、`#[no_std]`クレートではこの属性はサポートされていません。

[should_panic]: https://doc.rust-jp.rs/rust-by-example-ja/testing/unit_testing.html#testing-panics

`#[should_panic]`属性は使えませんが、パニックハンドラから成功のエラーコードで終了するような結合テストを作れば、似たような動きをさせることはできます。そのようなテストを`should_panic`という名前で作ってみましょう：

```rust
// in tests/should_panic.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use blog_os::{QemuExitCode, exit_qemu, serial_println};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

これは`_start`関数や、独自テストランナー属性などをまだ定義していないので未完成です。足りない部分を追加しましょう：

```rust
// in tests/should_panic.rs

#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test();
        serial_println!("[test did not panic]");
        exit_qemu(QemuExitCode::Failed);
    }
    exit_qemu(QemuExitCode::Success);
}
```

このテストは、`lib.rs`の`test_runner`を使い回さず、自前の、テストがパニックせずリターンしたときに失敗の終了コードを出すような`test_runner`関数を定義しています（私達はテストにパニックしてほしいわけですから）。もしテスト関数が一つも定義されていなければ、このランナーは成功のエラーコードで終了します。ランナーは一つテストを実行したら必ず終了するので、1つ以上の`#[test_case]`関数を定義しても意味はありません。

では、失敗するはずのテストを追加してみましょう：

```rust
// in tests/should_panic.rs

use blog_os::serial_print;

#[test_case]
fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}
```

このテストは`assert_eq`を使って`0`と`1`が等しいことをアサートしています。これはもちろん失敗するので、私達のテストは望み通りパニックします。ここで、`Testable`トレイトは使っていないので、関数名は`serial_print!`を使って自分で出力しないといけないことに注意してください。

`cargo test --test should_panic`を使ってテストすると、テストが期待通りパニックし、成功したことがわかるでしょう。アサーションをコメントアウトしテストをもう一度実行すると、"test did not panic"というメッセージとともに、テストが確かに失敗することがわかります。

この方法の無視できない欠点は、テスト関数を一つしか使えないことです。`#[test_case]`関数が複数ある場合、パニックハンドラが呼び出された後で（プログラムの）実行を続けることはできないので、最初の関数のみが実行されます。この問題を解決するいい方法を私は知らないので、もしなにかアイデアがあったら教えてください！

### <ruby>ハーネス<rp> (</rp><rt>harness</rt><rp>) </rp></ruby>のないテスト

<div class="note">

**訳注:** ハーネスとは、もともとは馬具の一種を意味する言葉です。転じて「制御する道具」一般を指し、また[テストハーネス](https://en.wikipedia.org/wiki/Test_harness)というと（`test_runner`のように）複数のテストケースを処理し、その振る舞い・出力などを適切に処理・整形してくれるプログラムのことを指します。

</div>

（私達の`should_panic`テストのように）一つしかテスト関数を持たない結合テストでは、テストランナーは必ずしも必要というわけではありません。このような場合、テストランナーは完全に無効化してしまって、`_start`関数からテストを直接実行することができます。

このためには、`Cargo.toml`でこのテストの`harness`フラグを無効化することがカギとなります。これは、結合テストにテストランナーが使われるかを定義しています。これが`false`に設定されると、標準のテストランナーと独自のテストランナーの両方が無効化され、通常の実行ファイルのように扱われるようになります。

`should_panic`テストの`harness`フラグを無効化してみましょう：

```toml
# in Cargo.toml

[[test]]
name = "should_panic"
harness = false
```

これで、テストランナーに関係するコードを取り除いて、`should_panic`テストを大幅に簡略化することができます。結果として以下のようになります：

```rust
// in tests/should_panic.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    loop{}
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

`should_fail`関数を`_start`関数から直接呼び出して、もしリターンしたら失敗の終了コードで終了するようにしました。今`cargo test --test should_panic`を実行しても、以前と全く同じように振る舞います。

`should_panic`なテストを作るとき以外にも`harness`属性は有用なことがあります。例えば、それぞれのテスト関数が副作用を持っており、指定された順番で実行されないといけないときなどです。

## まとめ

テストは、ある要素が望み通りの振る舞いをしていることを保証するのにとても便利なテクニックです。バグが存在しないことを証明することはできないとはいえ、バグを発見したり、特にリグレッションを防ぐのに便利な方法であることは間違いありません。

この記事では、私達のRust製カーネルでテストフレームワークを組み立てる方法を説明しました。Rustの<ruby>独自<rp> (</rp><rt>カスタム</rt><rp>) </rp></ruby>テストフレームワーク機能を使って、私達のベアメタル環境における、シンプルな`#[test_case]`属性のサポートを実装しました。私達のテストランナーは、QEMUの`isa-debug-exit`デバイスを使うことで、QEMUをテスト実行後に終了し、テストステータスを報告することができます。エラーメッセージを、VGAバッファの代わりにコンソールに出力するために、シリアルポートの単純なドライバを作りました。

`println`マクロのテストをいくつか作った後、記事の後半では結合テストについて見ました。結合テストは`tests`ディレクトリに置かれ、完全に独立した実行ファイルとして扱われることを学びました。結合テストから`exit_qemu`関数と`serial_println`マクロにアクセスできるようにするために、コードのほとんどをライブラリに移し、すべての実行ファイルと結合テストがインポートできるようにしました。結合テストはそれぞれ独自の環境で実行されるため、ハードウェアとの相互作用や、パニックするべきテストを作るといったことが可能になります。

QEMU内で現実に近い環境で実行できるテストフレームワークを手に入れました。今後の記事でより多くのテストを作っていくことで、カーネルがより複雑になってもメンテナンスし続けられるでしょう。

## 次は？

次の記事では、**CPU例外**を見ていきます。この例外というのは、CPUによってなにか「不法行為」――例えば、ゼロ除算やマップされていないメモリページへのアクセス（いわゆる「ページフォルト」）――が行われたときに投げられます。これらの例外を捕捉してテストできるようにしておくことは、将来エラーをデバッグするときに非常に重要です。例外の処理はまた、キーボードをサポートするのに必要になる、ハードウェア割り込みの処理に非常に似てもいます。
