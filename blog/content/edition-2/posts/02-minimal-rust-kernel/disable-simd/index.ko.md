+++
title = "SIMD 해제하기"
weight = 2
path = "ko/disable-simd"
template = "edition-2/extra.html"
+++

[Single Instruction Multiple Data (SIMD)] 명령어들은 여러 데이터 word에 동시에 덧셈 등의 작업을 실행할 수 있으며, 이를 통해 프로그램의 실행 시간을 상당히 단축할 수 있습니다. `x86_64` 아키텍처는 다양한 SIMD 표준들을 지원합니다:

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

<!-- more -->

- [MMX]: _Multi Media Extension_ 명령어 집합은 1997년에 등장하였으며, `mm0`에서 `mm7`까지 8개의 64비트 레지스터들을 정의합니다. 이 레지스터들은 그저 [x87 부동 소수점 장치][x87 floating point unit]의 레지스터들을 가리키는 별칭입니다.
- [SSE]: _Streaming SIMD Extensions_ 명령어 집합은 1999년에 등장하였습니다. 부동 소수점 연산용 레지스터를 재사용하는 대신 새로운 레지스터 집합을 도입했습니다. `xmm0`에서 `xmm15`까지 16개의 새로운 128비트 레지스터를 정의합니다.
- [AVX]: _Advanced Vector Extensions_ 은 SSE에 추가로 멀티미디어 레지스터의 크기를 늘리는 확장 표준입니다. `ymm0`에서 `ymm15`까지 16개의 새로운 256비트 레지스터를 정의합니다. `ymm` 레지스터들은 기존의 `xmm` 레지스터를 확장합니다 (`xmm0`이 `ymm0` 레지스터의 하부 절반을 차지하는 식으로 다른 15개의 짝에도 같은 방식의 확장이 적용됩니다).

[MMX]: https://en.wikipedia.org/wiki/MMX_(instruction_set)
[x87 floating point unit]: https://en.wikipedia.org/wiki/X87
[SSE]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions
[AVX]: https://en.wikipedia.org/wiki/Advanced_Vector_Extensions

이러한 SIMD 표준들을 사용하면 프로그램 실행 속도를 많이 향상할 수 있는 경우가 많습니다. 우수한 컴파일러는 [자동 벡터화 (auto-vectorization)][auto-vectorization]이라는 과정을 통해 일반적인 반복문을 SIMD 코드로 변환할 수 있습니다.

[auto-vectorization]: https://en.wikipedia.org/wiki/Automatic_vectorization

하지만 운영체제 커널은 크기가 큰 SIMD 레지스터들을 사용하기에 문제가 있습니다. 그 이유는 하드웨어 인터럽트가 일어날 때마다 커널이 사용 중이던 레지스터들의 상태를 전부 메모리에 백업해야 하기 때문입니다. 이렇게 하지 않으면 인터럽트 되었던 프로그램의 실행이 다시 진행될 때 인터럽트 당시의 프로그램 상태를 보존할 수가 없습니다. 따라서 커널이 SIMD 레지스터들을 사용하는 경우, 커널이 백업해야 하는 데이터 양이 많이 늘어나게 되어 (512-1600 바이트) 커널의 성능이 눈에 띄게 나빠집니다. 이러한 성능 손실을 피하기 위해서 `sse` 및 `mmx` 기능을 해제하는 것이 바람직합니다 (`avx` 기능은 해제된 상태가 기본 상태입니다).

컴파일 대상 환경 설정 파일의 `features` 필드를 이용해 해당 기능들을 해제할 수 있습니다. `mmx` 및 `sse` 기능을 해제하려면 아래와 같이 해당 기능 이름 앞에 빼기 기호를 붙여주면 됩니다:

```json
"features": "-mmx,-sse"
```

## 부동소수점 (Floating Point)

우리의 입장에서는 안타깝게도, `x86_64` 아키텍처는 부동 소수점 계산에 SSE 레지스터를 사용합니다. 따라서 SSE 기능이 해제된 상태에서 부동 소수점 계산을 컴파일하면 LLVM이 오류를 일으킵니다. Rust의 core 라이브러리는 이미 부동 소수점 숫자들을 사용하기에 (예: `f32` 및 `f64` 에 대한 각종 trait들을 정의함), 우리의 커널에서 부동 소수점 계산을 피하더라도 부동 소수점 계산을 컴파일하는 것을 피할 수 없습니다.

다행히도 LLVM은 `soft-float` 기능을 지원합니다. 이 기능을 통해 정수 계만으로 모든 부동소수점 연산 결과를 모방하여 산출할 수 있습니다. 일반 부동소수점 계산보다는 느리겠지만, 이 기능을 통해 우리의 커널에서도 SSE 기능 없이 부동소수점을 사용할 수 있습니다. 

우리의 커널에서 `soft-float` 기능을 사용하려면 컴파일 대상 환경 설정 파일의 `features` 필드에 덧셈 기호와 함께 해당 기능의 이름을 적어주면 됩니다:

```json
"features": "-mmx,-sse,+soft-float"
```
