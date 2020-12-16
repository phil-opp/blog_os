+++
title = "VGAテキストモード"
weight = 3
path = "ja/vga-text-mode"
date  = 2018-02-26

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "bd6fbcb1c36705b2c474d7fcee387bfea1210851"
# GitHub usernames of the people that translated this post
translators = ["woodyZootopia"]
+++

[VGAテキストモード][VGA text mode]は画面にテキストを出力するシンプルな方法です。この記事では、すべてのunsafeな要素を別のモジュールにカプセル化することで、それを安全かつシンプルに扱えるようにするインターフェースを作ります。また、Rustの[フォーマッティングマクロ][formatting macros]のサポートも実装します。

[VGA text mode]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode
[formatting macros]: https://doc.rust-lang.org/std/fmt/#related-macros

<!-- more -->

このブログの内容は [GitHub] 上で公開・開発されています。何か問題や質問などがあれば issue をたててください (訳注: リンクは原文(英語)のものになります)。また[こちら][at the bottom]にコメントを残すこともできます。この記事の完全なソースコードは[`post-03` ブランチ][post branch]にあります。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-03

<!-- toc -->

## VGAテキストバッファ
VGAテキストモードにおいて、文字を画面に出力するには、VGAハードウェアのテキストバッファにそれを書き込まないといけません。VGAテキストバッファは、普通25行と80列からなる2次元配列で、画面に直接書き出されます。それぞれの配列の要素は画面上の一つの文字を以下の形式で表現しています：

ビット | 値
------ | ----------------
0-7    | ASCII コードポイント
8-11   | フォアグラウンド（前景）色
12-14  | バックグラウンド（背景）色
15     | 点滅

最初の1バイトは、出力されるべき文字を[ASCIIエンコーディング][ASCII encoding]で表します。正確に言うと、完全にASCIIではなく、[コードページ437][_code page 437_]という、いくつか文字が追加され、軽微な修正のなされたものです。簡単のため、この記事ではASCII文字と呼ぶことにします。

[ASCII encoding]: https://ja.wikipedia.org/wiki/ASCII
[_code page 437_]: https://ja.wikipedia.org/wiki/コードページ437

2つ目のバイトはその文字がどのように出力されるのかを定義します。最初の4ビットが前景色（訳注：文字自体の色）を、次の3ビットが背景色を、最後のビットがその文字が点滅するのかを決めます。以下の色を使うことができます：

数字   | 色          | 数字 + Bright Bit   | <ruby>Bright<rp> (</rp><rt>明るい</rt><rp>) </rp></ruby> 色
------ | ----------  | ------------------- | -------------
0x0    | 黒          | 0x8                 | 暗いグレー
0x1    | 青          | 0x9                 | 明るい青
0x2    | 緑          | 0xa                 | 明るい緑
0x3    | シアン      | 0xb                 | 明るいシアン
0x4    | 赤          | 0xc                 | 明るい赤
0x5    | マゼンタ    | 0xd                 | ピンク
0x6    | 茶色        | 0xe                 | 黄色
0x7    | 明るいグレー| 0xf                 | 白

4ビット目は **bright bit** で、これは（1になっているとき）たとえば青を明るい青に変えます。背景色については、このビットは点滅ビットとして再利用されています。

VGAテキストバッファはアドレス`0xb8000`に[<ruby>memory-mapped<rp> (</rp><rt>メモリマップされた</rt><rp>) </rp></ruby> I/O][memory-mapped I/O]を通じてアクセスできます。これは、このアドレスへの読み書きをしても、RAMではなく直接VGAハードウェアのテキストバッファにアクセスするということを意味します。つまり、このアドレスに対する通常のメモリ操作を通じて、テキストバッファを読み書きできるのです。

[memory-mapped I/O]: https://ja.wikipedia.org/wiki/メモリマップドI/O

メモリマップされたハードウェアは通常のRAM操作すべてをサポートしてはいないかもしれないということに注意してください。たとえば、デバイスはバイトずつの読み取りしかサポートしておらず、`u64`が読まれるとゴミデータを返すかもしれません。ありがたいことに、テキストバッファを特別なやり方で取り扱う必要がないよう、テキストバッファは[通常の読み書きをサポートしています][supports normal reads and writes]。

[supports normal reads and writes]: https://web.stanford.edu/class/cs140/projects/pintos/specs/freevga/vga/vgamem.htm#manip

## Rustのモジュール
VGAバッファが動く仕組みを学んだので、さっそく画面出力を扱うRustのモジュールを作っていきます。

```rust
// in src/main.rs
mod vga_buffer;
```

このモジュールの中身のために、新しい`src/vga_buffer.rs`というファイルを作ります。このファイル以下のコードは、（そうならないよう指定されない限り）すべてこの新しいモジュールの中に入ります。

### 色
まず、様々な色をenumを使って表しましょう：

```rust
// in src/vga_buffer.rs

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}
```
ここでは、それぞれの色の数を指定するのに[C言語ライクなenum][C-like enum]を使っています。`repr(u8)`属性のため、それぞれのenumのヴァリアントは`u8`として格納されています。実際には4ビットでも十分なのですが、Rustには`u4`型はありませんので。

[C-like enum]: https://doc.rust-jp.rs/rust-by-example-ja/custom_types/enum/c_like.html

通常、コンパイラは使われていないヴァリアントそれぞれに対して警告を発します。`#[allow(dead_code)]`属性を使うことで`Color` enumに対するそれらの警告を消すことができます。

[`Copy`]、[`Clone`]、[`Debug`]、[`PartialEq`]、および [`Eq`]を[derive][deriving]することによって、この型の[コピーセマンティクス][copy semantics]を有効化し、この型を出力することと比較することを可能にします。

[deriving]: https://doc.rust-jp.rs/rust-by-example-ja/trait/derive.html
[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[`Clone`]: https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html
[`Debug`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html
[`PartialEq`]: https://doc.rust-lang.org/nightly/core/cmp/trait.PartialEq.html
[`Eq`]: https://doc.rust-lang.org/nightly/core/cmp/trait.Eq.html
[copy semantics]: https://doc.rust-jp.rs/book-ja/appendix-03-derivable-traits.html#値を複製するcloneとcopy


前景と背景の色を指定する完全なカラーコードを表現するために、`u8`の上に[ニュータイプ][newtype]を作ります。

[newtype]: https://doc.rust-jp.rs/rust-by-example-ja/generics/new_types.html

```rust
// in src/vga_buffer.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}
```
`ColorCode`構造体は前景色と背景色を持つので、完全なカラーコードを持ちます。前と同じように、`Copy`と`Debug`トレイトをこれにderiveします。`ColorCode`が`u8`と全く同じデータ構造を持つようにするために、[`repr(transparent)`]属性（訳注：翻訳当時、リンク先未訳）を使います。

[`repr(transparent)`]: https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent

### テキストバッファ
次に、画面上の文字とテキストバッファをそれぞれ表す構造体を追加していきます。

```rust
// in src/vga_buffer.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```
Rustにおいて、デフォルトの構造体におけるフィールドの並べ方は未定義なので、[`repr(C)`]属性が必要になります。これは、構造体のフィールドがCの構造体と全く同じように並べられることを保証してくれるので、フィールドの並べ方が正しいと保証してくれるのです。`Buffer`構造体については、[`repr(transparent)`]をもう一度使うことで、その唯一のフィールドと同じメモリレイアウトを持つようにしています。

[`repr(C)`]: https://doc.rust-jp.rs/rust-nomicon-ja/other-reprs.html#reprc

実際に画面に書き出すため、writer型を作ります。

```rust
// in src/vga_buffer.rs

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}
```
writerは常に最後の行に書き、行が一杯になったとき（もしくは`\n`を受け取った時）は1行上に送ります。`column_position`フィールドは、最後の行における現在の位置を持ちます。現在の前景および背景色は`color_code`によって指定されており、VGAバッファへの参照は`buffer`に格納されています。ここで、コンパイラにどのくらいの間参照が有効であるのかを教えるために[明示的なライフタイム][explicit lifetime]が必要になることに注意してください。[`'static`]ライフタイムは、その参照がプログラムの実行中ずっと有効であることを指定しています（これはVGAバッファについて正しいです）。

[explicit lifetime]: https://doc.rust-jp.rs/book-ja/ch10-03-lifetime-syntax.html#ライフタイム注釈記法
[`'static`]: https://doc.rust-jp.rs/book-ja/ch10-03-lifetime-syntax.html#静的ライフタイム

### 出力する
では`Writer`を使ってバッファの文字を変更しましょう。まず一つのASCII文字を書くメソッドを作ります：

```rust
// in src/vga_buffer.rs

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code,
                };
                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {/* TODO */}
}
```
（引数の）バイトが[改行コード][newline]のバイトすなわち`\n`の場合は、writerは何も出力しません。代わりに、あとで実装する`new_line`メソッドを呼びます。他のバイトは、2つ目のマッチケースにおいて画面に出力されます。

[newline]: https://ja.wikipedia.org/wiki/%E6%94%B9%E8%A1%8C%E3%82%B3%E3%83%BC%E3%83%89

バイトを出力する時、writerは現在の行がいっぱいかをチェックします。その場合、行を折り返すために先に`new_line`の呼び出しが必要です。その後で現在の場所のバッファに新しい`ScreenChar`を書き込みます。最後に、現在の<ruby>列の位置<rp> (</rp><rt>column position</rt><rp>) </rp></ruby>を進めます。

文字列全体を出力するには、バイト列に変換しひとつひとつ出力すればよいです：

```rust
// in src/vga_buffer.rs

impl Writer {
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // 出力可能なASCIIバイトか、改行コード
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // 出力可能なASCIIバイトではない
                _ => self.write_byte(0xfe),
            }

        }
    }
}
```

VGAテキストバッファはASCIIおよび[コードページ437][code page 437]にある追加のバイトのみをサポートしています。Rustの文字列はデフォルトでは[UTF-8]なのでVGAテキストバッファにはサポートされていないバイトを含んでいる可能性があります。matchを使って出力可能なASCIIバイト（改行コードか、空白文字から`~`文字の間のすべての文字）と出力不可能なバイトを分けています。出力不可能なバイトについては、文字`■`を出力します（これはVGAハードウェアにおいて16進コード`0xfe`を持っています）。

[code page 437]: https://ja.wikipedia.org/wiki/コードページ437
[UTF-8]: https://www.fileformat.info/info/unicode/utf8.htm

#### やってみよう！
適当な文字を画面に書き出すために、一時的に使う関数を作ってみましょう。

```rust
// in src/vga_buffer.rs

pub fn print_something() {
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };

    writer.write_byte(b'H');
    writer.write_string("ello ");
    writer.write_string("Wörld!");
}
```
この関数はまず、VGAバッファの`0xb8000`を指す新しいwriterを作ります。このための構文はやや奇妙に思われるかもしれません：まず、整数`0xb8000`を可変な[生ポインタ][raw pointer]にキャストします。次にこれを（`*`を使って）参照外しすることで可変な参照に変え、即座にそれを（`&mut`を使って）再び借用します。コンパイラはこの生ポインタが有効であることを保証できないので、この変換には[`unsafe`ブロック][`unsafe` block]が必要となります。

[raw pointer]: https://doc.rust-jp.rs/book-ja/ch19-01-unsafe-rust.html#生ポインタを参照外しする
[`unsafe` block]: https://doc.rust-jp.rs/book-ja/ch19-01-unsafe-rust.html

つぎに、この関数はそれにバイト`b'H'`を書きます。`b`というプレフィックスは、ASCII文字を表す[バイトリテラル][byte literal]を作ります。文字列`"ello "`と`"Wörld!"`を書くことで、私達の`write_string`関数と出力不可能な文字の処理をテストできます。出力を見るためには、`print_something`関数を`_start`関数から呼び出さなければなりません：

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    vga_buffer::print_something();

    loop {}
}
```

ここで、私達のプロジェクトを実行したら、`Hello W■■rld!`が画面の左 **下** に黄色で出力されるはずです。

[byte literal]: https://doc.rust-lang.org/reference/tokens.html#byte-literals

![QEMU output with a yellow `Hello W■■rld!` in the lower left corner](vga-hello.png)

`ö`は2つの`■`という文字として出力されていることに注目してください。これは、`ö`は[UTF-8]において2つのバイトで表され、それらはどちらも出力可能なASCIIの範囲に収まっていないためです。実は、これはUTF-8の基本的な特性です：マルチバイト値のそれぞれのバイトは、絶対に有効なASCIIではないのです。

### Volatile
メッセージが正しく出力されるのを確認できました。しかし、より強力に最適化をする将来のRustコンパイラでは、これはうまく行かないかもしれません。

問題なのは、私達は`Buffer`に書き込むけれども、それから読み取ることはないということです。コンパイラは私達が実際には（通常のRAMの代わりに）VGAバッファメモリにアクセスしていることを知らないので、文字が画面に出力されるという副作用も全く知りません。なので、それらの書き込みは不要で省略可能と判断するかもしれません。この誤った最適化を回避するためには、それらの書き込みを **[volatile]** であると指定する必要があります。これは、この書き込みには副作用があり、最適化により取り除かれるべきではないとコンパイラに命令します。

[volatile]: https://en.wikipedia.org/wiki/Volatile_(computer_programming)

VGAバッファへのvolatileな書き込みをするために、[volatile][volatile crate]ライブラリを使います。この **クレート**（Rustではパッケージのことをこう呼びます）は、`read`と`write`というメソッドを持つ`Volatile`というラッパー型を提供します。これらのメソッドは、内部的にcoreライブラリの[read_volatile]と[write_volatile]関数を使い、読み込み・書き込みが最適化により取り除かれないことを保証します。

[volatile crate]: https://docs.rs/volatile
[read_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.read_volatile.html
[write_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.write_volatile.html

`Cargo.toml`の`dependencies`セクションに`volatile`クレートを追加することで、このクレートへの依存関係を追加できます。

```toml
# in Cargo.toml

[dependencies]
volatile = "0.2.6"
```

`0.2.6`は[セマンティック][semantic]バージョン番号です。詳しくは、cargoドキュメントの[依存関係の指定][Specifying Dependencies]を見てください。

[semantic]: https://semver.org/lang/ja/
[Specifying Dependencies]: https://doc.crates.io/specifying-dependencies.html

これを使って、VGAバッファへの書き込みをvolatileにしてみましょう。`Buffer`型を以下のように変更します：

```rust
// in src/vga_buffer.rs

use volatile::Volatile;

struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```
`ScreenChar`の代わりに、`Volatile<ScreenChar>`を使っています（`Volatile`型は[ジェネリック][generic]であり（ほぼ）すべての型をラップできます）。これにより、間違って「普通の」書き込みをこれに対して行わないようにできます。これからは、代わりに`write`メソッドを使わなければいけません。

[generic]: https://doc.rust-lang.org/book/ch10-01-syntax.html

つまり、`Writer::write_byte`メソッドを更新しなければいけません：

```rust
// in src/vga_buffer.rs

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                ...

                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                ...
            }
        }
    }
    ...
}
```

`=`を使った通常の代入の代わりに`write`メソッドを使っています。これにより、コンパイラがこの書き込みを最適化して取り除いてしまわないことが保証されます。

### フォーマットマクロ
Rustの<ruby>フォーマットマクロ<rp> (</rp><rt>formatting macro</rt><rp>) </rp></ruby>もサポートすると良さそうです。そうすると、整数や浮動小数点数といった様々な型を簡単に出力できます。それらをサポートするためには、[`core::fmt::Write`]トレイトを実装する必要があります。このトレイトに必要なメソッドは`write_str`だけです。これは私達の`write_string`によく似ており、戻り値の型が`fmt::Result`であるだけです：

[`core::fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

```rust
// in src/vga_buffer.rs

use core::fmt;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
```
`Ok(())`は、`()`型を持つ`Ok`、というだけです。

Rustの組み込みの`write!`/`writeln!`フォーマットマクロが使えるようになりました。

```rust
// in src/vga_buffer.rs

pub fn print_something() {
    use core::fmt::Write;
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };

    writer.write_byte(b'H');
    writer.write_string("ello! ");
    write!(writer, "The numbers are {} and {}", 42, 1.0/3.0).unwrap();
}
```

このようにすると、画面の下端に`Hello! The numbers are 42 and 0.3333333333333333`が見えるはずです。`write!`の呼び出しは`Result`を返し、これは放置されると警告を出すので、[`unwrap`]関数（エラーの際パニックします）をこれに呼び出しています。VGAバッファへの書き込みは絶対に失敗しないので、この場合これは問題ではありません。

[`unwrap`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.unwrap

### 改行
現在、改行や、行に収まらない文字は無視しています。その代わりに、すべての文字を一行上に持っていき（一番上の行は消去されます）、前の行の最初から始めるようにしたいです。これをするために、`Writer`の`new_line`というメソッドの実装を追加します。

```rust
// in src/vga_buffer.rs

impl Writer {
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {/* TODO */}
}
```
すべての画面の文字をイテレートし、それぞれの文字を一行上に動かします。範囲記法 (`..`) は上端を含まないことに注意してください。また、0行目はシフトしたら画面から除かれるので、この行についても省いています（最初の範囲は`1`から始まっています）。

newlineのプログラムを完成させるには、`clear_row`メソッドを追加すればよいです：

```rust
// in src/vga_buffer.rs

impl Writer {
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}
```
このメソッドはすべての文字を空白文字で書き換えることによって行をクリアしてくれます。

## <ruby>大域的<rp> (</rp><rt>global</rt><rp>) </rp></ruby>なインターフェース
`Writer`のインスタンスを動かさずとも他のモジュールからインターフェースとして使える、大域的なwriterを提供するために、<ruby>静的<rp> (</rp><rt>static</rt><rp>) </rp></ruby>な`WRITER`を作りましょう：

```rust
// in src/vga_buffer.rs

pub static WRITER: Writer = Writer {
    column_position: 0,
    color_code: ColorCode::new(Color::Yellow, Color::Black),
    buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
};
```

しかし、これをコンパイルしようとすると、次のエラーが起こります：

```
error[E0015]: calls in statics are limited to constant functions, tuple structs and tuple variants
（エラー[E0015]: static内における呼び出しは、定数関数、タプル構造体、タプルヴァリアントに限定されています）
 --> src/vga_buffer.rs:7:17
  |
7 |     color_code: ColorCode::new(Color::Yellow, Color::Black),
  |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error[E0396]: raw pointers cannot be dereferenced in statics
（エラー[E0396]: 生ポインタはstatic内では参照外しできません）
 --> src/vga_buffer.rs:8:22
  |
8 |     buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  |                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ dereference of raw pointer in constant
  |                                                                        （定数内での生ポインタの参照外し）

error[E0017]: references in statics may only refer to immutable values
（エラー[E0017]: static内における参照が参照してよいのは不変変数だけです）
 --> src/vga_buffer.rs:8:22
  |
8 |     buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  |                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ statics require immutable values
  |                                                                        （staticは不変変数を必要とします）

error[E0017]: references in statics may only refer to immutable values
（エラー[E0017]: static内における参照が参照してよいのは不変変数だけです）
 --> src/vga_buffer.rs:8:13
  |
8 |     buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ statics require immutable values
  |                                                                                 （staticは不変変数を必要とします）
```

何が起こっているかを理解するには、実行時に初期化される通常の変数とは対照的に、静的変数はコンパイル時に初期化されるということを知らないといけません。この初期化表現を評価するRustコンパイラのコンポーネントを"[const evaluator]"といいます。この機能はまだ限定的ですが、「[定数内でpanicできるようにする][Allow panicking in constants]」RFCのように、この機能を拡張する作業が現在も進行しています。

[const evaluator]: https://rustc-dev-guide.rust-lang.org/const-eval.html
[Allow panicking in constants]: https://github.com/rust-lang/rfcs/pull/2345

`ColorCode::new`に関する問題は[`const`関数][`const` functions]を使って解決できるかもしれませんが、ここでの根本的な問題は、Rustのconst evaluatorがコンパイル時に生ポインタを参照へと変えることができないということです。いつかうまく行くようになるのかもしれませんが、その時までは、別の方法を行わなければなりません。

[`const` functions]: https://doc.rust-lang.org/unstable-book/language-features/const-fn.html

### <ruby>怠けた<rp> (</rp><rt>Lazy</rt><rp>) </rp></ruby>静的変数
定数でない関数で一度だけ静的変数を初期化したい、というのはRustにおいてよくある問題です。嬉しいことに、[lazy_static]というクレートにすでに良い解決方法が存在します。このクレートは、初期化が後回しにされる`static`を定義する`lazy_static!`マクロを提供します。その値をコンパイル時に計算する代わりに、この`static`は最初にアクセスされたときに初めて初期化します。したがって、初期化は実行時に起こるので、どんなに複雑な初期化プログラムも可能ということです。

<div class="note">

**訳注:**  lazyは、普通「遅延（評価）」などと訳されます。「怠けているので、アクセスされるギリギリまで評価されない」という英語のイメージを伝えたかったので上のように訳してみました。

</div>

[lazy_static]: https://docs.rs/lazy_static/1.0.1/lazy_static/

私達のプロジェクトに`lazy_static`クレートを追加しましょう：

```toml
# in Cargo.toml

[dependencies.lazy_static]
version = "1.0"
features = ["spin_no_std"]
```

標準ライブラリをリンクしないので、`spin_no_std`機能が必要です。

`lazy_static`を使えば、静的な`WRITER`が問題なく定義できます：

```rust
// in src/vga_buffer.rs

use lazy_static::lazy_static;

lazy_static! {
    pub static ref WRITER: Writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };
}
```

しかし、この`WRITER`は<ruby>不変<rp> (</rp><rt>immutable</rt><rp>) </rp></ruby>なので、全く使い物になりません。なぜならこれは、この`WRITER`に何も書き込めないということを意味するからです（私達のすべての書き込みメソッドは`&mut self`を取るからです）。ひとつの解決策には、[<ruby>可変<rp> (</rp><rt>mutable</rt><rp>) </rp></ruby>で静的な変数][mutable static]を使うということがあります。しかし、そうすると、あらゆる読み書きが容易にデータ競合やその他の良くないことを引き起こしてしまうので、それらがすべてunsafeになってしまいます。`static mut`を使うことも、[それを削除しようという提案][remove static mut]すらあることを考えると、できる限り避けたいです。しかし他に方法はあるのでしょうか？不変静的変数を[RefCell]や、果ては[UnsafeCell]のような、[<ruby>内部可変性<rp> (</rp><rt>interior mutability</rt><rp>) </rp></ruby>][interior mutability]を提供するcell型と一緒に使うという事も考えられます。しかし、それらの型は（ちゃんとした理由があって）[Sync]ではないので、静的変数で使うことはできません。

[mutable static]: https://doc.rust-jp.rs/book-ja/ch19-01-unsafe-rust.html#可変で静的な変数にアクセスしたり変更する
[remove static mut]: https://internals.rust-lang.org/t/pre-rfc-remove-static-mut/1437
[RefCell]: https://doc.rust-jp.rs/book-ja/ch15-05-interior-mutability.html#refcelltで実行時に借用を追いかける
[UnsafeCell]: https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html
[interior mutability]: https://doc.rust-jp.rs/book-ja/ch15-05-interior-mutability.html
[Sync]: https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html

### スピンロック
同期された内部可変性を得るためには、標準ライブラリを使えるなら[Mutex]を使うことができます。これは、リソースがすでにロックされていた場合、スレッドをブロックすることにより相互排他性を提供します。しかし、私達の初歩的なカーネルにはブロックの機能はもちろんのこと、スレッドの概念すらないので、これも使うことはできません。しかし、コンピュータサイエンスの世界には、OSを必要としない非常に単純なmutexが存在するのです：それが[<ruby>スピンロック<rp> (</rp><rt>spinlock</rt><rp>) </rp></ruby>][spinlock]です。スピンロックを使うと、ブロックする代わりに、スレッドは単純にリソースを何度も何度もロックしようとすることで、mutexが開放されるまでの間CPU時間を使い尽くします。

[Mutex]: https://doc.rust-lang.org/nightly/std/sync/struct.Mutex.html
[spinlock]: https://ja.wikipedia.org/wiki/スピンロック

スピンロックによるmutexを使うには、[spinクレート][spin crate]への依存を追加すればよいです：

[spin crate]: https://crates.io/crates/spin

```toml
# in Cargo.toml
[dependencies]
spin = "0.5.2"
```

すると、スピンを使ったMutexを使うことができ、静的な`WRITER`に安全な[内部可変性][interior mutability]を追加できます。

```rust
// in src/vga_buffer.rs

use spin::Mutex;
...
lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}
```
`print_something`関数を消して、`_start`関数から直接出力しましょう：

```rust
// in src/main.rs
#[no_mangle]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    write!(vga_buffer::WRITER.lock(), ", some numbers: {} {}", 42, 1.337).unwrap();

    loop {}
}
```
`fmt::Write`トレイトの関数を使うためには、このトレイトをインポートする必要があります。

### 安全性
コードにはunsafeブロックが一つ（`0xb8000`を指す参照`Buffer`を作るために必要なもの）しかないことに注目してください。その後は、すべての命令が<ruby>安全<rp> (</rp><rt>safe</rt><rp>) </rp></ruby>です。Rustは配列アクセスにはデフォルトで境界チェックを行うので、間違ってバッファの外に書き込んでしまうことはありえません。よって、必要とされる条件を型システムにすべて組み込んだので、安全なインターフェースを外部に提供できます。

### printlnマクロ
大域的なwriterを手に入れたので、プログラムのどこでも使える`println`マクロを追加できます。Rustの[マクロの構文][macro syntax]はすこしややこしいので、一からマクロを書くことはしません。代わりに、標準ライブラリで[`println!`マクロ][`println!` macro]のソースを見てみます：

[macro syntax]: https://doc.rust-lang.org/nightly/book/ch19-06-macros.html#declarative-macros-with-macro_rules-for-general-metaprogramming
[`println!` macro]: https://doc.rust-lang.org/nightly/std/macro.println!.html

```rust
#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}
```

マクロは1つ以上のルールを使って定義されます（`match`アームと似ていますね）。`println`には2つのルールがあります：1つ目は引数なし呼び出し（例えば `println!()`）のためのもので、これは`print!("\n")`に展開され、よってただ改行を出力するだけになります。2つ目のルールはパラメータ付きの呼び出し（例えば`println!("Hello")`や `println!("Number: {}", 4)`）のためのものです。これも`print!`マクロの呼び出しへと展開され、すべての引数に加え、改行`\n`を最後に追加して渡します。

`#[macro_export]`属性はマクロを（その定義されたモジュールだけではなく）クレート全体および外部クレートで使えるようにします。また、これはマクロをクレートルートに置くため、`std::macros::println`の代わりに`use std::println`を使ってマクロをインポートしないといけないということを意味します。

[`print!`マクロ][`print!` macro]は以下のように定義されています：

[`print!` macro]: https://doc.rust-lang.org/nightly/std/macro.print!.html

```rust
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
```

このマクロは`io`モジュール内の[`_print`関数][`_print` function]の呼び出しへと展開しています。[`$crate`という変数][`$crate` variable]は、他のクレートで使われた際、`std`へと展開することによって、マクロが`std`クレートの外側で使われたとしてもうまく動くようにしてくれます。

[`format_args`マクロ][`format_args` macro]が与えられた引数から[fmt::Arguments]型を作り、これが`_print`へと渡されています。libstdの[`_print`関数]は`print_to`を呼び出すのですが、これは様々な`Stdout`デバイスをサポートいているためかなり煩雑です。ここではただVGAバッファに出力したいだけなので、そのような煩雑な実装は必要ありません。

[`_print` function]: https://github.com/rust-lang/rust/blob/29f5c699b11a6a148f097f82eaa05202f8799bbc/src/libstd/io/stdio.rs#L698
[`$crate` variable]: https://doc.rust-lang.org/1.30.0/book/first-edition/macros.html#the-variable-crate
[`format_args` macro]: https://doc.rust-lang.org/nightly/std/macro.format_args.html
[fmt::Arguments]: https://doc.rust-lang.org/nightly/core/fmt/struct.Arguments.html

VGAバッファに出力するには、`println!`マクロと`print!`マクロをコピーし、独自の`_print`関数を使うように修正してやればいいです：

```rust
// in src/vga_buffer.rs

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}
```

元の`println`の定義と異なり、`print!`マクロの呼び出しにも`$crate`をつけるようにしています。これにより、`println`だけを使いたいと思ったら`print!`マクロもインポートしなくていいようになります。

標準ライブラリのように、`#[macro_export]`属性を両方のマクロに与え、クレートのどこでも使えるようにします。このようにすると、マクロはクレートの名前空間のルートに置かれるので、`use crate::vga_buffer::println`としてインポートするとうまく行かないことに注意してください。代わりに、 `use crate::println`としなければいけません。

`_print`関数は静的な`WRITER`をロックし、その`write_fmt`メソッドを呼び出します。このメソッドは`Write`トレイトのものなので、このトレイトもインポートしないといけません。最後に追加した`unwrap()`は、画面出力がうまく行かなかったときパニックします。しかし、`write_str`は常に`Ok`を返すようにしているので、これは起きないはずです。

マクロは`_print`をモジュールの外側から呼び出せる必要があるので、この関数は<ruby>公開<rp> (</rp><rt>public</rt><rp>) </rp></ruby>されていなければなりません。しかし、これは<ruby>非公開<rp> (</rp><rt>private</rt><rp>) </rp></ruby>の実装の詳細であると考え、[`doc(hidden)`属性][`doc(hidden)` attribute]をつけることで、生成されたドキュメントから隠すようにします。

[`doc(hidden)` attribute]: https://doc.rust-lang.org/nightly/rustdoc/the-doc-attribute.html#dochidden

### `println`を使ってHello World
こうすることで、`_start`関数で`println`を使えるようになります：

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() {
    println!("Hello World{}", "!");

    loop {}
}
```

マクロはすでに名前空間のルートにいるので、main関数内でマクロをインポートしなくても良いということに注意してください。

期待通り、画面に Hello World! と出ています：

![QEMU printing “Hello World!”](vga-hello-world.png)

### パニックメッセージを出力する

`println`マクロを手に入れたので、これを私達のパニック関数で使って、パニックメッセージとパニックの場所を出力させることができます：

```rust
// in main.rs

/// この関数はパニック時に呼ばれる。
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
```
`panic!("Some panic message");`という文を`_start`関数に書くと、次の出力を得ます：

![QEMU printing “panicked at 'Some panic message', src/main.rs:28:5](vga-panic.png)

つまり、パニックが起こったということだけでなく、パニックメッセージとそれがコードのどこで起こったかまで知ることができます。

## まとめ
この記事では、VGAテキストバッファの構造と、どのようにすれば`0xb8000`番地におけるメモリマッピングを通じてそれに書き込みを行えるかを学びました。このメモリマップされたバッファへの書き込みというunsafeな操作をカプセル化し、安全で便利なインターフェースを外部に提供するRustモジュールを作りました。

また、cargoのおかげでサードパーティのライブラリへの依存関係を簡単に追加できることも分かりました。`lazy_static`と`spin`という2つの依存先は、OS開発においてとても便利であり、今後の記事においても使っていきます。

## 次は？
次の記事ではRustに組み込まれている単体テストフレームワークをセットアップする方法を説明します。その後、この記事のVGAバッファモジュールに対する基本的な単体テストを作ります。
