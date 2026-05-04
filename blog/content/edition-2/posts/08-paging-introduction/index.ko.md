+++
title = "페이징 소개"
weight = 8
path = "ko/paging-introduction"
date = 2019-01-14

[extra]
chapter = "Memory Management"
# Please update this when updating the translation
translation_based_on_commit = "ac943091147a57fcac8bde8876776c7aaff5c3d8"
# GitHub usernames of the people that translated this post
translators = ["potatogim"]
# GitHub usernames of the people that contributed to this translation
translation_contributors = []
+++

이 포스트에서는 우리가 만들 운영체제에서도 사용할 매우 일반적인 메모리 관리 방법인 _페이징_ 기법을 소개합니다. 왜 메모리 격리가 필요한지, _세그먼테이션_이 어떻게 동작하는지, _가상 메모리_가 무엇인지, 페이징이 어떻게 메모리 단편화 문제를 해결하는지를 설명합니다. 또한 x86_64 아키텍처에서 멀티 레벨 페이지 테이블의 레이아웃을 살펴봅니다.

<!-- more -->

이 블로그는 [GitHub 저장소][GitHub]에서 오픈 소스로 개발되고 있으니, 문제나 문의사항이 있다면 저장소의 'Issue' 기능을 이용해 제보해주세요. [페이지 맨 아래][at the bottom]에 댓글을 남기실 수도 있습니다. 이 포스트와 관련된 모든 소스 코드는 저장소의 [`post-07 브랜치`][post branch]에서 확인하실 수 있습니다.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-08

<!-- toc -->

## 메모리 보호

운영체제의 주요 작업 중 하나는 프로그램을 서로 격리하는 것입니다. 웹 브라우저가 텍스트 편집기를 방해할 수 없어야 한다는 것이 이러한 예입니다. 운영 체제는 이러한 목표를 달성하기 위해 하드웨어 기능을 활용하여 한 프로세스의 메모리 영역에 다른 프로세스가 접근할 수 없도록하며, 하드웨어와 운영체제 구현에 따라 이러한 구현의 접근 방식이 상이합니다.

예를 들어, 임베디드 시스템에 사용되는 일부 ARM Cortex-M 프로세서에는 [_메모리 보호 장치_](MPU; Memory Protection Unit)가 있어 서로 다른 접근 권한(예: 권한 없음, 읽기 전용, 읽기-쓰기)을 갖는 소수의 메모리 영역(예: 8개)을 정의할 수 있습니다. MPU는 메모리 접근 요청이 발생하면 요청된 영역에 위치한 메모리 주소에 대해 올바른 권한이 있는지 확인하고 그렇지 않으면 예외를 발생시킵니다. 운영체제는 프로세스가 전환될 때에 메모리 영역과 접근 권한도 같이 전환하여 각 프로세스가 자신의 메모리에만 접근하도록함으로써 프로세스를 서로 격리할 수 있습니다.

[_메모리 보호 장치_]: https://developer.arm.com/docs/ddi0337/e/memory-protection-unit/about-the-mpu

x86 아키텍처에서는 [세그먼테이션]과 [페이징]이라는 2가지의 다른 접근법을 제공합니다: 

[세그먼테이션]: https://en.wikipedia.org/wiki/X86_memory_segmentation
[페이징]: https://en.wikipedia.org/wiki/Virtual_memory#Paged_virtual_memory

## 세그먼테이션 

세그먼테이션은 주소 지정을 할 수 있는 메모리의 크기를 늘리기 위해 1978년에 이미 도입되었습니다. 당시에는 CPU가 16비트 주소만 사용했기 때문에 주소 지정 가능한 메모리의 크기가 64&nbsp;KiB로 제한되었습니다. 이 64&nbsp;KiB라는 제한을 극복하기 위해 오프셋 주소를 포함하는 세그먼트 레지스터가 추가적으로 도입되었고, CPU는 각 메모리 접근에 이 오프셋을 자동으로 추가해서 최대 1&nbsp;MiB 크기의 메모리에 접근할 수 있게 되었습니다.

세그먼트 레지스터는 메모리 접근 유형에 따라 CPU에 의해 자동으로 선택됩니다. 인출 명령에는 코드 세그먼트 `CS`가 사용되고, 스택 작업(push/pop)에는 스택 세그먼트 `SS`가 사용됩니다. 다른 명령어는 데이터 세그먼트 `DS` 또는 추가 세그먼트 `ES`를 사용합니다. 나중에는 자유롭게 사용할 수 있는 `FS`와 `GS`라는 2개의 세그먼트 레지스터가 추가되었습니다.

세그먼테이션의 첫 번째 버전에서는 세그먼트 레지스터가 오프셋을 직접 포함했으며 접근 제어가 수행되지 않았습니다. 이는 나중에 [_보호 모드_] 도입과 함께 변경되었습니다. CPU가 보호 모드에서 실행될 때 세그먼트 디스크립터에는 로컬 또는 글로벌 [_디스크립터 테이블_]의 인덱스가 포함되며 여기에는 오프셋 주소 외에도 세그먼트 크기 및 접근 권한이 포함됩니다. 각 프로세스별로 별도의 전역/로컬 디스크립터 테이블을 적재함으로써 각 프로세스는 해당 프로세스에 할당된 메모리 영역으로 메모리 접근을 한정하게 되며, 운영체제는 프로세스를 서로 격리할 수 있습니다.

[_보호 모드_]: https://en.wikipedia.org/wiki/X86_memory_segmentation#Protected_mode
[_디스크립터 테이블_]: https://en.wikipedia.org/wiki/Global_Descriptor_Table

실제로 메모리에 접근하기 전에 메모리 주소를 수정한다는 관점에서 세그먼테이션은 이제는 거의 모든 곳에서 사용되고 있는 기술인 _가상 메모리_를 차용했다고 볼 수 있습니다.

### 가상 메모리

가상 메모리의 기본 발상은 하위 계층의 물리적 저장 장치로부터 메모리 주소를 추상화하는 것입니다. 저장 장치에 직접 접근하는 대신 변환 단계가 먼저 수행됩니다. 세그먼테이션의 경우 변환 단계는 활성 세그먼트의 오프셋 주소를 추가하는 것입니다. 오프셋이 `0x1111000`인 세그먼트에서 메모리 주소 `0x1234000`에 접근하는 프로그램을 상상해보겠습니다. 이 경우, 실제로 접근되는 주소는 `0x2345000`입니다.

두 가지 주소 유형을 구분하기 위해서 변환하기 전 주소를 _가상 주소_라고 하고 변환하고 난 뒤의 주소를 _물리 주소_라고 합니다. 이 두 종류의 주소 사이의 한 가지 중요한 차이점은 물리 주소는 항상 동일한 별개의 메모리 위치를 참조하는 고유한 주소이고, 가상 주소는 변환 방식에 따라 다른 위치를 참조한다는 점입니다. 즉, 두 개의 서로 다른 가상 주소가 동일한 물리 주소를 참조하는 것 또한 가능하다는 말입니다. 또한 동일한 가상 주소가 서로 다른 변환 방식을 사용한다면 사용한 변환 방식에 따라 각기 다른 물리 주소를 참조할 수도 있습니다.

동일한 프로그램을 병렬로 두 번 실행하는 경우가 이러한 특성이 유용한 예입니다:

![주소가 0-150인 2개의 가상 주소 공간이 하나는 100-250으로, 하나는 300-450으로 변환](segmentation-same-program-twice.svg)

여기서는 동일한 프로그램이 두 번 실행되지만 변환 방식이 다릅니다. 첫 번째 인스턴스는 세그먼트 오프셋이 100이므로 가상 주소 0–150이 물리 주소 100–250으로 변환됩니다. 두 번째 인스턴스의 오프셋은 300이며 가상 주소 0–150을 물리적 주소 300–450으로 변환합니다. 이를 통해 두 프로그램은 서로 간섭하지 않고 동일한 코드를 실행하고 동일한 가상 주소를 사용할 수 있습니다.

또 다른 장점은 이제 프로그램들이 완전히 다른 가상 주소를 사용하더라도 임의의 물리적 메모리 위치에 배치할 수 있다는 것입니다. 따라서 운영체제는 프로그램을 다시 컴파일할 필요 없이 사용 가능한 메모리를 최대한 활용할 수 있습니다.

### 단편화

세그먼테이션은 가상 주소와 물리 주소를 구분함으로써 정말 강력해지지만, 이로 인해 단편화 문제가 생깁니다. 예를 들어, 위에서 본 프로그램의 세 번째 사본을 실행하고 싶다고 상상해보겠습니다.:

![3개의 가상 주소 공간이 있지만 세 번째 프로세스의 연속적인 메모리 공간이 부족함](segmentation-fragmentation.svg)

유휴 메모리 공간이 충분하지만 프로그램의 세 번째 인스턴스를 겹치지 않고 가상 메모리에 매핑할 방법이 없습니다. 문제는 _연속적인_ 메모리가 필요하지만 유휴 메모리 공간을 사용할 수 없다는 점입니다.

이러한 단편화를 제거하는 한 가지 방법은 실행을 일시 중지한 뒤에 사용된 메모리들을 인접하도록 이동하고 변환을 업데이트한 뒤에 실행을 재개하는 것입니다:

![단편화 제거 후 3개의 가상 주소 공간](segmentation-fragmentation-compacted.svg)

이제 프로그램의 세 번째 인스턴스를 시작하기에 충분한 연속 공간이 있습니다.

이 단편화 제거 절차의 단점은 많은 양의 메모리를 복사해야 하므로 성능이 저하된다는 것과 메모리가 과도하게 단편화되기 전에 정기적으로 수행해야 한다는 것입니다. 이로 인해 프로그램이 임의의 시간에 일시 중지되고 응답하지 않을 수 있으므로 성능을 예측할 수 없습니다.

이러한 단편화 문제는 대부분의 시스템에서 더 이상 세그먼테이션이 사용되지 않는 이유 중 하나입니다. 실제로 x86의 64비트 모드에서는 세그먼테이션이 더 이상 지원되지 않습니다. 대신 단편화 문제를 완전히 방지하는 _페이징_이 사용됩니다.

## 페이징

페이징의 기본 발상은 가상/물리 메모리 공간을 작은 고정 크기 블록으로 나누는 것입니다. 가상 메모리 공간의 블록을 _페이지_라고 하고 물리 주소 공간의 블록을 _프레임_이라고 합니다. 각 페이지는 프레임에 개별적으로 매핑될 수 있으므로 더 큰 메모리 영역을 비연속적인 물리적 프레임으로 분할할 수 있습니다.

The advantage of this becomes visible if we recap the example of the fragmented memory space, but use paging instead of segmentation this time:
단편화된 메모리 공간의 예를 다시 떠올려보면 페이징의 이점이 도드라져 보이겠지만, 여기에선 세그먼테이션 대신 페이징을 사용합니다.:

![페이징을 통해 세 번째 프로그램 인스턴스를 여러 개의 작은 물리 영역으로 분할](paging-fragmentation.svg)

이 예에서 페이지 크기는 50바이트이며 이는 각 메모리 영역이 세 페이지로 분할됨을 의미합니다. 각 페이지는 개별적으로 프레임에 매핑되므로 연속적인 가상 메모리 영역을 비연속적인 물리적 프레임에 매핑할 수 있습니다. 이를 통해 이전에 단편화 제거를 수행하지 않고 프로그램의 세 번째 인스턴스를 시작할 수 있습니다.

### 숨겨진 단편화

페이징은 세그먼테이션이 가변적인 크기를 갖는 다수의 메모리 공간을 사용하는 것에 비해 적은 개수의 작고 고정된 크기의 메모리 영역을 많이 사용합니다. 모든 프레임의 크기가 같기 때문에 사용하기에 너무 작은 프레임이 없으므로 단편화가 발생하지 않습니다.

또는 단편화가 발생하지 않는 것처럼 _보입니다_. 소위 _내부 단편화_라고 하는 일부 숨겨진 종류의 단편화는 여전히 있습니다. 내부 단편화는 모든 메모리 영역이 페이지 크기의 정확한 배수가 아니기 때문에 발생합니다. 위의 예에서 크기가 101인 프로그램을 상상해보겠습니다. 여전히 크기가 50인 세 페이지가 필요하므로 필요한 것보다 49바이트를 더 많이 차지합니다. 이러한 두 종류의 단편화를 구별하기 위해 세그먼테이션을 사용할 때 발생하는 단편화의 종류를 _외부 단편화_라고 합니다.

내부 단편화는 안타까운 일이지만 세그먼테이션에서 발생하는 외부 단편화보다는 나은 경우가 많습니다. 여전히 메모리를 낭비하지만 단편화 제거가 필요하지 않으며 단편화된 양을 예측할 수 있게 해줍니다 (대체적으로는 메모리 영역당 절반 페이지 정도).

### 페이지 테이블

We saw that each of the potentially millions of pages is individually mapped to a frame. This mapping information needs to be stored somewhere. Segmentation uses an individual segment selector register for each active memory region, which is not possible for paging since there are way more pages than registers. Instead, paging uses a table structure called _page table_ to store the mapping information.

For our above example, the page tables would look like this:

![Three page tables, one for each program instance. For instance 1, the mapping is 0->100, 50->150, 100->200. For instance 2, it is 0->300, 50->350, 100->400. For instance 3, it is 0->250, 50->450, 100->500.](paging-page-tables.svg)

We see that each program instance has its own page table. A pointer to the currently active table is stored in a special CPU register. On `x86`, this register is called `CR3`. It is the job of the operating system to load this register with the pointer to the correct page table before running each program instance.

On each memory access, the CPU reads the table pointer from the register and looks up the mapped frame for the accessed page in the table. This is entirely done in hardware and completely invisible to the running program. To speed up the translation process, many CPU architectures have a special cache that remembers the results of the last translations.

Depending on the architecture, page table entries can also store attributes such as access permissions in a flags field. In the above example, the "r/w" flag makes the page both readable and writable.

### Multilevel Page Tables

The simple page tables we just saw have a problem in larger address spaces: they waste memory. For example, imagine a program that uses the four virtual pages `0`, `1_000_000`, `1_000_050`, and `1_000_100` (we use `_` as a thousands separator):

![Page 0 mapped to frame 0 and pages `1_000_000`–`1_000_150` mapped to frames 100–250](single-level-page-table.svg)

It only needs 4 physical frames, but the page table has over a million entries. We can't omit the empty entries because then the CPU would no longer be able to jump directly to the correct entry in the translation process (e.g., it is no longer guaranteed that the fourth page uses the fourth entry).

To reduce the wasted memory, we can use a **two-level page table**. The idea is that we use different page tables for different address regions. An additional table called _level 2_ page table contains the mapping between address regions and (level 1) page tables.

This is best explained by an example. Let's define that each level 1 page table is responsible for a region of size `10_000`. Then the following tables would exist for the above example mapping:

![Page 0 points to entry 0 of the level 2 page table, which points to the level 1 page table T1. The first entry of T1 points to frame 0; the other entries are empty. Pages `1_000_000`–`1_000_150` point to the 100th entry of the level 2 page table, which points to a different level 1 page table T2. The first three entries of T2 point to frames 100–250; the other entries are empty.](multilevel-page-table.svg)

Page 0 falls into the first `10_000` byte region, so it uses the first entry of the level 2 page table. This entry points to level 1 page table T1, which specifies that page `0` points to frame `0`.

The pages `1_000_000`, `1_000_050`, and `1_000_100` all fall into the 100th `10_000` byte region, so they use the 100th entry of the level 2 page table. This entry points to a different level 1 page table T2, which maps the three pages to frames `100`, `150`, and `200`. Note that the page address in level 1 tables does not include the region offset. For example, the entry for page `1_000_050` is just `50`.

We still have 100 empty entries in the level 2 table, but much fewer than the million empty entries before. The reason for these savings is that we don't need to create level 1 page tables for the unmapped memory regions between `10_000` and `1_000_000`.

The principle of two-level page tables can be extended to three, four, or more levels. Then the page table register points to the highest level table, which points to the next lower level table, which points to the next lower level, and so on. The level 1 page table then points to the mapped frame. The principle in general is called a _multilevel_ or _hierarchical_ page table.

Now that we know how paging and multilevel page tables work, we can look at how paging is implemented in the x86_64 architecture (we assume in the following that the CPU runs in 64-bit mode).

## Paging on x86_64

The x86_64 architecture uses a 4-level page table and a page size of 4&nbsp;KiB. Each page table, independent of the level, has a fixed size of 512 entries. Each entry has a size of 8 bytes, so each table is 512 * 8&nbsp;B = 4&nbsp;KiB large and thus fits exactly into one page.

The page table index for each level is derived directly from the virtual address:

![Bits 0–12 are the page offset, bits 12–21 the level 1 index, bits 21–30 the level 2 index, bits 30–39 the level 3 index, and bits 39–48 the level 4 index](x86_64-table-indices-from-address.svg)

We see that each table index consists of 9 bits, which makes sense because each table has 2^9 = 512 entries. The lowest 12 bits are the offset in the 4&nbsp;KiB page (2^12 bytes = 4&nbsp;KiB). Bits 48 to 64 are discarded, which means that x86_64 is not really 64-bit since it only supports 48-bit addresses.

Even though bits 48 to 64 are discarded, they can't be set to arbitrary values. Instead, all bits in this range have to be copies of bit 47 in order to keep addresses unique and allow future extensions like the 5-level page table. This is called _sign-extension_ because it's very similar to the [sign extension in two's complement]. When an address is not correctly sign-extended, the CPU throws an exception.

[sign extension in two's complement]: https://en.wikipedia.org/wiki/Two's_complement#Sign_extension

It's worth noting that the recent "Ice Lake" Intel CPUs optionally support [5-level page tables] to extend virtual addresses from 48-bit to 57-bit. Given that optimizing our kernel for a specific CPU does not make sense at this stage, we will only work with standard 4-level page tables in this post.

[5-level page tables]: https://en.wikipedia.org/wiki/Intel_5-level_paging

### Example Translation

Let's go through an example to understand how the translation process works in detail:

![An example of a 4-level page hierarchy with each page table shown in physical memory](x86_64-page-table-translation.svg)

The physical address of the currently active level 4 page table, which is the root of the 4-level page table, is stored in the `CR3` register. Each page table entry then points to the physical frame of the next level table. The entry of the level 1 table then points to the mapped frame. Note that all addresses in the page tables are physical instead of virtual, because otherwise the CPU would need to translate those addresses too (which could cause a never-ending recursion).

The above page table hierarchy maps two pages (in blue). From the page table indices, we can deduce that the virtual addresses of these two pages are `0x803FE7F000` and `0x803FE00000`. Let's see what happens when the program tries to read from address `0x803FE7F5CE`. First, we convert the address to binary and determine the page table indices and the page offset for the address:

![The sign extension bits are all 0, the level 4 index is 1, the level 3 index is 0, the level 2 index is 511, the level 1 index is 127, and the page offset is 0x5ce](x86_64-page-table-translation-addresses.png)

With these indices, we can now walk the page table hierarchy to determine the mapped frame for the address:

- We start by reading the address of the level 4 table out of the `CR3` register.
- The level 4 index is 1, so we look at the entry with index 1 of that table, which tells us that the level 3 table is stored at address 16&nbsp;KiB.
- We load the level 3 table from that address and look at the entry with index 0, which points us to the level 2 table at 24&nbsp;KiB.
- The level 2 index is 511, so we look at the last entry of that page to find out the address of the level 1 table.
- Through the entry with index 127 of the level 1 table, we finally find out that the page is mapped to frame 12&nbsp;KiB, or 0x3000 in hexadecimal.
- The final step is to add the page offset to the frame address to get the physical address 0x3000 + 0x5ce = 0x35ce.

![The same example 4-level page hierarchy with 5 additional arrows: "Step 0" from the CR3 register to the level 4 table, "Step 1" from the level 4 entry to the level 3 table, "Step 2" from the level 3 entry to the level 2 table, "Step 3" from the level 2 entry to the level 1 table, and "Step 4" from the level 1 table to the mapped frames.](x86_64-page-table-translation-steps.svg)

The permissions for the page in the level 1 table are `r`, which means read-only. The hardware enforces these permissions and would throw an exception if we tried to write to that page. Permissions in higher level pages restrict the possible permissions in lower levels, so if we set the level 3 entry to read-only, no pages that use this entry can be writable, even if lower levels specify read/write permissions.

It's important to note that even though this example used only a single instance of each table, there are typically multiple instances of each level in each address space. At maximum, there are:

- one level 4 table,
- 512 level 3 tables (because the level 4 table has 512 entries),
- 512 * 512 level 2 tables (because each of the 512 level 3 tables has 512 entries), and
- 512 * 512 * 512 level 1 tables (512 entries for each level 2 table).

### Page Table Format

Page tables on the x86_64 architecture are basically an array of 512 entries. In Rust syntax:

```rust
#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}
```

As indicated by the `repr` attribute, page tables need to be page-aligned, i.e., aligned on a 4&nbsp;KiB boundary. This requirement guarantees that a page table always fills a complete page and allows an optimization that makes entries very compact.

Each entry is 8 bytes (64 bits) large and has the following format:

Bit(s) | Name | Meaning
------ | ---- | -------
0 | present | the page is currently in memory
1 | writable | it's allowed to write to this page
2 | user accessible | if not set, only kernel mode code can access this page
3 | write-through caching | writes go directly to memory
4 | disable cache | no cache is used for this page
5 | accessed | the CPU sets this bit when this page is used
6 | dirty | the CPU sets this bit when a write to this page occurs
7 | huge page/null | must be 0 in P1 and P4, creates a 1&nbsp;GiB page in P3, creates a 2&nbsp;MiB page in P2
8 | global | page isn't flushed from caches on address space switch (PGE bit of CR4 register must be set)
9-11 | available | can be used freely by the OS
12-51 | physical address | the page aligned 52bit physical address of the frame or the next page table
52-62 | available | can be used freely by the OS
63 | no execute | forbid executing code on this page (the NXE bit in the EFER register must be set)

We see that only bits 12–51 are used to store the physical frame address. The remaining bits are used as flags or can be freely used by the operating system. This is possible because we always point to a 4096-byte aligned address, either to a page-aligned page table or to the start of a mapped frame. This means that bits 0–11 are always zero, so there is no reason to store these bits because the hardware can just set them to zero before using the address. The same is true for bits 52–63, because the x86_64 architecture only supports 52-bit physical addresses (similar to how it only supports 48-bit virtual addresses).

Let's take a closer look at the available flags:

- The `present` flag differentiates mapped pages from unmapped ones. It can be used to temporarily swap out pages to disk when the main memory becomes full. When the page is accessed subsequently, a special exception called _page fault_ occurs, to which the operating system can react by reloading the missing page from disk and then continuing the program.
- The `writable` and `no execute` flags control whether the contents of the page are writable or contain executable instructions, respectively.
- The `accessed` and `dirty` flags are automatically set by the CPU when a read or write to the page occurs. This information can be leveraged by the operating system, e.g., to decide which pages to swap out or whether the page contents have been modified since the last save to disk.
- The `write-through caching` and `disable cache` flags allow the control of caches for every page individually.
- The `user accessible` flag makes a page available to userspace code, otherwise, it is only accessible when the CPU is in kernel mode. This feature can be used to make [system calls] faster by keeping the kernel mapped while a userspace program is running. However, the [Spectre] vulnerability can allow userspace programs to read these pages nonetheless.
- The `global` flag signals to the hardware that a page is available in all address spaces and thus does not need to be removed from the translation cache (see the section about the TLB below) on address space switches. This flag is commonly used together with a cleared `user accessible` flag to map the kernel code to all address spaces.
- The `huge page` flag allows the creation of pages of larger sizes by letting the entries of the level 2 or level 3 page tables directly point to a mapped frame. With this bit set, the page size increases by factor 512 to either 2&nbsp;MiB = 512 * 4&nbsp;KiB for level 2 entries or even 1&nbsp;GiB = 512 * 2&nbsp;MiB for level 3 entries. The advantage of using larger pages is that fewer lines of the translation cache and fewer page tables are needed.

[system calls]: https://en.wikipedia.org/wiki/System_call
[Spectre]: https://en.wikipedia.org/wiki/Spectre_(security_vulnerability)

The `x86_64` crate provides types for [page tables] and their [entries], so we don't need to create these structures ourselves.

[page tables]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page_table/struct.PageTable.html
[entries]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page_table/struct.PageTableEntry.html

### The Translation Lookaside Buffer

A 4-level page table makes the translation of virtual addresses expensive because each translation requires four memory accesses. To improve performance, the x86_64 architecture caches the last few translations in the so-called _translation lookaside buffer_ (TLB). This allows skipping the translation when it is still cached.

Unlike the other CPU caches, the TLB is not fully transparent and does not update or remove translations when the contents of page tables change. This means that the kernel must manually update the TLB whenever it modifies a page table. To do this, there is a special CPU instruction called [`invlpg`] (“invalidate page”) that removes the translation for the specified page from the TLB, so that it is loaded again from the page table on the next access. The TLB can also be flushed completely by reloading the `CR3` register, which simulates an address space switch. The `x86_64` crate provides Rust functions for both variants in the [`tlb` module].

[`invlpg`]: https://www.felixcloutier.com/x86/INVLPG.html
[`tlb` module]: https://docs.rs/x86_64/0.14.2/x86_64/instructions/tlb/index.html

It is important to remember to flush the TLB on each page table modification because otherwise, the CPU might keep using the old translation, which can lead to non-deterministic bugs that are very hard to debug.

## Implementation

One thing that we did not mention yet: **Our kernel already runs on paging**. The bootloader that we added in the ["A minimal Rust Kernel"] post has already set up a 4-level paging hierarchy that maps every page of our kernel to a physical frame. The bootloader does this because paging is mandatory in 64-bit mode on x86_64.

["A minimal Rust kernel"]: @/edition-2/posts/02-minimal-rust-kernel/index.md#creating-a-bootimage

This means that every memory address that we used in our kernel was a virtual address. Accessing the VGA buffer at address `0xb8000` only worked because the bootloader _identity mapped_ that memory page, which means that it mapped the virtual page `0xb8000` to the physical frame `0xb8000`.

Paging makes our kernel already relatively safe, since every memory access that is out of bounds causes a page fault exception instead of writing to random physical memory. The bootloader even sets the correct access permissions for each page, which means that only the pages containing code are executable and only data pages are writable.

### Page Faults

Let's try to cause a page fault by accessing some memory outside of our kernel. First, we create a page fault handler and register it in our IDT, so that we see a page fault exception instead of a generic [double fault]:

[double fault]: @/edition-2/posts/06-double-faults/index.md

```rust
// in src/interrupts.rs

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        […]

        idt.page_fault.set_handler_fn(page_fault_handler); // new

        idt
    };
}

use x86_64::structures::idt::PageFaultErrorCode;
use crate::hlt_loop;

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}
```

The [`CR2`] register is automatically set by the CPU on a page fault and contains the accessed virtual address that caused the page fault. We use the [`Cr2::read`] function of the `x86_64` crate to read and print it. The [`PageFaultErrorCode`] type provides more information about the type of memory access that caused the page fault, for example, whether it was caused by a read or write operation. For this reason, we print it too. We can't continue execution without resolving the page fault, so we enter a [`hlt_loop`] at the end.

[`CR2`]: https://en.wikipedia.org/wiki/Control_register#CR2
[`Cr2::read`]: https://docs.rs/x86_64/0.14.2/x86_64/registers/control/struct.Cr2.html#method.read
[`PageFaultErrorCode`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.PageFaultErrorCode.html
[LLVM bug]: https://github.com/rust-lang/rust/issues/57270
[`hlt_loop`]: @/edition-2/posts/07-hardware-interrupts/index.md#the-hlt-instruction

Now we can try to access some memory outside our kernel:

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    // new
    let ptr = 0xdeadbeaf as *mut u32;
    unsafe { *ptr = 42; }

    // as before
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

When we run it, we see that our page fault handler is called:

![EXCEPTION: Page Fault, Accessed Address: VirtAddr(0xdeadbeaf), Error Code: CAUSED_BY_WRITE, InterruptStackFrame: {…}](qemu-page-fault.png)

The `CR2` register indeed contains `0xdeadbeaf`, the address that we tried to access. The error code tells us through the [`CAUSED_BY_WRITE`] that the fault occurred while trying to perform a write operation. It tells us even more through the [bits that are _not_ set][`PageFaultErrorCode`]. For example, the fact that the `PROTECTION_VIOLATION` flag is not set means that the page fault occurred because the target page wasn't present.

[`CAUSED_BY_WRITE`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.PageFaultErrorCode.html#associatedconstant.CAUSED_BY_WRITE

We see that the current instruction pointer is `0x2031b2`, so we know that this address points to a code page. Code pages are mapped read-only by the bootloader, so reading from this address works but writing causes a page fault. You can try this by changing the `0xdeadbeaf` pointer to `0x2031b2`:

```rust
// Note: The actual address might be different for you. Use the address that
// your page fault handler reports.
let ptr = 0x2031b2 as *mut u32;

// read from a code page
unsafe { let x = *ptr; }
println!("read worked");

// write to a code page
unsafe { *ptr = 42; }
println!("write worked");
```

By commenting out the last line, we see that the read access works, but the write access causes a page fault:

![QEMU with output: "read worked, EXCEPTION: Page Fault, Accessed Address: VirtAddr(0x2031b2), Error Code: PROTECTION_VIOLATION | CAUSED_BY_WRITE, InterruptStackFrame: {…}"](qemu-page-fault-protection.png)

We see that the _"read worked"_ message is printed, which indicates that the read operation did not cause any errors. However, instead of the _"write worked"_ message, a page fault occurs. This time the [`PROTECTION_VIOLATION`] flag is set in addition to the [`CAUSED_BY_WRITE`] flag, which indicates that the page was present, but the operation was not allowed on it. In this case, writes to the page are not allowed since code pages are mapped as read-only.

[`PROTECTION_VIOLATION`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.PageFaultErrorCode.html#associatedconstant.PROTECTION_VIOLATION

### Accessing the Page Tables

Let's try to take a look at the page tables that define how our kernel is mapped:

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    use x86_64::registers::control::Cr3;

    let (level_4_page_table, _) = Cr3::read();
    println!("Level 4 page table at: {:?}", level_4_page_table.start_address());

    […] // test_main(), println(…), and hlt_loop()
}
```

The [`Cr3::read`] function of the `x86_64` returns the currently active level 4 page table from the `CR3` register. It returns a tuple of a [`PhysFrame`] and a [`Cr3Flags`] type. We are only interested in the frame, so we ignore the second element of the tuple.

[`Cr3::read`]: https://docs.rs/x86_64/0.14.2/x86_64/registers/control/struct.Cr3.html#method.read
[`PhysFrame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/frame/struct.PhysFrame.html
[`Cr3Flags`]: https://docs.rs/x86_64/0.14.2/x86_64/registers/control/struct.Cr3Flags.html

When we run it, we see the following output:

```
Level 4 page table at: PhysAddr(0x1000)
```

So the currently active level 4 page table is stored at address `0x1000` in _physical_ memory, as indicated by the [`PhysAddr`] wrapper type. The question now is: how can we access this table from our kernel?

[`PhysAddr`]: https://docs.rs/x86_64/0.14.2/x86_64/addr/struct.PhysAddr.html

Accessing physical memory directly is not possible when paging is active, since programs could easily circumvent memory protection and access the memory of other programs otherwise. So the only way to access the table is through some virtual page that is mapped to the physical frame at address `0x1000`. This problem of creating mappings for page table frames is a general problem since the kernel needs to access the page tables regularly, for example, when allocating a stack for a new thread.

Solutions to this problem are explained in detail in the next post.

## Summary

This post introduced two memory protection techniques: segmentation and paging. While the former uses variable-sized memory regions and suffers from external fragmentation, the latter uses fixed-sized pages and allows much more fine-grained control over access permissions.

Paging stores the mapping information for pages in page tables with one or more levels. The x86_64 architecture uses 4-level page tables and a page size of 4&nbsp;KiB. The hardware automatically walks the page tables and caches the resulting translations in the translation lookaside buffer (TLB). This buffer is not updated transparently and needs to be flushed manually on page table changes.

We learned that our kernel already runs on top of paging and that illegal memory accesses cause page fault exceptions. We tried to access the currently active page tables, but we weren't able to do it because the CR3 register stores a physical address that we can't access directly from our kernel.

## What's next?

The next post explains how to implement support for paging in our kernel. It presents different ways to access physical memory from our kernel, which makes it possible to access the page tables that our kernel runs on. At this point, we are able to implement functions for translating virtual to physical addresses and for creating new mappings in the page tables.
