+++
title = "獨立的 Rust 二進制檔"
weight = 1
path = "zh-TW/freestanding-rust-binary"
date = 2018-02-10

[extra]
commit = 24d04e0e39a3395ecdce795bab0963cb6afe1bfd

+++

建立我們自己的作業系統核心的第一步是建立一個不連結標準函式庫的 Rust 執行檔，這使得無需基礎作業系統即可在[裸機][bare metal]上執行 Rust 程式碼。

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

此網誌在 [GitHub] 上公開開發，如果您有任何問題或疑問，請在那開一個 issue，您也可以在[下面][at the bottom]發表評論，這篇文章的完整開源程式碼可以在 [`post-01`][post branch] 分支中找到。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## 介紹
要編寫作業系統核心，我們需要不依賴於任何作業系統功能的程式碼。這代表我們不能使用執行緒、檔案系統、堆記憶體、網路、隨機數、標準輸出或任何其他需要作業系統抽象或特定硬體的功能。這也是理所當然的，因為我們正在嘗試寫出自己的 OS 和我們的驅動程式。

這意味著我們不能使用大多數的 [Rust 標準函式庫][Rust standard library]，但是我們還是可以使用 _很多_ Rust 的功能。比如說我們可以使用[疊代器][iterators]、[閉包][closures]、[模式配對][pattern matching]、[option] 和 [result]、[字串格式化][string formatting]，當然還有[所有權系統][ownership system]。這些功能讓我們能夠以非常有表達力且高階的方式編寫核心，而無需擔心[未定義行為][undefined behavior]或[記憶體安全][memory safety]。

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

為了在 Rust 中建立 OS 核心，我們需要建立一個無須底層作業系統即可運行的執行檔，這類的執行檔通常稱為「獨立式（freestanding）」或「裸機（bare-metal）」的執行檔。

這篇文章描述了建立一個獨立的 Rust 執行檔的必要步驟，並解釋為什麼需要這些步驟。如果您只對簡單的範例感興趣，可以直接跳到 **[總結](#總結)**。

## 停用標準函式庫

Rust 所有的 crate 在預設情況下都會連結[標準函式庫][standard library]，而標準函式庫會依賴作業系統的功能，像式執行緒、檔案系統或是網路。它也會依賴 C 語言的標準函式庫 `libc`，因為其與作業系統緊密相關。既然我們的計劃是編寫自己的作業系統，我們就得用到 [`no_std` 屬性][`no_std` attribute]來停止標準函式庫的自動引用（automatic inclusion）。

[standard library]: https://doc.rust-lang.org/std/
[`no_std` attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

我們先從建立一個新的 cargo 專案開始，最簡單的辦法是輸入下面的命令：

```
cargo new blog_os --bin --edition 2018
```

我將專案命名為 `blog_os`，當然讀者也可以自己的名稱。`--bin` 選項說明我們將要建立一個執行檔（而不是一個函式庫），`--edition 2018` 選項指明我們的 crate 想使用 Rust [2018 版本][2018 edition]。當我們執行這行指令的時候，cargo 會為我們建立以下目錄結構：

[2018 edition]: https://rust-lang-nursery.github.io/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

`Cargo.toml` 包含 crate 的設置，像是 crate 的名稱、作者、[語意化版本][semantic version]以及依賴套件。`src/main.rs` 檔案則包含 crate 的根模組（root module）以及我們的 `main` 函式。您可以用 `cargo build` 編譯您的 crate 然後在 `target/debug` 目錄下運行編譯過後的 `blog_os` 執行檔。

[semantic version]: https://semver.org/lang/zh-TW/

### no_std 屬性

現在我們的 crate 背後依然有和標準函式庫連結。讓我們加上 [`no_std` 屬性][`no_std` attribute] 來停用：

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

當我們嘗試用 `cargo build` 編譯時會出現以下錯誤訊息：

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

出現這個錯誤的原因是因為 [`println` 巨集（macro）][`println` macro]是標準函式庫的一部份，而我們不再包含它，所以我們無法再輸出東西來。這也是理所當然因為 `println` 會寫到[標準輸出][standard output]，而這是一個由作業系統提供的特殊檔案描述符。

[`println` macro]: https://doc.rust-lang.org/std/macro.println.html
[standard output]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

所以讓我們移除這行程式碼，然後用空的 main 函式再試一次：

```rust
// main.rs

#![no_std]

fn main() {}
```

```
> cargo build
error: `#[panic_handler]` function required, but not found
error: language item required, but not found: `eh_personality`
```

現在編譯氣告訴我們缺少 `#[panic_handler]` 函式以及 _language item_。

## 實作 panic 處理函式

`panic_handler` 屬性定義了當 [panic] 發生時編譯器需要呼叫的函式。在標準函式庫中有自己的 panic 處理函式，但在 `no_std` 的環境中我們得定義我們自己的：

[panic]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// main.rs

use core::panic::PanicInfo;

/// 此函式會在 panic 時呼叫。
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

[`PanicInfo` parameter][PanicInfo] 包含 panic 發生時的檔案、行數以及可選的錯誤訊息。這個函式不會返回，所以它被標記為[發散函式][diverging function]，只會返回[“never” 型態][“never” type] `!`。現在我們什麼事可以做，所以我們只需寫一個無限迴圈。

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[“never” type]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## eh_personality Language Item

Language item 是一些編譯器需求的特殊函式或類型。舉例來說，Rust 的 [`Copy`] trait 就是一個 language item，告訴編譯器哪些類型擁有[_複製的語意_][`Copy`]。當我們搜尋 `Copy` trait 的[實作][copy code]時，我們會發現一個特殊的 `#[lang = "copy"]` 屬性將它定義為一個 language item。

我們可以自己實現 language item，但這只應是最後的手段。因為 language item 屬於非常不穩定的實作細節，而且不會做類型檢查（所以編譯器甚至不會確保它們的參數類型是否正確）。幸運的是，我們有更穩定的方式來修復上面關於 language item 的錯誤。

`eh_personality` language item 標記的函式將被用於實作[堆疊回溯][stack unwinding]。在預設情況下當 panic 發生時，Rust 會使用堆疊回溯來執行所有存在堆疊上變數的解構子（destructor）。這確保所有使用的記憶體都被釋放，並讓 parent thread 獲取 panic 資訊並繼續運行。但是堆疊回溯是一個複雜的過程，通常會需要一些 OS 的函式庫如 Linux 的 [libunwind] 或 Windows 的 [structured exception handling]。所以我們並不希望在我們的作業系統中使用它。

[stack unwinding]: http://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: http://www.nongnu.org/libunwind/
[structured exception handling]: https://msdn.microsoft.com/en-us/library/windows/desktop/ms680657(v=vs.85).aspx

### 停用回溯

在某些狀況下回溯可能並不是我們要的功能，因此 Rust 提供了[在 panic 時中止][abort on panic]的選項。這個選項能停用回溯標誌訊息的產生，也因此能縮小生成的二進制檔案大小。我們能用許多方式開啟這個選項，而最簡單的方式就是把以下幾行設置加入我們的 `Cargo.toml`：

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

這些選項能將　`dev` 設置（用於 `cargo build`）和 `release` 設置（用於 `cargo build --release`）的 panic 策略設為 `abort`。現在編譯器不會再要求我們提供 `eh_personality` language item。

[abort on panic]: https://github.com/rust-lang/rust/pull/32900

現在我們已經修復了上面的錯誤，但是如果我們嘗試編譯的話，又會出現一個新的錯誤：

```
> cargo build
error: requires `start` lang_item
```

我們的程式缺少 `start` 這個用來定義入口點（entry point）的 language item。

## `start` 屬性

我們通常會認為執行一個程式時，首先被呼叫的是 `main` 函式。但是大多數語言都擁有一個[執行時系統][runtime system]，它通常負責垃圾回收（garbage collection）像是 Java 或軟體執行緒（software threads）像是 Go 的 goroutines。這個執行時系統需要在 main 函式前啟動，因為它需要讓先進行初始化。

[runtime system]: https://en.wikipedia.org/wiki/Runtime_system

在一個典型使用標準函式庫的 Rust 程式中，程式運行是從一個名為 `crt0`（“C runtime zero”）的執行時函式庫開始的，它會設置 C 程式的執行環境。這包含建立堆疊和可執行程式參數的傳入。在這之後，這個執行時函式庫會呼叫 [Rust 的執行時入口點][rt::lang_start]，而此處就是由 `start` language item 標記。 Rust 只有一個非常小的執行時系統，負責處理一些小事情，像是堆疊溢位或是印出 panic 時回溯的訊息。再來執行時系統最終才會呼叫 main 函式。

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

我們的獨立式可執行檔並沒有辦法存取 Rust 執行時系統或 `crt0`，所以我們需要定義自己的入口點。實作 `start` language item 並沒有用，因為這樣還是會需要 `crt0`。所以我們要做的是直接覆寫 `crt0` 的入口點。

### 重寫入口點

為了告訴 Rust 編譯器我們不要使用一般的入口點呼叫順序，我們先加上 `#![no_main]` 屬性。

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// 此函式會在 panic 時呼叫。
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

您可能會注意到我們移除了 `main` 函式，原因是因為既然沒有了底層的執行時系統呼叫，那麼 `main` 也沒必要存在。我們要重寫作業系統的入口點，定義為 `_start` 函式：

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

我們使用 `no_mangle` 屬性來停用[名字修飾][name mangling]，確保 Rust 編譯器輸出的函式名稱會是 `_start`。沒有這個屬性的話，編譯器會產生符號像是 `_ZN3blog_os4_start7hb173fedf945531caE` 來讓每個函式的名稱都是獨一無二的。我們會需要這項屬性的原因是因為我們接下來希望連結器能夠呼叫入口點函式的名稱。

我們還將函式標記為 `extern "C"` 來告訴編譯器這個函式應當使用 [C 的調用約定][C calling convention]，而不是 Rust 的調用約定。而函式名稱選用 `_start` 的原因是因為這是大多數系統的預設入口點名稱。

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[C calling convention]: https://en.wikipedia.org/wiki/Calling_convention

`!` 返回型態代表這個函式是發散函式，它不允許返回。這是必要的因為入口點不會被任何函式呼叫，只會直接由作業系統或啟動程式（bootloader）執行。所以取代返回值的是入口點需要執行作業系統的 [`exit` 系統呼叫][`exit` system call]。在我們的例子中，關閉機器似乎是個理想的動作，因為獨立的二進制檔案返回後也沒什麼事可做。現在我們先寫一個無窮迴圈來滿足需求。

[`exit` system call]: https://en.wikipedia.org/wiki/Exit_(system_call)

當我們現在運行 `cargo build` 的話會看到很醜的 _連結器_ 錯誤。

## 連結器錯誤

連結器是用來將產生的程式碼結合起來成為執行檔的程式。因為 Linux、Windows 和 macOS 之間的執行檔格式都不同，每個系統都會有自己的連結器錯誤。不過造成錯誤的原因通常都差不多：連結器預設的設定會認為我們的程式依賴於 C 的執行時系統，但我們並沒有。

為了解決這個錯誤，我們需要告訴連結器它不需要包含 C 的執行時系統。我們可以選擇提供特定的連結器參數設定，或是選擇編譯為裸機目標。

### 編譯為裸機目標

Rust 在預設情況下會嘗試編譯出符合你目前系統環境的可執行檔。舉例來說，如果你正在 `x86_64` 上使用 Windows，那麼 Rust 就會嘗試編譯出 `.exe`，一個使用 `x86_64` 指令集的 Windows 執行檔。這樣的環境稱之為主機系統（host system）。

為了描述不同環境，Rust 使用 [_target triple_] 的字串。要查看目前系統的 target triple，你可以執行 `rustc --version --verbose`：

[_target triple_]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple

```
rustc 1.35.0-nightly (474e7a648 2019-04-07)
binary: rustc
commit-hash: 474e7a6486758ea6fc761893b1a49cd9076fb0ab
commit-date: 2019-04-07
host: x86_64-unknown-linux-gnu
release: 1.35.0-nightly
LLVM version: 8.0
```

上面的輸出訊息來自 `x86_64` 上的 Linux 系統，我們可以看到 `host` 的 target triple 為 `x86_64-unknown-linux-gnu`，分別代表 CPU 架構 (`x86_64`)、供應商 (`unknown`) 以及作業系統 (`linux`) 和 [ABI] (`gnu`)。

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

在依據主機的 triple 編譯時，Rust 編譯器和連結器理所當然地會認為預設是底層的作業系統並使用 C 執行時系統，這便是造成錯誤的原因。要避免這項錯誤，我們可以選擇編譯出沒有底層作業系統的不同環境。

其中一個裸機環境的例子是 `thumbv7em-none-eabihf` target triple，它描述了[嵌入式][embedded] [ARM] 系統。其中的細節目前並不重要，我們現在只需要知道沒有底層作業系統的 target triple 是用 `none` 描述的。想要編譯這樣的目標的話，我們需要將它新增至 rustup：

[embedded]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture

```
rustup target add thumbv7em-none-eabihf
```

這會下載一份該系統的標準（以及 core）函式庫，現在我們可以用此目標建立我們的獨立執行檔了：

```
cargo build --target thumbv7em-none-eabihf
```

我們傳入 `--target` [交叉編譯][cross compile]我們在裸機系統的執行檔。因為目標系統沒有作業系統，連結器不會嘗試連結 C 執行時系統並成功建立，不會產生任何連結器錯誤。

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

這將會是我們到時候用來建立自己的作業系統核心的方法。不過我們不會用到 `thumbv7em-none-eabihf`，我們將會使用[自訂目標][custom target]來描述一個 `x86_64` 的裸機環境。

[custom target]: https://doc.rust-lang.org/rustc/targets/custom.html

### 連結器引數

除了編譯裸機系統為目標以外，我們也可以傳入特定的引數組合給連結器來解決錯誤。這不會是我們到時候用在我們核心的方法，所以以下的內容不是必需的，只是用來補齊資訊。點選下面的 _「連結器引數」_ 來顯示額外資訊。

<details>

<summary>連結器引數</summary>

在這部份我們將討論 Linux、Windows 和 macOS 上發生的連結器錯誤，然後解釋如何傳入額外引數給連結器以解決錯誤。注意執行檔和連結器在不同作業系統之間都會相異，所以不同系統需要傳入不同引數。

#### Linux

以下是 Linux 上會出現的（簡化過）連結器錯誤：

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

問題的原因是因為連結器在一開始包含了 C 的執行時系統，而且剛好也叫做 `_start`。它需要一些 C 標準函式庫 `libc` 提供的符號，但我們用 `no_std` 來停用它了，所以連結器無法找出引用來源。我們可以用 `-nostartfiles` 來告訴連結器一開始不必連結 C 的執行時系統。

要傳入的其中一個方法是透過 cargo 的 `cargo rustc` 命令，此命令行為和  `cargo build` 一樣，不過允許傳入一些選項到 Rust 底層的編譯器 `rustc`。`rustc` 有 `-C link-arg` 的選項會繼續將引數傳到連結器，這樣一來我們的指令會長得像這樣：

```
cargo rustc -- -C link-arg=-nostartfiles
```

現在我們的 crate 便能產生出 Linux 上的獨立執行檔了！

我們不必再指明入口點的函式名稱，因為連結器預設會尋找 `_start` 函式。
#### Windows

在 Windows 上會出現不一樣的（簡化過）連結器錯誤：

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

"entry point must be defined" 錯誤表示連結器找不到入口點，在 Windows 上預設的入口點名稱會[依據使用的子系統][windows-subsystems]。如果是 `CONSOLE` 子系統的話，連結器會尋找 `mainCRTStartup` 函式名稱；而 `WINDOWS` 子系統的話則會尋找 `WinMainCRTStartup` 函式名稱。要覆蓋預設的選項並讓連結器尋找我們的 `_start` 函式的話，我們可以傳入 `/ENTRY` 引數給連結器：

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

從引數格式來看我們可以清楚理解 Windows 連結器與 Linux 連結器是完全不同的程式。

現在會出現另一個連結器錯誤：

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

此錯誤出現的原因是因為 Windows 執行檔可以使用不同的[子系統][windows-subsystems]。一般的程式會依據入口點名稱來決定：如果入口點名稱為 `main`　則會使用 `CONSOLE` 子系統；如果入口點名稱為 `WinMain` 則會使用 `WINDOWS` 子系統。由於我們的函式 `_start` 名稱不一樣，我們必須指明子系統：

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

我們使用 `CONSOLE` 子系統不過 `WINDOWS` 一樣也可以。與其輸入好多次 `-C link-arg` ，我們可以用 `-C link-args` 來傳入許多引數。

使用此命令後，我們的執行檔應當能成功在 Windows 上建立。

#### macOS

以下是 Linux 上會出現的（簡化過）連結器錯誤：

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

此錯誤訊息告訴我們連結器無法找到入口點函式 `main`，基於某些原因 macOS 上的函式都會加上前綴 `_`。為了設定入口點為我們的函式 `_start`，我們傳入 `-e` 連結器引數：

```
cargo rustc -- -C link-args="-e __start"
```

`-e` 表示肉口點的函式名稱，然後由於 macOS 上所有的函式都會加上前綴 `_`，我們需要設置入口點為 `__start` 而不是 `_start`。

接下來會出現另一個連結器錯誤：

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS [官方並不支援靜態連結執行檔][does not officially support statically linked binaries]且要求程式預設要連結到 `libSystem` 函式庫。要覆蓋這個設定並連結靜態執行檔，我們傳入 `-static` 給連結器：

[does not officially support statically linked binaries]: https://developer.apple.com/library/content/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

但這樣還不夠，我們會遇到第三個連結器錯誤：

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

這錯誤出現的原因是因為 macOS 的程式預設都會連結到 `crt0` (“C runtime zero”)。這和我們在 Linux 上遇到的類似，所以也可以用 `-nostartfiles` 連結器引數來解決：

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

現在我們的程式應當能成功在 macOS 上建立。

#### 統一建構命令

現在我們得依據主機平台來使用不同的建構命令，這樣感覺不是很理想。我們可以建立個檔案 `.cargo/config` 來解決，裡面會包含平台相關的引數：

```toml
# in .cargo/config

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

`rustflags` 包含的引數會自動加到 `rustc` 如果條件符合的話。想了解更多關於 `.cargo/config` 的資訊請參考[官方文件][official documentation](https://doc.rust-lang.org/cargo/reference/config.html)。

這樣一來我們就能同時在三個平台只用 `cargo build` 來建立了。

#### 你該這麼作嗎？

雖然我們可以在 Linux、Windows 和 macOS 上建立獨立執行檔，不過這可能不是好主意。我們目前會需要這樣做的原因是因為我們的執行檔仍然需要仰賴一些事情，像是當 `_start` 函式呼叫時堆疊已經初始化完畢。少了 C 執行時系統，有些要求可能會無法達成，造成我們的程式失效，像是 segmentation fault。

如果你想要建立一個運行在已存作業系統上的最小執行檔，改用 `libc` 然後如這邊[所述](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html)設置 `#[start]` 屬性可能會是更好的做法。

</details>

## 總結

一個最小的 Rust 獨立執行檔會看起來像這樣：

`src/main.rs`：

```rust
#![no_std] // 不連結標準函式庫
#![no_main] // 停用 Rust 層級的入口點

use core::panic::PanicInfo;

#[no_mangle] // 不修飾函式名稱
pub extern "C" fn _start() -> ! {
    // 因為連結器預設會尋找 `_start` 函式名稱
    // 所以這個函式就是入口點
    loop {}
}

/// 此函式會在 panic 時呼叫
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

`Cargo.toml`：

```toml
[package]
name = "crate_name"
version = "0.1.0"
authors = ["Author Name <author@example.com>"]

# `cargo build` 時需要的設置
[profile.dev]
panic = "abort" # 停用 panic 時堆疊回溯

# `cargo build --release` 時需要的設置
[profile.release]
panic = "abort" # 停用 panic 時堆疊回溯
```

要建構出此執行檔，我們需要選擇一個裸機目標來編譯像是 `thumbv7em-none-eabihf`：

```
cargo build --target thumbv7em-none-eabihf
```

不然我們也可以用主機系統來編譯，不過要加上額外的連結器引數：

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

注意這只是最小的 Rust 獨立執行檔範例，它還是會仰賴一些事情發生，像是當 `_start` 函式呼叫時堆疊已經初始化完畢。**所以如果想真的使用這樣的執行檔的話還需要更多步驟。**

## 接下來呢？

[下一篇文章][next post] 將會講解如何將我們的獨立執行檔轉成最小的作業系統核心。這包含建立自訂目標、用啟動程式組合我們的執行檔，還有學習如何輸出一些東西到螢幕上。

[next post]: @/second-edition/posts/02-minimal-rust-kernel/index.md