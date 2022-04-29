+++
title = "Red Zone 기능 해제하기"
weight = 1
path = "ko/red-zone"
template = "edition-2/extra.html"
+++

[red zone]은 [System V ABI]에서 사용 가능한 최적화 기법으로, 스택 포인터를 변경하지 않은 채로 함수들이 임시적으로 스택 프레임 아래의 128 바이트 공간을 사용할 수 있게 해줍니다:

[red zone]: https://eli.thegreenplace.net/2011/09/06/stack-frame-layout-on-x86-64#the-red-zone
[System V ABI]: https://wiki.osdev.org/System_V_ABI

<!-- more -->

![stack frame with red zone](red-zone.svg)

위 사진은 `n`개의 지역 변수를 가진 함수의 스택 프레임을 보여줍니다. 함수가 호출되었을 때, 함수의 반환 주소 및 지역 변수들을 스택에 저장할 수 있도록 스택 포인터의 값이 조정됩니다.

red zone은 조정된 스택 포인터 아래의 128바이트의 메모리 구간을 가리킵니다. 함수가 또 다른 함수를 호출하지 않는 구간에서만 사용하는 임시 데이터의 경우, 함수가 이 구간에 해당 데이터를 저장하는 데 이용할 수 있습니다. 따라서 스택 포인터를 조정하기 위해 필요한 명령어 두 개를 생략할 수 있는 상황이 종종 있습니다 (예: 다른 함수를 호출하지 않는 함수).

하지만 이 최적화 기법을 사용하는 도중 소프트웨어 예외(exception) 혹은 하드웨어 인터럽트가 일어날 경우 큰 문제가 생깁니다. 함수가 red zone을 사용하던 도중 예외가 발생한 상황을 가정해보겠습니다:

![red zone overwritten by exception handler](red-zone-overwrite.svg)

CPU와 예외 처리 핸들러가 red zone에 있는 데이터를 덮어씁니다. 하지만 이 데이터는 인터럽트된 함수가 사용 중이었던 것입니다. 따라서 예외 처리 핸들러로부터 반환하여 다시 인터럽트된 함수가 계속 실행되게 되었을 때 변경된 red zone의 데이터로 인해 함수가 오작동할 수 있습니다. 이런 현상으로 인해 [디버깅하는 데에 몇 주씩 걸릴 수 있는 이상한 버그][take weeks to debug]가 발생할지도 모릅니다.

[take weeks to debug]: https://forum.osdev.org/viewtopic.php?t=21720

미래에 예외 처리 로직을 구현할 때 이러한 오류가 일어나는 것을 피하기 위해 우리는 미리 red zone 최적화 기법을 해제한 채로 프로젝트를 진행할 것입니다. 컴파일 대상 환경 설정 파일에 `"disable-redzone": true` 줄을 추가함으로써 해당 기능을 해제할 수 있습니다.
