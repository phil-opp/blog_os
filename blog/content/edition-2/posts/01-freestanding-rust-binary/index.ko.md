+++
title = "Rust로 'Freestanding 실행파일' 만들기"
weight = 1
path = "ko/freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "c1af4e31b14e562826029999b9ab1dce86396b93"
# GitHub usernames of the people that translated this post
translators = ["JOE1994", "Quqqu"]
+++

운영체제 커널을 만드는 첫 단계는 표준 라이브러리(standard library)를 링크하지 않는 Rust 실행파일을 만드는 것입니다. 이 실행파일은 운영체제가 없는 [bare metal] 시스템에서 동작할 수 있습니다.

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

이 블로그는 [GitHub 저장소][GitHub]에서 오픈 소스로 개발되고 있으니, 문제나 문의사항이 있다면 저장소의 'Issue' 기능을 이용해 제보해주세요. [페이지 맨 아래][at the bottom]에 댓글을 남기실 수도 있습니다. 이 포스트와 관련된 모든 소스 코드는 저장소의 [`post-01 브랜치`][post branch]에서 확인하실 수 있습니다.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## 소개
운영체제 커널을 만드려면 운영체제에 의존하지 않는 코드가 필요합니다. 자세히 설명하자면, 스레드, 파일, 동적 메모리, 네트워크, 난수 생성기, 표준 출력 및 기타 운영체제의 추상화 또는 특정 하드웨어의 기능을 필요로 하는 것들은 전부 사용할 수 없다는 뜻입니다. 우리는 스스로 운영체제 및 드라이버를 직접 구현하려는 상황이니 어찌 보면 당연한 조건입니다.

운영체제에 의존하지 않으려면 [Rust 표준 라이브러리][Rust standard library]의 많은 부분을 사용할 수 없습니다.
그래도 우리가 이용할 수 있는 Rust 언어 자체의 기능들은 많이 남아 있습니다. 예를 들어 [반복자][iterators], [클로저][closures], [패턴 매칭][pattern matching], [option] / [result], [문자열 포맷 설정][string formatting], 그리고 [소유권 시스템][ownership system] 등이 있습니다. 이러한 기능들은 우리가 커널을 작성할 때 [undefined behavior]나 [메모리 안전성][memory safety]에 대한 걱정 없이 큰 흐름 단위의 코드를 작성하는 데에 집중할 수 있도록 해줍니다.

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

Rust로 운영체제 커널을 작성하려면, 운영체제 없이도 실행가능한 실행파일이 필요합니다. 이러한 실행파일은
보통 "freestanding 실행파일" 혹은 "bare-metal 실행파일" 이라고 불립니다.

이 포스트에서는 "freestanding 실행 파일" 을 만드는 데 필요한 것들을 여러 단계로 나누고, 각 단계가 왜 필요한지에 대해 설명해드립니다. 중간 과정은 생략하고 그저 최소한의 예제 코드만 확인하고 싶으시면 **[요약 섹션으로 넘어가시면 됩니다](#summary)**.

## Rust 표준 라이브러리 링크 해제하기
모든 Rust 프로그램들은 Rust 표준 라이브러리를 링크하는데, 이 라이브러리는 스레드, 파일, 네트워킹 등의 기능을 제공하기 위해 운영체제에 의존합니다. Rust 표준 라이브러리는 또한 C 표준 라이브러리인 `libc`에도 의존합니다 (`libc`는 운영체제의 여러 기능들을 이용합니다).
우리가 운영체제를 직접 구현하기 위해서는 운영체제를 이용하는 라이브러리들은 사용할 수 없습니다. 그렇기에 우선 [`no_std` 속성][`no_std` attribute]을 이용해 자동으로 Rust 표준 라이브러리가 링크되는 것을 막아야 합니다.

[standard library]: https://doc.rust-lang.org/std/
[`no_std` attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

제일 먼저 아래의 명령어를 통해 새로운 cargo 애플리케이션 크레이트를 만듭니다.

```
cargo new blog_os --bin --edition 2018
```

프로젝트 이름은 `blog_os` 또는 원하시는 이름으로 정해주세요. `--bin` 인자는 우리가 cargo에게 실행 파일 (라이브러리와 대조됨)을 만들겠다고 알려주고, `--edition 2018` 인자는 cargo에게 우리가 [Rust 2018 에디션][2018 edition]을 사용할 것이라고 알려줍니다.
위 명령어를 실행하고 나면, cargo가 아래와 같은 크레이트 디렉토리를 만들어줍니다.

[2018 edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

크레이트 설정은 `Cargo.toml`에 전부 기록해야 합니다 (크레이트 이름, 크레이트 원작자, [semantic version] 번호, 의존 라이브러리 목록 등).
`src/main.rs` 파일에 크레이트 실행 시 맨 처음 호출되는 `main` 함수를 포함한 중추 모듈이 있습니다.
`cargo build` 명령어를 통해 크레이트를 빌드하면 `target/debug` 디렉토리에 `blog_os` 실행파일이 생성됩니다.

[semantic version]: https://semver.org/

### `no_std` 속성

현재 우리가 만든 크레이트는 암시적으로 Rust 표준 라이브러리를 링크합니다. 아래와 같이 [`no_std` 속성]을 이용해 더 이상 표준 라이브러리가 링크되지 않게 해줍니다.

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

이제 `cargo build` 명령어를 다시 실행하면 아래와 같은 오류 메세지가 뜰 것입니다:

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

이 오류가 뜨는 이유는 [`println` 매크로][`println` macro]를 제공하는 Rust 표준 라이브러리를 우리의 크레이트에 링크하지 않게 되었기 때문입니다.
`println`은 [표준 입출력][standard output] (운영체제가 제공하는 특별한 파일 서술자)으로 데이터를 쓰기 때문에, 우리는 이제 `println`을 이용해 메세지를 출력할 수 없습니다.

[`println` macro]: https://doc.rust-lang.org/std/macro.println.html
[standard output]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

`println` 매크로 호출 코드를 지운 후 크레이트를 다시 빌드해봅시다.

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

오류 메세지를 통해 컴파일러가 `#[panic_handler]` 함수와 _language item_ 을 필요로 함을 확인할 수 있습니다.

## 패닉 (Panic) 시 호출되는 함수 구현하기

컴파일러는 [패닉][panic]이 일어날 경우 `panic_handler` 속성이 적용된 함수가 호출되도록 합니다. 표준 라이브러리는 패닉 시 호출되는 함수가 제공되지만, `no_std` 환경에서는 우리가 패닉 시 호출될 함수를 직접 설정해야 합니다.

[panic]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// in main.rs

use core::panic::PanicInfo;

/// 패닉이 일어날 경우, 이 함수가 호출됩니다.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

[`PanicInfo` 인자][PanicInfo]는 패닉이 일어난 파일명, 패닉이 파일 내 몇 번째 줄에서 일어났는지, 그리고 패닉시 전달된 메세지에 대한 정보를 가진 구조체입니다.
위 `panic` 함수는 절대로 반환하지 않기에 ["never" 타입][“never” type] `!`을 반환하도록 적어 컴파일러에게 이 함수가 [반환 함수][diverging function]임을 알립니다.
당장 이 함수에서 우리가 하고자 하는 일은 없기에 그저 함수가 반환하지 않도록 무한루프를 넣어줍니다.

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[“never” type]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## `eh_personality` Language Item

Language item은 컴파일러가 내부적으로 요구하는 특별한 함수 및 타입들을 가리킵니다. 예를 들어 [`Copy`] 트레잇은 어떤 타입들이 [_copy semantics_][`Copy`] 를 가지는지 컴파일러에게 알려주는 language item 입니다.
[`Copy` 트레잇이 구현된 코드][copy code]에 있는 `#[lang = "copy"]` 속성을 통해 이 트레잇이 language item으로 선언되어 있음을 확인할 수 있습니다.

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

임의로 구현한 language item을 사용할 수는 있지만, 위험할 수도 있기에 주의해야 합니다.
그 이유는 language item의 구현 코드는 매우 자주 변경되어 불안정하며, language item에 대해서 컴파일러가 타입 체크 조차 하지 않습니다 (예시: language item 함수의 인자 타입이 정확한지 조차 체크하지 않습니다).
임의로 구현한 language item을 이용하는 것보다 더 안정적으로 위의 language item 오류를 고칠 방법이 있습니다.

[`eh_personality` language item]은 [스택 되감기 (stack unwinding)][stack unwinding]을 구현하는 함수를 가리킵니다. 기본적으로 Rust는 [패닉][panic]이 일어났을 때 스택 되감기를 통해 스택에 살아있는 각 변수의 소멸자를 호출합니다. 이를 통해 자식 스레드에서 사용 중이던 모든 메모리 리소스가 반환되고, 부모 스레드가 패닉에 대처한 후 계속 실행될 수 있게 합니다. 스택 되감기는 복잡한 과정으로 이루어지며 운영체제마다 특정한 라이브러리를 필요로 하기에 (예: Linux는 [libunwind], Windows는 [structured exception handling]), 우리가 구현할 운영체제에서는 이 기능을 사용하지 않을 것입니다.

[`eh_personality` language item]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: https://www.nongnu.org/libunwind/
[structured exception handling]: https://docs.microsoft.com/en-us/windows/win32/debug/structured-exception-handling

### 스택 되감기를 해제하는 방법

스택 되감기가 불필요한 상황들이 여럿 있기에, Rust 언어는 [패닉 시 실행 종료][abort on panic] 할 수 있는 선택지를 제공합니다. 이는 스택 되감기에 필요한 심볼 정보 생성을 막아주어 실행 파일의 크기 자체도 많이 줄어들게 됩니다. 스택 되감기를 해제하는 방법은 여러가지 있지만, 가장 쉬운 방법은 `Cargo.toml` 파일에 아래의 코드를 추가하는 것입니다.

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

위의 코드를 통해 `dev` 빌드 (`cargo build` 실행)와 `release` 빌드 (`cargo build --release` 실행) 에서 모두 패닉 시 실행이 종료되도록 설정되었습니다.
이제 더 이상 컴파일러가 `eh_personality` language item을 필요로 하지 않습니다.

[abort on panic]: https://github.com/rust-lang/rust/pull/32900

위에서 본 오류들을 고쳤지만, 크레이트를 빌드하려고 하면 새로운 오류가 뜰 것입니다:

```
> cargo build
error: requires `start` lang_item
```

우리의 프로그램에는 프로그램 실행 시 최초 실행 시작 지점을 지정해주는 `start` language item이 필요합니다.

## `start` 속성

혹자는 프로그램 실행 시 언제나 `main` 함수가 가장 먼저 호출된다고 생각할지도 모릅니다. 대부분의 프로그래밍 언어들은 [런타임 시스템][runtime system]을 가지고 있는데, 이는 가비지 컬렉션 (예시: Java) 혹은 소프트웨어 스레드 (예시: GoLang의 goroutine) 등의 기능을 담당합니다.
이러한 런타임 시스템은 프로그램 실행 이전에 초기화 되어야 하기에 `main` 함수 호출 이전에 먼저 호출됩니다.

[runtime system]: https://en.wikipedia.org/wiki/Runtime_system

러스트 표준 라이브러리를 링크하는 전형적인 러스트 실행 파일의 경우, 프로그램 실행 시 C 런타임 라이브러리인 `crt0` (“C runtime zero”) 에서 실행이 시작됩니다. `crt0`는 C 프로그램의 환경을 설정하고 초기화하는 런타임 시스템으로, 스택을 만들고 프로그램에 주어진 인자들을 적절한 레지스터에 배치합니다. `crt0`가 작업을 마친 후 `start` language item으로 지정된 [Rust 런타임의 실행 시작 함수][rt::lang_start]를 호출합니다.
Rust는 최소한의 런타임 시스템을 가지며, 주요 기능은 스택 오버플로우 가드를 초기화하고 패닉 시 역추적 (backtrace) 정보를 출력하는 것입니다. Rust 런타임의 초기화 작업이 끝난 후에야 `main` 함수가 호출됩니다.

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

우리의 "freestanding 실행 파일" 은 Rust 런타임이나 `crt0`에 접근할 수 없기에, 우리가 직접 프로그램 실행 시작 지점을 지정해야 합니다.
`crt0`가 `start` language item을 호출해주는 방식으로 동작하기에, `start` language item을 구현하고 지정하는 것만으로는 문제를 해결할 수 없습니다.
대신 우리가 직접 `crt0`의 시작 지점을 대체할 새로운 실행 시작 지점을 제공해야 합니다.

### 실행 시작 지점 덮어쓰기
`#![no_main]` 속성을 이용해 Rust 컴파일러에게 우리가 일반적인 실행 시작 호출 단계를 이용하지 않겠다고 선언합니다.

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// 패닉이 일어날 경우, 이 함수가 호출됩니다.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

`main` 함수가 사라진 것을 눈치채셨나요? `main` 함수를 호출해주는 런타임 시스템이 없는 이상 `main` 함수의 존재도 더 이상 의미가 없습니다.
우리는 운영체제가 호출하는 프로그램 실행 시작 지점 대신 우리의 새로운 `_start` 함수를 실행 시작 지점으로 대체할 것입니다.

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

`#[no_mangle]` 속성을 통해 [name mangling]을 해제하여 Rust 컴파일러가 `_start` 라는 이름 그대로 함수를 만들도록 합니다. 이 속성이 없다면, 컴파일러가 각 함수의 이름을 고유하게 만드는 과정에서 이 함수의 실제 이름을 `_ZN3blog_os4_start7hb173fedf945531caE` 라는 이상한 이름으로 바꿔 생성합니다. 우리가 원하는 실제 시작 지점 함수의 이름을 정확히 알고 있어야 링커 (linker)에도 그 이름을 정확히 전달할 수 있기에 (후속 단계에서 진행) `#[no_mangle]` 속성이 필요합니다.

또한 우리는 이 함수에 `extern "C"`라는 표시를 추가하여 이 함수가 Rust 함수 호출 규약 대신에 [C 함수 호출 규약][C calling convention]을 사용하도록 합니다. 함수의 이름을 `_start`로 지정한 이유는 그저 런타임 시스템들의 실행 시작 함수 이름이 대부분 `_start`이기 때문입니다.

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[C calling convention]: https://en.wikipedia.org/wiki/Calling_convention

`!` 반환 타입은 이 함수가 발산 함수라는 것을 의미합니다. 시작 지점 함수는 오직 운영체제나 부트로더에 의해서만 직접 호출됩니다. 따라서 시작 지점 함수는 반환하는 대신 운영체제의 [`exit` 시스템콜][`exit` system call]을 이용해 종료됩니다. 우리의 "freestanding 실행 파일" 은 실행 종료 후 더 이상 실행할 작업이 없기에, 시작 지점 함수가 작업을 마친 후 기기를 종료하는 것이 합리적입니다. 여기서는 일단 `!` 타입의 조건을 만족시키기 위해 무한루프를 넣어 줍니다.

[`exit` system call]: https://en.wikipedia.org/wiki/Exit_(system_call)

다시 `cargo build`를 실행하면, 끔찍한 _링커_ 오류를 마주하게 됩니다.

## 링커 오류

링커는 컴파일러가 생성한 코드들을 묶어 실행파일로 만드는 프로그램입니다. 실행 파일 형식은 Linux, Windows, macOS 마다 전부 다르기에 각 운영체제는 자신만의 링커가 있고 링커마다 다른 오류 메세지를 출력할 것입니다.
오류가 나는 근본적인 원인은 모두 동일한데, 링커는 주어진 프로그램이 C 런타임 시스템을 이용할 것이라고 가정하는 반면 우리의 크레이트는 그렇지 않기 때문입니다.

이 링커 오류를 해결하려면 링커에게 C 런타임을 링크하지 말라고 알려줘야 합니다. 두 가지 방법이 있는데, 하나는 링커에 특정 인자들을 주는 것이고, 또다른 하나는 크레이트 컴파일 대상 기기를 bare metal 기기로 설정하는 것입니다.

### Bare Metal 시스템을 목표로 빌드하기

기본적으로 Rust는 당신의 현재 시스템 환경에서 실행할 수 있는 실행파일을 생성하고자 합니다. 예를 들어 Windows `x86_64` 사용자의 경우, Rust는 `x86_64` 명령어 셋을 사용하는 `.exe` 확장자 실행파일을 생성합니다. 사용자의 기본 시스템 환경을 "호스트" 시스템이라고 부릅니다.

여러 다른 시스템 환경들을 표현하기 위해 Rust는 [_target triple_]이라는 문자열을 이용합니다. 현재 호스트 시스템의 target triple이 궁금하시다면 `rustc --version --verbose` 명령어를 실행하여 확인 가능합니다.

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

위의 출력 내용은 `x86_64` Linux 시스템에서 얻은 것입니다. 호스트 target triple이 `x86_64-unknown-linux-gnu`으로 나오는데, 이는 CPU 아키텍쳐 정보 (`x86_64`)와 하드웨어 판매자 (`unknown`), 운영체제 (`linux`) 그리고 [응용 프로그램 이진 인터페이스 (ABI)][ABI] (`gnu`) 정보를 모두 담고 있습니다.

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

우리의 호스트 시스템 triple을 위해 컴파일하는 경우, Rust 컴파일러와 링커는 Linux나 Windows와 같은 운영체제가 있다고 가정하고 또한 운영체제가 C 런타임 시스템을 사용할 것이라고 가정하기 때문에 링커 오류 메세지가 출력된 것입니다. 이런 링커 오류를 피하려면 운영체제가 없는 시스템 환경에서 코드가 구동하는 것을 목표로 컴파일해야 합니다.

운영체제가 없는 bare metal 시스템 환경의 한 예시로 `thumbv7em-none-eabihf` target triple이 있습니다 (이는 [임베디드][embedded] [ARM] 시스템을 가리킵니다). Target triple의 `none`은 시스템에 운영체제가 동작하지 않음을 의미하며, 이 target triple의 나머지 부분의 의미는 아직 모르셔도 괜찮습니다. 이 시스템 환경에서 구동 가능하도록 컴파일하려면 rustup에서 해당 시스템 환경을 추가해야 합니다.

[embedded]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture

```
rustup target add thumbv7em-none-eabihf
```

위 명령어를 실행하면 해당 시스템을 위한 Rust 표준 라이브러리 및 코어 라이브러리를 설치합니다. 이제 해당 target triple을 목표로 하는 freestanding 실행파일을 만들 수 있습니다.

```
cargo build --target thumbv7em-none-eabihf
```

`--target` 인자를 통해 우리가 해당 bare metal 시스템을 목표로 [크로스 컴파일][cross compile]할 것이라는 것을 cargo에게 알려줍니다. 목표 시스템 환경에 운영체제가 없는 것을 링커도 알기 때문에 C 런타임을 링크하려고 시도하지 않으며 이제는 링커 에러 없이 빌드가 성공할 것입니다.

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

우리는 이 방법을 이용하여 우리의 운영체제 커널을 빌드해나갈 것입니다. 위에서 보인 `thumbv7em-none-eabihf` 시스템 환경 대신 bare metal `x86_64` 시스템 환경을 묘사하는 [커스텀 시스템 환경][custom target]을 설정하여 빌드할 것입니다. 더 자세한 내용은 다음 포스트에서 더 설명하겠습니다.

[custom target]: https://doc.rust-lang.org/rustc/targets/custom.html

### 링커 인자

Bare metal 시스템을 목표로 컴파일하는 대신, 링커에게 특정 인자들을 추가로 주어 링커 오류를 해결하는 방법도 있습니다.
이 방법은 앞으로 우리가 작성해나갈 커널 코드를 빌드할 때는 사용하지 않을 것이지만, 더 알고싶어 하실 분들을 위해서 이 섹션을 준비했습니다.
아래의 _"링커 인자"_ 텍스트를 눌러 이 섹션의 내용을 확인하세요.

<details>

<summary>링커 인자</summary>

이 섹션에서는 Linux, Windows 그리고 macOS 각각의 운영체제에서 나타나는 링커 오류에 대해 다루고 각 운영체제마다 링커에 어떤 추가 인자들을 주어 링커 오류를 해결할 수 있는지 설명할 것입니다.

#### Linux

Linux 에서는 아래와 같은 링커 오류 메세지가 출력됩니다 (일부 생략됨):

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

이 상황을 설명하자면 링커가 기본적으로 C 런타임의 실행 시작 루틴을 링크하는데, 이 루틴 역시 `_start`라는 이름을 가집니다. 이 `_start` 루틴은 C 표준 라이브러리 (`libc`)가 포함하는 여러 symbol들을 필요로 하지만, 우리는 `no_std` 속성을 이용해 크레이트에서 `libc`를 링크하지 않기 때문에 링커가 몇몇 symbol들의 출처를 찾지 못하여 위와 같은 링커 오류 메세지가 출력되는 것입니다. 이 문제를 해결하려면, 링커에게 `--nostartfiles` 인자를 전달하여 더 이상 링커가 C 런타임의 실행 시작 루틴을 링크하지 않도록 해야 합니다.

링커에 인자를 전달하는 한 방법은 `cargo rustc` 명령어를 이용하는 것입니다. 이 명령어는 `cargo build`와 유사하게 동작하나, `rustc`(Rust 컴파일러)에 직접 인자를 전달할 수 있게 해줍니다. `rustc`는 `-C link-arg` 인자를 통해 링커에게 인자를 전달할 수 있게 해줍니다. 우리가 이용할 새로운 빌드 명령어는 아래와 같습니다:

```
cargo rustc -- -C link-arg=-nostartfiles
```

이제 우리의 크레이트가 성공적으로 빌드되고 Linux에서 동작하는 freestanding 실행파일이 생성됩니다!

우리는 위의 빌드 명령어에서 실행 시작 함수의 이름을 명시적으로 전달하지 않았는데, 그 이유는 링커가 기본적으로 `_start` 라는 이름의 함수를 찾아 그 함수를 실행 시작 함수로 이용하기 때문입니다.

#### Windows

Windows에서는 다른 링커 오류를 마주하게 됩니다 (일부 생략):

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

오류 메세지 "entry point must be defined"는 링커가 실행 시작 지점을 찾을 수 없다는 것을 알려줍니다. Windows에서는 기본 실행 시작 지점의 이름이 [사용 중인 서브시스템(subsystem)에 따라 다릅니다][windows-subsystems]. `CONSOLE` 서브시스템의 경우 링커가 `mainCRTStartup`이라는 함수를 실행 시작 지점으로 간주하고, `WINDOWS` 서브시스템의 경우 링커가 `WinMainCRTStartup`이라는 이름의 함수를 실행 시작 지점으로 간주합니다. 이러한 기본값을 변경하여 링커가 `_start`라는 이름의 함수를 실행 시작 지점으로 간주하도록 만드려면 링커에 `/ENTRY` 인자를 넘겨주어야 합니다:

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

Linux에서와는 다른 인자 형식을 통해 Windows의 링커는 Linux의 링커와 완전히 다른 프로그램이라는 것을 유추할 수 있습니다.

이제 또 다른 링커 오류가 발생합니다:

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

이 오류가 뜨는 이유는 Windows 실행파일들은 여러 가지 [서브시스템][windows-subsystems]을 사용할 수 있기 때문입니다. 일반적인 프로그램들의 경우, 실행 시작 지점 함수의 이름에 따라 어떤 서브시스템을 사용하는지 추론합니다: 실행 시작 지점의 이름이 `main`인 경우 `CONSOLE` 서브시스템이 사용 중이라는 것을 알 수 있으며, 실행 시작 지점의 이름이 `WinMain`인 경우 `WINDOWS` 서브시스템이 사용 중이라는 것을 알 수 있습니다. 우리는 `_start`라는 새로운 이름의 실행 시작 지점을 이용할 것이기에, 우리가 어떤 서브시스템을 사용할 것인지 인자를 통해 명시적으로 링커에게 알려줘야 합니다:

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

위 명령어에서는 `CONSOLE` 서브시스템을 서용했지만, `WINDOWS` 서브시스템을 적용해도 괜찮습니다. `-C link-arg` 인자를 반복해서 쓰는 대신, `-C link-args`인자를 이용해 여러 인자들을 빈칸으로 구분하여 전달할 수 있습니다.

이 명령어를 통해 우리의 실행 파일을 Windows에서도 성공적으로 빌드할 수 있을 것입니다.

#### macOS

macOS에서는 아래와 같은 링커 오류가 출력됩니다 (일부 생략):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

위 오류 메세지는 우리에게 링커가 실행 시작 지점 함수의 기본값 이름 `main`을 찾지 못했다는 것을 알려줍니다 (macOS에서는 무슨 이유에서인지 모든 함수들의 이름 맨 앞에 `_` 문자가 앞에 붙습니다). 실행 시작 지점 함수의 이름을 `_start`로 새롭게 지정해주기 위해 아래와 같이 링커 인자 `-e`를 이용합니다:

```
cargo rustc -- -C link-args="-e __start"
```

`-e` 인자를 통해 실행 시작 지점 함수 이름을 설정합니다. macOS에서는 모든 함수의 이름 앞에 추가로 `_` 문자가 붙기에, 실행 시작 지점 함수의 이름을 `_start` 대신 `__start`로 지정해줍니다.

이제 아래와 같은 링커 오류가 나타날 것입니다:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS는 [공식적으로는 정적으로 링크된 실행파일을 지원하지 않으며][does not officially support statically linked binaries], 기본적으로 모든 프로그램이 `libSystem` 라이브러리를 링크하도록 요구합니다. 이러한 기본 요구사항을 무시하고 정적으로 링크된 실행 파일을 만드려면 링커에게 `-static` 인자를 주어야 합니다:

[does not officially support statically linked binaries]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

아직도 충분하지 않았는지, 세 번째 링커 오류가 아래와 같이 출력됩니다:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

이 오류가 뜨는 이유는 macOS에서 모든 프로그램은 기본적으로 `crt0` (“C runtime zero”)를 링크하기 때문입니다. 이 오류는 우리가 Linux에서 봤던 오류와 유사한 것으로, 똑같이 링커에 `-nostartfiles` 인자를 주어 해결할 수 있습니다:

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

이제는 우리의 프로그램을 macOS에서 성공적으로 빌드할 수 있을 것입니다.

#### 플랫폼 별 빌드 명령어들을 하나로 통합하기

위에서 살펴본 대로 호스트 플랫폼 별로 상이한 빌드 명령어가 필요한데, `.cargo/config.toml` 이라는 파일을 만들고 플랫폼 마다 필요한 상이한 인자들을 명시하여 여러 빌드 명령어들을 하나로 통합할 수 있습니다.

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

`rustflags`에 포함된 인자들은 `rustc`가 실행될 때마다 자동적으로 `rustc`에 인자로 전달됩니다. `.cargo/config.toml`에 대한 더 자세한 정보는 [공식 안내 문서](https://doc.rust-lang.org/cargo/reference/config.html)를 통해 확인해주세요.

이제 `cargo build` 명령어 만으로 세 가지 플랫폼 어디에서도 우리의 프로그램을 성공적으로 빌드할 수 있습니다.

#### 이렇게 하는 것이 괜찮나요?

Linux, Windows 또는 macOS 위에서 동작하는 freestanding 실행파일을 빌드하는 것이 가능하긴 해도 좋은 방법은 아닙니다. 운영체제가 갖춰진 환경을 목표로 빌드를 한다면, 실행 파일 동작 시 다른 많은 조건들이 런타임에 의해 제공될 것이라는 가정 하에 빌드가 이뤄지기 때문입니다 (예: 실행 파일이 `_start` 함수가 호출되는 시점에 이미 스택이 초기화되어있을 것이라고 간주하고 작동합니다). C 런타임 없이는 실행 파일이 필요로 하는 조건들이 갖춰지지 않아 결국 세그멘테이션 오류가 나는 등 프로그램이 제대로 실행되지 못할 수 있습니다.

이미 존재하는 운영체제 위에서 동작하는 최소한의 실행 파일을 만들고 싶다면, `libc`를 링크하고 [이 곳의 설명](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html)에 따라 `#[start]` 속성을 설정하는 것이 더 좋은 방법일 것입니다.

</details>

## 요약 {#summary}

아래와 같은 최소한의 코드로 "freestanding" Rust 실행파일을 만들 수 있습니다:

`src/main.rs`:

```rust
#![no_std] // Rust 표준 라이브러리를 링크하지 않도록 합니다
#![no_main] // Rust 언어에서 사용하는 실행 시작 지점 (main 함수)을 사용하지 않습니다

use core::panic::PanicInfo;

#[no_mangle] // 이 함수의 이름을 mangle하지 않습니다
pub extern "C" fn _start() -> ! {
    // 링커는 기본적으로 '_start' 라는 이름을 가진 함수를 실행 시작 지점으로 삼기에,
    // 이 함수는 실행 시작 지점이 됩니다
    loop {}
}

/// 패닉이 일어날 경우, 이 함수가 호출됩니다.
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

# `cargo build` 실행 시 이용되는 빌드 설정
[profile.dev]
panic = "abort" # 패닉 시 스택 되감기를 하지 않고 바로 프로그램 종료

# `cargo build --release` 실행 시 이용되는 빌드 설정
[profile.release]
panic = "abort" # 패닉 시 스택 되감기를 하지 않고 바로 프로그램 종료
```

이 실행 파일을 빌드하려면, `thumbv7em-none-eabihf`와 같은 bare metal 시스템 환경을 목표로 컴파일해야 합니다:

```
cargo build --target thumbv7em-none-eabihf
```

또다른 방법으로, 각 호스트 시스템마다 추가적인 링커 인자들을 전달해주어 호스트 시스템 환경을 목표로 컴파일할 수도 있습니다:

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

주의할 것은 이것이 정말 최소한의 freestanding Rust 실행 파일이라는 것입니다. 실행 파일은 여러 가지 조건들을 가정하는데, 그 예로 실행파일 동작 시 `_start` 함수가 호출될 때 스택이 초기화되어 있을 것을 가정합니다. **이 freestanding 실행 파일을 이용해 실제로 유용한 작업을 처리하려면 아직 더 많은 코드 구현이 필요합니다**.

## 다음 단계는 무엇일까요?

[다음 포스트][next post]에서는 우리의 freestanding 실행 파일을 최소한의 기능을 갖춘 운영체제 커널로 만드는 과정을 단게별로 설명할 것입니다.
예시로 커스텀 시스템 환경을 설정하는 방법, 우리의 실행 파일을 부트로더와 합치는 방법, 그리고 화면에 메세지를 출력하는 방법 등에 대해 다루겠습니다.

[next post]: @/edition-2/posts/02-minimal-rust-kernel/index.md
