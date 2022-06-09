+++
title = "최소 기능을 갖춘 커널"
weight = 2
path = "ko/minimal-rust-kernel"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "c1af4e31b14e562826029999b9ab1dce86396b93"
# GitHub usernames of the people that translated this post
translators = ["JOE1994", "Quqqu"]
+++

이번 포스트에서는 x86 아키텍처에서 최소한의 기능으로 동작하는 64비트 Rust 커널을 함께 만들 것입니다. 지난 포스트 [Rust로 'Freestanding 실행파일' 만들기][freestanding Rust binary] 에서 작업한 것을 토대로 부팅 가능한 디스크 이미지를 만들고 화면에 데이터를 출력해볼 것입니다.

[freestanding Rust binary]: @/edition-2/posts/01-freestanding-rust-binary/index.md

<!-- more -->

이 블로그는 [GitHub 저장소][GitHub]에서 오픈 소스로 개발되고 있으니, 문제나 문의사항이 있다면 저장소의 'Issue' 기능을 이용해 제보해주세요. [페이지 맨 아래][at the bottom]에 댓글을 남기실 수도 있습니다. 이 포스트와 관련된 모든 소스 코드는 저장소의 [`post-02 브랜치`][post branch]에서 확인하실 수 있습니다.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-02

<!-- toc -->

## 부팅 과정 {#the-boot-process}

전원이 켜졌을 때 컴퓨터가 맨 처음 하는 일은 바로 마더보드의 [롬 (ROM)][ROM]에 저장된 펌웨어 코드를 실행하는 것입니다.
이 코드는 [시동 자체 시험][power-on self-test]을 진행하고, 사용 가능한 램 (RAM)을 확인하며, CPU 및 하드웨어의 초기화 작업을 진행합니다.
그 후에는 부팅 가능한 디스크를 감지하고 운영체제 커널을 부팅하기 시작합니다.

[ROM]: https://en.wikipedia.org/wiki/Read-only_memory
[power-on self-test]: https://en.wikipedia.org/wiki/Power-on_self-test

x86 시스템에는 두 가지 펌웨어 표준이 존재합니다: 하나는 "Basic Input/Output System"(**[BIOS]**)이고 다른 하나는 "Unified Extensible Firmware Interface" (**[UEFI]**) 입니다. BIOS 표준은 구식 표준이지만, 간단하며 1980년대 이후 출시된 어떤 x86 하드웨어에서도 지원이 잘 됩니다. UEFI는 신식 표준으로서 더 많은 기능들을 갖추었지만, 제대로 설정하고 구동시키기까지의 과정이 더 복잡합니다 (적어도 제 주관적 입장에서는 그렇게 생각합니다).

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[UEFI]: https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

우리가 만들 운영체제에서는 BIOS 표준만을 지원할 것이지만, UEFI 표준도 지원하고자 하는 계획이 있습니다. UEFI 표준을 지원할 수 있도록 도와주시고 싶다면 [해당 깃헙 이슈][Github issue]를 확인해주세요.

### BIOS 부팅

UEFI 표준으로 동작하는 최신 기기들도 가상 BIOS를 지원하기에, 존재하는 거의 모든 x86 시스템들이 BIOS 부팅을 지원합니다. 덕분에 하나의 BIOS 부팅 로직을 구현하면 여태 만들어진 거의 모든 컴퓨터를 부팅시킬 수 있습니다. 동시에 이 방대한 호환성이 BIOS의 가장 큰 약점이기도 한데,
그 이유는 1980년대의 구식 부트로더들에 대한 하위 호환성을 유지하기 위해 부팅 전에는 항상 CPU를 16비트 호환 모드 ([real mode]라고도 불림)로 설정해야 하기 때문입니다.

이제 BIOS 부팅 과정의 첫 단계부터 살펴보겠습니다:

여러분이 컴퓨터의 전원을 켜면, 제일 먼저 컴퓨터는 마더보드의 특별한 플래시 메모리로부터 BIOS 이미지를 로드합니다. BIOS 이미지는 자가 점검 및 하드웨어 초기화 작업을 처리한 후에 부팅 가능한 디스크가 있는지 탐색합니다. 부팅 가능한 디스크가 있다면, 제어 흐름은 해당 디스크의 _부트로더 (bootloader)_ 에게 넘겨집니다. 이 부트로더는 디스크의 가장 앞 주소 영역에 저장되는 512 바이트 크기의 실행 파일입니다. 대부분의 부트로더들의 경우 로직을 저장하는 데에 512 바이트보다 더 큰 용량이 필요하기에, 부트로더의 로직을 둘로 쪼개어 첫 단계 로직을 첫 512 바이트 안에 담고, 두 번째 단계 로직은 첫 단계 로직에 의해 로드된 이후 실행됩니다.

부트로더는 커널 이미지가 디스크의 어느 주소에 저장되어있는지 알아낸 후 메모리에 커널 이미지를 로드해야 합니다. 그다음 CPU를 16비트 [real mode]에서 32비트 [protected mode]로 전환하고, 그 후에 다시 CPU를 64비트 [long mode]로 전환한 이후부터 64비트 레지스터 및 메인 메모리의 모든 주소를 사용할 수 있게 됩니다. 부트로더가 세 번째로 할 일은 BIOS로부터 메모리 매핑 정보 등의 필요한 정보를 알아내어 운영체제 커널에 전달하는 것입니다.

[real mode]: https://en.wikipedia.org/wiki/Real_mode
[protected mode]: https://en.wikipedia.org/wiki/Protected_mode
[long mode]: https://en.wikipedia.org/wiki/Long_mode
[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation

부트로더를 작성하는 것은 상당히 성가신 작업인데, 그 이유는 어셈블리 코드도 작성해야 하고 "A 레지스터에 B 값을 저장하세요" 와 같이 원리를 단번에 이해하기 힘든 작업이 많이 수반되기 때문입니다. 따라서 이 포스트에서는 부트로더를 만드는 것 자체를 다루지는 않고, 대신 운영체제 커널의 맨 앞에 부트로더를 자동으로 추가해주는 [bootimage]라는 도구를 제공합니다.

[bootimage]: https://github.com/rust-osdev/bootimage

본인의 부트로더를 직접 작성하는 것에 흥미가 있으시다면, 이 주제로 여러 포스트가 나올 계획이니 기대해주세요!

#### Multiboot 표준

운영체제마다 부트로더 구현 방법이 다르다면 한 운영체제에서 동작하는 부트로더가 다른 운영체제에서는 호환이 되지 않을 것입니다. 이런 불편한 점을 막기 위해 [Free Software Foundation]에서 1995년에 [Multiboot]라는 부트로더 표준을 개발했습니다. 이 표준은 부트로더와 운영체제 사이의 상호 작용 방식을 정의하였는데, 이 Multiboot 표준에 따르는 부트로더는 Multiboot 표준을 지원하는 어떤 운영체제에서도 동작합니다. 이 표준을 구현한 대표적인 예로 리눅스 시스템에서 가장 인기 있는 부트로더인 [GNU GRUB]이 있습니다.

[Free Software Foundation]: https://en.wikipedia.org/wiki/Free_Software_Foundation
[Multiboot]: https://wiki.osdev.org/Multiboot
[GNU GRUB]: https://en.wikipedia.org/wiki/GNU_GRUB

운영체제 커널이 Multiboot를 지원하게 하려면 커널 파일의 맨 앞에 [Multiboot 헤더][Multiboot header]를 삽입해주면 됩니다. 이렇게 하면 GRUB에서 운영체제를 부팅하는 것이 매우 쉬워집니다. 하지만 GRUB 및 Multiboot 표준도 몇 가지 문제점들을 안고 있습니다:

[Multiboot header]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format

- 오직 32비트 protected mode만을 지원합니다. 64비트 long mode를 이용하고 싶다면 CPU 설정을 별도로 변경해주어야 합니다.
- Multiboot 표준 및 GRUB은 부트로더 구현의 단순화를 우선시하여 개발되었기에, 이에 호응하는 커널 측의 구현이 번거로워진다는 단점이 있습니다. 예를 들어, GRUB이 Multiboot 헤더를 제대로 찾을 수 있으려면 커널 측에서 [조정된 기본 페이지 크기 (adjusted default page size)][adjusted default page size]를 링크하는 것이 강제됩니다. 또한, 부트로더가 커널로 전달하는 [부팅 정보][boot information]는 적절한 추상 레벨에서 표준화된 형태로 전달되는 대신 하드웨어 아키텍처마다 상이한 형태로 제공됩니다.
- GRUB 및 Multiboot 표준에 대한 문서화 작업이 덜 되어 있습니다.
- GRUB이 호스트 시스템에 설치되어 있어야만 커널 파일로부터 부팅 가능한 디스크 이미지를 만들 수 있습니다. 이 때문에 Windows 및 Mac에서는 부트로더를 개발하는 것이 Linux보다 어렵습니다.

[adjusted default page size]: https://wiki.osdev.org/Multiboot#Multiboot_2
[boot information]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format

이러한 단점들 때문에 우리는 GRUB 및 Multiboot 표준을 사용하지 않을 것입니다. 하지만 미래에 우리의 [bootimage] 도구가 Multiboot 표준을 지원하도록 하는 것도 계획 중입니다. Multiboot 표준을 지원하는 운영체제를 커널을 개발하는 것에 관심이 있으시다면, 이 블로그 시리즈의 [첫 번째 에디션][first edition]을 확인해주세요.

[first edition]: @/edition-1/_index.md

### UEFI

(아직 UEFI 표준을 지원하지 않지만, UEFI 표준을 지원할 수 있도록 도와주시려면 해당 [깃헙 이슈](https://github.com/phil-opp/blog_os/issues/349)에 댓글을 남겨주세요!)

## 최소한의 기능을 갖춘 운영체제 커널
컴퓨터의 부팅 과정에 대해서 대략적으로 알게 되었으니, 이제 우리 스스로 최소한의 기능을 갖춘 운영체제 커널을 작성해볼 차례입니다. 우리의 목표는 부팅 이후 화면에 "Hello World!" 라는 메세지를 출력하는 디스크 이미지를 만드는 것입니다. 지난 포스트에서 만든 [freestanding Rust 실행파일][freestanding Rust binary] 을 토대로 작업을 이어나갑시다.

지난 포스트에서 우리는 `cargo`를 통해 freestanding 실행파일을 만들었었는데, 호스트 시스템의 운영체제에 따라 프로그램 실행 시작 지점의 이름 및 컴파일 인자들을 다르게 설정해야 했습니다. 이것은 `cargo`가 기본적으로 _호스트 시스템_ (여러 분이 실행 중인 컴퓨터 시스템) 을 목표로 빌드하기 때문이었습니다. 우리의 커널은 다른 운영체제 (예를 들어 Windows) 위에서 실행될 것이 아니기에, 호스트 시스템에 설정 값을 맞추는 대신에 우리가 명확히 정의한 _목표 시스템 (target system)_ 을 목표로 컴파일할 것입니다.

### Rust Nightly 설치하기 {#installing-rust-nightly}
Rust는 _stable_, _beta_ 그리고 _nightly_ 이렇게 세 가지의 채널을 통해 배포됩니다. Rust Book에 [세 채널들 간의 차이에 대해 잘 정리한 챕터]((https://doc.rust-lang.org/book/appendix-07-nightly-rust.html#choo-choo-release-channels-and-riding-the-trains))가 있습니다. 운영체제를 빌드하기 위해서는 _nightly_ 채널에서만 제공하는 실험적인 기능들을 이용해야 하기에 _nightly_ 버전의 Rust를 설치하셔야 합니다.

여러 버전의 Rust 언어 설치 파일들을 관리할 때 [rustup]을 사용하는 것을 강력 추천합니다. rustup을 통해 nightly, beta 그리고 stable 컴파일러들을 모두 설치하고 업데이트할 수 있습니다. `rustup override set nightly` 명령어를 통해 현재 디렉토리에서 항상 nightly 버전의 Rust를 사용하도록 설정할 수 있습니다.
`rust-toolchain`이라는 파일을 프로젝트 루트 디렉토리에 만들고 이 파일에 `nightly`라는 텍스트를 적어 놓아도 같은 효과를 볼 수 있습니다. `rustc --version` 명령어를 통해 현재 nightly 버전이 설치되어 있는지 확인할 수 있습니다 (출력되는 버전 넘버가 `-nightly`라는 텍스트로 끝나야 합니다).

[rustup]: https://www.rustup.rs/

nightly 컴파일러는 _feature 플래그_ 를 소스코드의 맨 위에 추가함으로써 여러 실험적인 기능들을 선별해 이용할 수 있게 해줍니다. 예를 들어, `#![feature(asm)]` 를 `main.rs`의 맨 위에 추가하면 [`asm!` 매크로][`asm!` macro]를 사용할 수 있습니다. `asm!` 매크로는 인라인 어셈블리 코드를 작성할 때 사용합니다.
이런 실험적인 기능들은 말 그대로 "실험적인" 기능들이기에 미래의 Rust 버전들에서는 예고 없이 변경되거나 삭제될 수도 있습니다. 그렇기에 우리는 이 실험적인 기능들을 최소한으로만 사용할 것입니다.

[`asm!` macro]: https://doc.rust-lang.org/stable/reference/inline-assembly.html

### 컴파일 대상 정의하기
Cargo는 `--target` 인자를 통해 여러 컴파일 대상 시스템들을 지원합니다. 컴파일 대상은 소위 _[target triple]_ 을 통해 표현되는데, CPU 아키텍쳐와 CPU 공급 업체, 운영체제, 그리고 [ABI]를 파악할 수 있습니다. 예를 들어 `x86_64-unknown-linux-gnu`는 `x86_64` CPU, 임의의 CPU 공급 업체, Linux 운영체제, 그리고 GNU ABI를 갖춘 시스템을 나타냅니다. Rust는 Android를 위한 `arm-linux-androideabi`와 [WebAssembly를 위한 `wasm32-unknown-unknown`](https://www.hellorust.com/setup/wasm-target/)를 비롯해 [다양한 target triple들][platform-support]을 지원합니다.

[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[ABI]: https://stackoverflow.com/a/2456882
[platform-support]: https://forge.rust-lang.org/release/platform-support.html
[custom-targets]: https://doc.rust-lang.org/nightly/rustc/targets/custom.html

우리가 목표로 하는 컴파일 대상 환경 (운영체제가 따로 없는 환경)을 정의하려면 몇 가지 특별한 설정 인자들을 사용해야 하기에 [Rust 에서 기본적으로 지원하는 target triple][platform-support] 중에서는 우리가 쓸 수 있는 것은 없습니다. 다행히도 Rust에서는 JSON 파일을 이용해 [우리가 목표로 하는 컴파일 대상 환경][custom-targets]을 직접 정의할 수 있습니다. 예를 들어, `x86_64-unknown-linux-gnu` 환경을 직접 정의하는 JSON 파일의 내용은 아래와 같습니다:

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

대부분의 필드 값들은 LLVM이 해당 환경을 목표로 코드를 생성하는 과정에서 필요합니다. 예시로, [`data-layout`] 필드는 다양한 정수, 부동소수점 표기 소수, 포인터 등의 메모리 상 실제 크기를 지정합니다. 또한 `target-pointer-width`와 같이 Rust가 조건부 컴파일을 하는 과정에서 이용하는 필드들도 있습니다.
마지막 남은 종류의 필드들은 crate가 어떻게 빌드되어야 하는지 결정합니다. 예를 들어 `pre-link-args` 필드는 [링커][linker]에 전달될 인자들을 설정합니다.

[`data-layout`]: https://llvm.org/docs/LangRef.html#data-layout
[linker]: https://en.wikipedia.org/wiki/Linker_(computing)

우리도 `x86_64` 시스템에서 구동할 운영체제 커널을 작성할 것이기에, 우리가 사용할 컴파일 대상 환경 환경 설정 파일 (JSON 파일) 또한 위의 내용과 많이 유사할 것입니다. 일단 `x86_64-blog_os.json`이라는 파일을 만들고 아래와 같이 파일 내용을 작성해주세요:

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

우리의 운영체제는 bare metal 환경에서 동작할 것이기에, `llvm-target` 필드의 운영체제 값과 `os` 필드의 값은 `none`입니다.

아래의 빌드 관련 설정들을 추가해줍니다:


```json
"linker-flavor": "ld.lld",
"linker": "rust-lld",
```

현재 사용 중인 플랫폼의 기본 링커 대신 Rust와 함께 배포되는 크로스 플랫폼 [LLD] 링커를 사용해 커널을 링크합니다 (기본 링커는 리눅스 환경을 지원하지 않을 수 있습니다).

[LLD]: https://lld.llvm.org/

```json
"panic-strategy": "abort",
```

해당 환경이 패닉 시 [스택 되감기][stack unwinding]을 지원하지 않기에, 위 설정을 통해 패닉 시 프로그램이 즉시 실행 종료되도록 합니다. 위 설정은 Cargo.toml 파일에 `panic = "abort"` 설정을 추가하는 것과 비슷한 효과이기에, Cargo.toml에서는 해당 설정을 지우셔도 괜찮습니다 (다만, Cargo.toml에서의 설정과는 달리 이 설정은 이후 단계에서 우리가 `core` 라이브러리를 재컴파일할 때에도 유효하게 적용된다는 점이 중요합니다. 위 설정은 꼭 추가해주세요!).

[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php

```json
"disable-redzone": true,
```

커널을 작성하려면, 커널이 인터럽트에 대해 어떻게 대응하는지에 대한 로직도 작성하게 될 것입니다. 안전하게 이런 로직을 작성하기 위해서는 _“red zone”_ 이라고 불리는 스택 포인터 최적화 기능을 해제해야 합니다 (그렇지 않으면 해당 기능으로 인해 스택 메모리가 우리가 원치 않는 값으로 덮어쓰일 수 있습니다). 이 내용에 대해 더 자세히 알고 싶으시면 [red zone 기능 해제][disabling the red zone] 포스트를 확인해주세요.

[disabling the red zone]: @/edition-2/posts/02-minimal-rust-kernel/disable-red-zone/index.ko.md

```json
"features": "-mmx,-sse,+soft-float",
```

`features` 필드는 컴파일 대상 환경의 기능들을 활성화/비활성화 하는 데 이용합니다. 우리는 `-` 기호를 통해 `mmx`와 `sse` 기능들을 비활성화시키고 `+` 기호를 통해 `soft-float` 기능을 활성화시킬 것입니다. `features` 필드의 문자열 내부 플래그들 사이에 빈칸이 없도록 해야 합니다. 그렇지 않으면 LLVM이 `features` 필드의 문자열 값을 제대로 해석하지 못하기 때문입니다.

`mmx`와 `sse`는 [Single Instruction Multiple Data (SIMD)] 명령어들의 사용 여부를 결정하는데, 해당 명령어들은 프로그램의 실행 속도를 훨씬 빠르게 만드는 데에 도움을 줄 수 있습니다. 하지만 운영체제에서 큰 SIMD 레지스터를 사용할 경우 커널의 성능에 문제가 생길 수 있습니다. 그 이유는 커널이 인터럽트 되었던 프로그램을 다시 실행하기 전에 모든 레지스터 값들을 인터럽트 직전 시점의 상태로 복원시켜야 하기 때문입니다. 커널이 SIMD 레지스터를 사용하려면 각 시스템 콜 및 하드웨어 인터럽트가 일어날 때마다 모든 SIMD 레지스터에 저장된 값들을 메인 메모리에 저장해야 할 것입니다. SIMD 레지스터들이 총 차지하는 용량은 매우 크고 (512-1600 바이트) 인터럽트 또한 자주 일어날 수 있기에,
SIMD 레지스터 값들을 메모리에 백업하고 또 다시 복구하는 과정은 커널의 성능을 심각하게 해칠 수 있습니다. 이를 피하기 위해 커널이 SIMD 명령어를 사용하지 않도록 설정합니다 (물론 우리의 커널 위에서 구동할 프로그램들은 SIMD 명령어들을 사용할 수 있습니다!).

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

`x86_64` 환경에서 SIMD 기능을 비활성화하는 것에는 걸림돌이 하나 있는데, 그것은 바로 `x86_64` 환경에서 부동소수점 계산 시 기본적으로 SIMD 레지스터가 사용된다는 것입니다. 이 문제를 해결하기 위해 `soft-float` 기능 (일반 정수 계산만을 이용해 부동소수점 계산을 소프트웨어 단에서 모방)을 활성화시킵니다.

더 자세히 알고 싶으시다면, 저희가 작성한 [SIMD 기능 해제](@/edition-2/posts/02-minimal-rust-kernel/disable-simd/index.ko.md)에 관한 포스트를 확인해주세요.

#### 요약
컴파일 대상 환경 설정 파일을 아래와 같이 작성합니다:

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

### 커널 빌드하기
우리가 정의한 새로운 컴파일 대상 환경을 목표로 컴파일할 때에 리눅스 시스템의 관례를 따를 것입니다 (LLVM이 기본적으로 리눅스 시스템 관례를 따르기에 그렇습니다). 즉, [지난 포스트][previous post]에서 설명한 것처럼 우리는 실행 시작 지점의 이름을 `_start`로 지정할 것입니다:

[previous post]: @/edition-2/posts/01-freestanding-rust-binary/index.md

```rust
// src/main.rs

#![no_std] // Rust 표준 라이브러리를 링크하지 않도록 합니다
#![no_main] // Rust 언어에서 사용하는 실행 시작 지점 (main 함수)을 사용하지 않습니다

use core::panic::PanicInfo;

/// 패닉이 일어날 경우, 이 함수가 호출됩니다.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle] // 이 함수의 이름을 mangle하지 않습니다
pub extern "C" fn _start() -> ! {
    // 링커는 기본적으로 '_start' 라는 이름을 가진 함수를 실행 시작 지점으로 삼기에,
    // 이 함수는 실행 시작 지점이 됩니다
    loop {}
}
```

호스트 운영체제에 관계 없이 실행 시작 지점 함수의 이름은 `_start`로 지정해야 함을 기억해주세요.

이제 `--target` 인자를 통해 위에서 다룬 JSON 파일의 이름을 전달하여 우리가 정의한 새로운 컴파일 대상 환경을 목표로 커널을 빌드할 수 있습니다:

```
> cargo build --target x86_64-blog_os.json

error[E0463]: can't find crate for `core`
```

실패하였군요! 이 오류는 Rust 컴파일러가 더 이상 [`core` 라이브러리][`core` library]를 찾지 못한다는 것을 알려줍니다. 이 라이브러리는 `Result`와 `Option` 그리고 반복자 등 Rust의 기본적인 타입들을 포함하며, 모든 `no_std` 크레이트에 암시적으로 링크됩니다. 

[`core` library]: https://doc.rust-lang.org/nightly/core/index.html

문제는 core 라이브러리가 _미리 컴파일된 상태_ 의 라이브러리로 Rust 컴파일러와 함께 배포된다는 것입니다. `x86_64-unknown-linux-gnu` 등 배포된 라이브러리가 지원하는 컴파일 목표 환경을 위해 빌드하는 경우 문제가 없지만, 우리가 정의한 커스텀 환경을 위해 빌드하는 경우에는 라이브러리를 이용할 수 없습니다. 기본적으로 지원되지 않는 새로운 시스템 환경을 위해 코드를 빌드하기 위해서는 새로운 시스템 환경에서 구동 가능하도록 `core` 라이브러리를 새롭게 빌드해야 합니다.

#### `build-std` 기능

이제 cargo의 [`build-std 기능`][`build-std` feature]이 필요한 시점이 왔습니다. Rust 언어 설치파일에 함께 배포된 `core` 및 다른 표준 라이브러리 크레이트 버전을 사용하는 대신, 이 기능을 이용하여 해당 크레이트들을 직접 재컴파일하여 사용할 수 있습니다. 이 기능은 아직 비교적 새로운 기능이며 아직 완성된 기능이 아니기에, "unstable" 한 기능으로 표기되며 [nightly 버전의 Rust 컴파일러][nightly Rust compilers]에서만 이용가능합니다.

[`build-std` feature]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std
[nightly Rust compilers]: #installing-rust-nightly

해당 기능을 사용하려면, [cargo 설정][cargo configuration] 파일 `.cargo/config.toml`을 아래와 같이 만들어야 합니다:

```toml
# .cargo/config.toml 에 들어갈 내용

[unstable]
build-std = ["core", "compiler_builtins"]
```

위 설정은 cargo에게 `core`와 `compiler_builtins` 라이브러리를 새로 컴파일하도록 지시합니다. `compiler_builtins`는 `core`가 사용하는 라이브러리입니다. 해당 라이브러리들의 소스 코드가 있어야 새로 컴파일할 수 있기에, `rustup component add rust-src` 명령어를 통해 소스 코드를 설치합니다.

<div class="note">

**주의:** `unstable.build-std` 설정 키를 이용하려면 2020-07-15 혹은 그 이후에 출시된 Rust nightly 버전을 사용하셔야 합니다.

</div>

cargo 설정 키 `unstable.build-std`를 설정하고 `rust-src` 컴포넌트를 설치한 후에 다시 빌드 명령어를 실행합니다:

```
> cargo build --target x86_64-blog_os.json
   Compiling core v0.0.0 (/…/rust/src/libcore)
   Compiling rustc-std-workspace-core v1.99.0 (/…/rust/src/tools/rustc-std-workspace-core)
   Compiling compiler_builtins v0.1.32
   Compiling blog_os v0.1.0 (/…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

이제 `cargo build` 명령어가 `core`, `rustc-std-workspace-cord` (`compiler_builtins`가 필요로 하는 라이브러리) 그리고 `compiler_builtins` 라이브러리를 우리의 커스텀 컴파일 대상을 위해 다시 컴파일하는 것을 확인할 수 있습니다.

#### 메모리 관련 내장 함수

Rust 컴파일러는 특정 군의 내장 함수들이 (built-in function) 모든 시스템에서 주어진다고 가정합니다. 대부분의 내장 함수들은 우리가 방금 컴파일한 `compiler_builtins` 크레이트가 이미 갖추고 있습니다. 하지만 그중 몇몇 메모리 관련 함수들은 기본적으로 사용 해제 상태가 되어 있는데, 그 이유는 해당 함수들을 호스트 시스템의 C 라이브러리가 제공하는 것이 관례이기 때문입니다. `memset`(메모리 블럭 전체에 특정 값 저장하기), `memcpy` (한 메모리 블럭의 데이터를 다른 메모리 블럭에 옮겨쓰기), `memcmp` (메모리 블럭 두 개의 데이터를 비교하기) 등이 이 분류에 해당합니다. 여태까지는 우리가 이 함수들 중 어느 하나도 사용하지 않았지만, 운영체제 구현을 더 추가하다 보면 필수적으로 사용될 함수들입니다 (예를 들어, 구조체를 복사하여 다른 곳에 저장할 때).

우리는 운영체제의 C 라이브러리를 링크할 수 없기에, 다른 방식으로 이러한 내장 함수들을 컴파일러에 전달해야 합니다. 한 방법은 우리가 직접 `memset` 등의 내장함수들을 구현하고 컴파일 과정에서 함수명이 바뀌지 않도록 `#[no_mangle]` 속성을 적용하는 것입니다. 하지만 이 방법의 경우 우리가 직접 구현한 함수 로직에 아주 작은 실수만 있어도 undefined behavior를 일으킬 수 있기에 위험합니다. 예를 들어 `memcpy`를 구현하는 데에 `for`문을 사용한다면 무한 재귀 루프가 발생할 수 있는데, 그 이유는 `for`문의 구현이 내부적으로 trait 함수인 [`IntoIterator::into_iter`]를 호출하고 이 함수가 다시 `memcpy` 를 호출할 수 있기 때문입니다. 그렇기에 충분히 검증된 기존의 구현 중 하나를 사용하는 것이 바람직합니다.

[`IntoIterator::into_iter`]: https://doc.rust-lang.org/stable/core/iter/trait.IntoIterator.html#tymethod.into_iter

다행히도 `compiler_builtins` 크레이트가 이미 필요한 내장함수 구현을 전부 갖추고 있으며, C 라이브러리에서 오는 내장함수 구현과 충돌하지 않도록 사용 해제되어 있었던 것 뿐입니다. cargo의 [`build-std-features`] 플래그를 `["compiler-builtins-mem"]`으로 설정함으로써 `compiler_builtins`에 포함된 내장함수 구현을 사용할 수 있습니다. `build-std` 플래그와 유사하게 이 플래그 역시 커맨드 라인에서 `-Z` 플래그를 이용해 인자로 전달하거나 `.cargo/config.toml`의 `[unstable]` 테이블에서 설정할 수 있습니다. 우리는 매번 이 플래그를 사용하여 빌드할 예정이기에 `.cargo/config.toml`을 통해 설정을 하는 것이 장기적으로 더 편리할 것입니다:

[`build-std-features`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features

```toml
# .cargo/config.toml 에 들어갈 내용

[unstable]
build-std-features = ["compiler-builtins-mem"]
build-std = ["core", "compiler_builtins"]
```

(`compiler-builtins-mem` 기능에 대한 지원이 [굉장히 최근에 추가되었기에](https://github.com/rust-lang/rust/pull/77284), Rust nightly `2020-09-30` 이상의 버전을 사용하셔야 합니다.)

이 기능은 `compiler_builtins` 크레이트의 [`mem` 기능 (feature)][`mem` feature]를 활성화 시킵니다. 이는 `#[no_mangle]` 속성이 [`memcpy` 등의 함수 구현][`memcpy` etc. implementations]에 적용되게 하여 링크가 해당 함수들을 식별하고 사용할 수 있게 합니다.

[`mem` feature]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/Cargo.toml#L54-L55
[`memcpy` etc. implementations]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/src/mem.rs#L12-L69

이제 우리의 커널은 컴파일러가 요구하는 함수들에 대한 유효한 구현을 모두 갖추게 되었기에, 커널 코드가 더 복잡해지더라도 상관 없이 컴파일하는 데에 문제가 없을 것입니다.

#### 기본 컴파일 대상 환경 설정하기

기본 컴파일 대상 환경을 지정하여 설정해놓으면 `cargo build` 명령어를 실행할 때마다 `--target` 인자를 넘기지 않아도 됩니다. [cargo 설정][cargo configuration] 파일인 `.cargo/config.toml`에 아래의 내용을 추가해주세요:

[cargo configuration]: https://doc.rust-lang.org/cargo/reference/config.html

```toml
# .cargo/config.toml 에 들어갈 내용

[build]
target = "x86_64-blog_os.json"
```

이로써 `cargo`는 명시적으로 `--target` 인자가 주어지지 않으면 `x86_64-blog_os.json`에 명시된 컴파일 대상 환경을 기본 값으로 이용합니다. `cargo build` 만으로 간단히 커널을 빌드할 수 있게 되었습니다. cargo 설정 옵션들에 대해 더 자세한 정보를 원하시면 [공식 문서][cargo configuration]을 확인해주세요.

`cargo build`만으로 이제 bare metal 환경을 목표로 커널을 빌드할 수 있지만, 아직 실행 시작 지점 함수 `_start`는 텅 비어 있습니다.
이제 이 함수에 코드를 추가하여 화면에 메세지를 출력해볼 것입니다.

### 화면에 출력하기
현재 단계에서 가장 쉽게 화면에 문자를 출력할 수 있는 방법은 바로 [VGA 텍스트 버퍼][VGA text buffer]를 이용하는 것입니다. 이것은 VGA 하드웨어에 매핑되는 특수한 메모리 영역이며 화면에 출력될 내용이 저장됩니다. 주로 이 버퍼는 주로 25행 80열 (행마다 80개의 문자 저장)로 구성됩니다. 각 문자는 ASCII 문자로서 전경색 혹은 배경색과 함께 화면에 출력됩니다. 화면 출력 결과의 모습은 아래와 같습니다:

[VGA text buffer]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode

![ASCII 문자들을 출력한 화면의 모습](https://upload.wikimedia.org/wikipedia/commons/f/f8/Codepage-437.png)

VGA 버퍼가 정확히 어떤 구조를 하고 있는지는 다음 포스트에서 VGA 버퍼 드라이버를 작성하면서 다룰 것입니다. "Hello World!" 메시지를 출력하는 데에는 그저 버퍼의 시작 주소가 `0xb8000`이라는 것, 그리고 각 문자는 ASCII 문자를 위한 1바이트와 색상 표기를 위한 1바이트가 필요하다는 것만 알면 충분합니다.

코드 구현은 아래와 같습니다:

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

우선 정수 `0xb8000`을 [raw 포인터][raw pointer]로 형변환 합니다. 그 다음 [static (정적 변수)][static] [바이트 문자열][byte string] `HELLO`의 반복자를 통해 각 바이트를 읽고, [`enumerate`] 함수를 통해 각 바이트의 문자열 내에서의 인덱스 값 `i`를 얻습니다. for문의 내부에서는 [`offset`] 함수를 통해 VGA 버퍼에 문자열의 각 바이트 및 색상 코드를 저장합니다 (`0xb`: light cyan 색상 코드).

[iterate]: https://doc.rust-lang.org/stable/book/ch13-02-iterators.html
[static]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate
[byte string]: https://doc.rust-lang.org/reference/tokens.html#byte-string-literals
[raw pointer]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

메모리 쓰기 작업을 위한 코드 주변에 [`unsafe`] 블록이 있는 것에 주목해주세요. 여기서 `unsafe` 블록이 필요한 이유는 Rust 컴파일러가 우리가 만든 raw 포인터가 유효한 포인터인지 검증할 능력이 없기 때문입니다. `unsafe` 블록 안에 포인터에 대한 쓰기 작업 코드를 적음으로써, 우리는 컴파일러에게 해당 메모리 쓰기 작업이 확실히 안전하다고 선언한 것입니다. `unsafe` 블록이 Rust의 모든 안전성 체크를 해제하는 것은 아니며, `unsafe` 블록 안에서만 [다섯 가지 작업들을 추가적으로][five additional things] 할 수 있습니다.

[`unsafe`]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html
[five additional things]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html#unsafe-superpowers

**이런 식의 Rust 코드를 작성하는 것은 절대 바람직하지 않다는 것을 강조드립니다!** unsafe 블록 안에서 raw pointer를 쓰다보면 메모리 버퍼 크기를 넘어선 메모리 주소에 데이터를 저장하는 등의 실수를 범하기 매우 쉽습니다.

그렇기에 `unsafe` 블록의 사용을 최소화하는 것이 바람직하며, 그렇게 하기 위해 Rust에서 우리는 안전한 추상 계층을 만들어 이용할 수 있습니다. 예를 들어, 모든 위험한 요소들을 전부 캡슐화한 VGA 버퍼 타입을 만들어 외부 사용자가 해당 타입을 사용 중에 메모리 안전성을 해칠 가능성을 _원천 차단_ 할 수 있습니다. 이런 설계를 통해 최소한의 `unsafe` 블록만을 사용하면서 동시에 우리가 [메모리 안전성][memory safety]을 해치는 일이 없을 것이라 자신할 수 있습니다. 이러한 안전한 추상 레벨을 더한 VGA 버퍼 타입은 다음 포스트에서 만들게 될 것입니다.

[memory safety]: https://en.wikipedia.org/wiki/Memory_safety

## 커널 실행시키기

이제 우리가 얻은 실행 파일을 실행시켜볼 차례입니다. 우선 컴파일 완료된 커널을 부트로더와 링크하여 부팅 가능한 디스크 이미지를 만들어야 합니다. 그 다음에 해당 디스크 이미지를 QEMU 가상머신에서 실행시키거나 USB 드라이브를 이용해 실제 컴퓨터에서 부팅할 수 있습니다.

### 부팅 가능한 디스크 이미지 만들기

부팅 가능한 디스크 이미지를 만들기 위해서는 컴파일된 커널을 부트로더와 링크해야합니다. [부팅에 대한 섹션][section about booting]에서 알아봤듯이, 부트로더는 CPU를 초기화하고 커널을 불러오는 역할을 합니다.

[section about booting]: #the-boot-process

우리는 부트로더를 직접 작성하는 대신에 [`bootloader`] 크레이트를 사용할 것입니다. 이 크레이트는 Rust와 인라인 어셈블리만으로 간단한 BIOS 부트로더를 구현합니다. 운영체제 커널을 부팅하는 데에 이 크레이트를 쓰기 위해 의존 크레이트 목록에 추가해줍니다:

[`bootloader`]: https://crates.io/crates/bootloader

```toml
# Cargo.toml 에 들어갈 내용

[dependencies]
bootloader = "0.9.8"
```

부트로더를 의존 크레이트로 추가하는 것만으로는 부팅 가능한 디스크 이미지를 만들 수 없습니다. 커널 컴파일이 끝난 후 커널을 부트로더와 함께 링크할 수 있어야 하는데, cargo는 현재 [빌드 직후 스크립트 실행][post-build scripts] 기능을 지원하지 않습니다.

[post-build scripts]: https://github.com/rust-lang/cargo/issues/545

이 문제를 해결하기 위해 저희가 `bootimage` 라는 도구를 만들었습니다. 이 도구는 커널과 부트로더를 각각 컴파일 한 이후에 둘을 링크하여 부팅 가능한 디스크 이미지를 생성해줍니다. 이 도구를 설치하려면 터미널에서 아래의 명령어를 실행해주세요.

```
cargo install bootimage
```

`bootimage` 도구를 실행시키고 부트로더를 빌드하려면 `llvm-tools-preview` 라는 rustup 컴포넌트가 필요합니다. 명령어 `rustup component add llvm-tools-preview`를 통해 해당 컴포넌트를 설치합니다.

`bootimage` 도구를 설치하고 `llvm-tools-preview` 컴포넌트를 추가하셨다면, 이제 아래의 명령어를 통해 부팅 가능한 디스크 이미지를 만들 수 있습니다:

```
> cargo bootimage
```

이 도구가 `cargo build`를 통해 커널을 다시 컴파일한다는 것을 확인하셨을 것입니다. 덕분에 커널 코드가 변경되어도 `cargo bootimage` 명령어 만으로도 해당 변경 사항이 바로 빌드에 반영됩니다. 그 다음 단계로 이 도구가 부트로더를 컴파일 할 것인데, 시간이 제법 걸릴 수 있습니다. 일반적인 의존 크레이트들과 마찬가지로 한 번 빌드한 후에 빌드 결과가 캐시(cache)되기 때문에, 두 번째 빌드부터는 소요 시간이 훨씬 적습니다. 마지막 단계로 `bootimage` 도구가 부트로더와 커널을 하나로 합쳐 부팅 가능한 디스크 이미지를 생성합니다. 

명령어 실행이 끝난 후, `target/x86_64-blog_os/debug` 디렉토리에 `bootimage-blog_os.bin`이라는 부팅 가능한 디스크 이미지가 생성되어 있을 것입니다. 이것을 가상머신에서 부팅하거나 USB 드라이브에 복사한 뒤 실제 컴퓨터에서 부팅할 수 있습니다 (우리가 만든 디스크 이미지는 CD 이미지와는 파일 형식이 다르기 때문에 CD에 복사해서 부팅하실 수는 없습니다).

#### 어떻게 동작하는 걸까요?

`bootimage` 도구는 아래의 작업들을 순서대로 진행합니다:

- 커널을 컴파일하여 [ELF] 파일 생성
- 부트로더 크레이트를 독립된 실행파일로서 컴파일
- 커널의 ELF 파일을 부트로더에 링크

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
[rust-osdev/bootloader]: https://github.com/rust-osdev/bootloader

부팅이 시작되면, 부트로더는 커널의 ELF 파일을 읽고 파싱합니다. 그 다음 프로그램의 세그먼트들을 페이지 테이블의 가상 주소에 매핑하고, `bss` 섹션의 모든 메모리 값을 0으로 초기화하며, 스택을 초기화합니다. 마지막으로, 프로그램 실행 시작 지점의 주소 (`_start` 함수의 주소)에서 제어 흐름이 계속되도록 점프합니다.

### QEMU에서 커널 부팅하기

이제 우리의 커널 디스크 이미지를 가상 머신에서 부팅할 수 있습니다. [QEMU]에서 부팅하려면 아래의 명령어를 실행하세요:

[QEMU]: https://www.qemu.org/

```
> qemu-system-x86_64 -drive format=raw,file=target/x86_64-blog_os/debug/bootimage-blog_os.bin
warning: TCG doesn't support requested feature: CPUID.01H:ECX.vmx [bit 5]
```

위 명령어를 실행하면 아래와 같은 새로운 창이 열릴 것입니다:

![QEMU showing "Hello World!"](qemu.png)

화면에 "Hello World!" 메세지가 출력된 것을 확인하실 수 있습니다.

### 실제 컴퓨터에서 부팅하기

USB 드라이브에 우리의 커널을 저장한 후 실제 컴퓨터에서 부팅하는 것도 가능합니다:

```
> dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

`sdX` 대신 여러분이 소지한 USB 드라이브의 기기명을 입력하시면 됩니다. 해당 기기에 쓰인 데이터는 전부 덮어씌워지기 때문에 정확한 기기명을 입력하도록 주의해주세요.

이미지를 USB 드라이브에 다 덮어썼다면, 이제 실제 하드웨어에서 해당 이미지를 통해 부트하여 실행할 수 있습니다. 아마 특별한 부팅 메뉴를 사용하거나 BIOS 설정에서 부팅 순서를 변경하여 USB로부터 부팅하도록 설정해야 할 것입니다. `bootloader` 크레이트가 아직 UEFI를 지원하지 않기에, UEFI 표준을 사용하는 기기에서는 부팅할 수 없습니다.

### `cargo run` 명령어 사용하기

QEMU에서 커널을 쉽게 실행할 수 있게 아래처럼 `runner`라는 새로운 cargo 설정 키 값을 추가합니다.

```toml
# .cargo/config.toml 에 들어갈 내용

[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```

`target.'cfg(target_os = "none")'`가 붙은 키 값은 `"os"` 필드 설정이 `"none"`으로 되어 있는 컴파일 대상 환경에만 적용됩니다. 따라서 우리의 `x86_64-blog_os.json` 또한 적용 대상에 포함됩니다. `runner` 키 값은 `cargo run` 명령어 실행 시 어떤 명령어를 실행할지 지정합니다. 빌드가 성공적으로 끝난 후에 `runner` 키 값의 명령어가 실행됩니다. [cargo 공식 문서][cargo configuration]를 통해 더 자세한 내용을 확인하실 수 있습니다.

명령어 `bootimage runner`는 프로젝트의 부트로더 라이브러리를 링크한 후에 QEMU를 실행시킵니다.
그렇기에 일반적인 `runner` 실행파일을 실행하듯이 `bootimage runner` 명령어를 사용하실 수 있습니다. [`bootimage` 도구의 Readme 문서][Readme of `bootimage`]를 통해 더 자세한 내용 및 다른 가능한 설정 옵션들을 확인하세요.

[Readme of `bootimage`]: https://github.com/rust-osdev/bootimage

이제 `cargo run` 명령어를 통해 우리의 커널을 컴파일하고 QEMU에서 부팅할 수 있습니다.

## 다음 단계는 무엇일까요?

다음 글에서는 VGA 텍스트 버퍼 (text buffer)에 대해 더 알아보고 VGA text buffer와 안전하게 상호작용할 수 있는 방법을 구현할 것입니다.
또한 `println` 매크로를 사용할 수 있도록 기능을 추가할 것입니다.
