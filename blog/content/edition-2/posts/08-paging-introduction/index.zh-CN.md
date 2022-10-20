+++
title = "内存分页初探"
weight = 8
path = "zh-CN/paging-introduction"
date = 2019-01-14

[extra]
# Please update this when updating the translation
translation_based_on_commit = "096c044b4f3697e91d8e30a2e817e567d0ef21a2"
# GitHub usernames of the people that translated this post
translators = ["liuyuran"]
+++

本文主要讲解 _内存分页_ 机制，一种我们将会应用到操作系统里的十分常见的内存模型。同时，也会展开说明为何需要进行内存隔离、_分段机制_ 是如何运作的、_虚拟内存_ 是什么，以及内存分页是如何解决内存碎片问题的，同时也会对x86_64的多级页表布局进行探索。

<!-- more -->

这个系列的 blog 在[GitHub]上开放开发，如果你有任何问题，请在这里开一个 issue 来讨论。当然你也可以在[底部][at the bottom]留言。你可以在[`post-08`][post branch]找到这篇文章的完整源码。

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-08

<!-- toc -->

## 内存保护

操作系统的主要任务之一就是隔离各个应用程序的执行环境，比如你的浏览器不应对你的文本编辑器造成影响，因此，操作系统会利用硬件级别的功能确保一个进程无法访问另一个进程的内存区域，但具体实现方式因硬件和操作系统实现而异。

比如一些 ARM Cortex-M 处理器（用于嵌入式系统）搭载了 [_内存保护单元_][_Memory Protection Unit_] (MPU)，该单元允许你定义少量具有不同读写权限的内存区域。MPU可以确保每一次对内存的访问都需要具备对应的权限，否则就会抛出异常。而操作系统则会在进程切换时，确保当前进程仅能访问自己所持有的内存区域，由此实现内存隔离。

[_Memory Protection Unit_]: https://developer.arm.com/docs/ddi0337/e/memory-protection-unit/about-the-mpu

在x86架构下，硬件层次为内存保护提供了两种不同的途径：[段][segmentation] 和 [页][paging]。

[segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation
[paging]: https://en.wikipedia.org/wiki/Virtual_memory#Paged_virtual_memory

## 内存分段

内存分段技术出现于1978年，初衷是用于扩展可用内存，该技术的最初背景是当时的CPU仅使用16位地址，而可使用的内存也只有64KiB。为了扩展可用内存，用于存储偏移量的段寄存器这个概念应运而生，CPU可以据此访问更多的内存，因此可用内存被成功扩展到了1MiB。

CPU可根据内存访问方式自动确定段寄存器的定义：对于指令获取操作，使用代码段寄存器 `CS`；对于栈操作（入栈/出栈），使用栈段寄存器 `SS`；对于其他指令，则使用数据段寄存器 `DS` 或额外段寄存器 `ES`。另外还有两个后来添加的扩展段寄存器 `FS` 和 `GS`，可以随意使用。

在最初版本的内存分段中，段寄存器仅仅是直接包含了偏移量，并不包含任何权限控制，直到 [_保护模式_][_protected mode_] 这个概念的出现。当CPU进入此模式后，段描述符会包含一个本地或全局的 [_描述符表_][_descriptor table_] 索引，它对应的数据包含了偏移量、段的大小和访问权限。通过加载各个进程所属的全局/本地描述符表，可以实现进程仅能访问属于自己的内存区域的效果，操作系统也由此实现了进程隔离。

[_protected mode_]: https://en.wikipedia.org/wiki/X86_memory_segmentation#Protected_mode
[_descriptor table_]: https://en.wikipedia.org/wiki/Global_Descriptor_Table

针对在判断权限前如何更正内存地址这个问题，内存分段使用了一个如今已经高度普及的技术：_虚拟内存_。

### 虚拟内存

所谓虚拟内存，就是将物理存储器地址抽象为一段完全独立的内存区域，在直接访问物理存储器之前，加入了一个地址转换的步骤。对于内存分页机制而言，地址转换就是在虚拟地址的基础上加入偏移量，如在偏移量为 `0x1111000` 的段中，虚拟地址 `0x1234000` 的对应的物理内存地址是 `0x2345000`。

首先我们需要明确两个名词，执行地址转换步骤之前的地址叫做 _虚拟地址_，而转换后的地址叫做 _物理地址_，两者最显著的区别就是物理地址是全局唯一的，而两个虚拟地址理论上可能指向同一个物理地址。同样的，如果使用不同的地址偏移量，同一个虚拟地址可能会对应不同的物理地址。

最直观的例子就是同时执行两个相同的程序：


![Two virtual address spaces with address 0–150, one translated to 100–250, the other to 300–450](segmentation-same-program-twice.svg)

如你所见，这就是两个相同程序的内存分配情况，两者具有不同的地址偏移量（即 _段基址_）。第一个程序实例的段基址为100，所以其虚拟地址范围0-150换算成物理地址就是100-250。第二个程序实例的段基址为300，所以其虚拟地址范围0-150换算成物理地址就是300-450。所以该机制允许程序共用同一套代码逻辑，使用同样的虚拟地址，并且不会干扰到彼此。

该机制的另一个优点就是让程序不局限于特定的某一段物理内存，而是依赖另一套虚拟内存地址，从而让操作系统在不重编译程序的前提下使用全部的内存区域。

### 内存碎片

虚拟内存机制已经让内存分段机制十分强大，但也有碎片化的问题，请看，如果我们同时执行三个程序实例的话：

![Three virtual address spaces, but there is not enough continuous space for the third](segmentation-fragmentation.svg)

在不能重叠使用的前提下，我们完全找不到足够的地方来容纳第三个程序，因为剩余的连续空间已经不够了。此时的问题在于，我们需要使用 _连续_ 的内存区域，不要将那些中间的空白部分白白浪费掉。

比较合适的办法就是暂停程序运行，将内存块移动到一个连续区间内，更新段基址信息，然后恢复程序运行：

![Three virtual address spaces after defragmentation](segmentation-fragmentation-compacted.svg)

这样我们就有足够的内存空间来运行第三个程序实例了。

但这样做也有一些问题，内存整理程序往往需要拷贝一段比较大的内存，这会很大程度上影响性能，但是又必须在碎片问题变得过于严重前完成这个操作。同时由于其消耗时间的不可预测性，程序很可能会随机挂起，甚至在用户视角下失去响应。

这也是大多数系统放弃内存分段技术的原因之一，事实上，该技术已经被x86平台的64位模式所抛弃，因为 _内存分页技术_ 已经完全解决了碎片化问题。

## 内存分页

内存分页的思想依然是使用虚拟地址映射物理地址，但是其分配单位变成了固定长度的较小的内存区域。这些虚拟内存块被称为 _页_，而其对应的物理内存则被称为 _页帧_，每一页都可以映射到一个对应的页帧中。这也就意味着我们可以将程序所使用的一大块内存区域打散到所有物理内存中，而不必分配一块连续的区域。

其优势就在于，如果我们遇到上文中提到的内存碎片问题时，内存分页技术会这样解决它：

![With paging the third program instance can be split across many smaller physical areas](paging-fragmentation.svg)

例如我们将页的单位设置为50字节，也就是说我们的每一个程序实例所使用的内存都被分割为三页。每一页都可以独立映射到一个页帧中，因此连续的虚拟内存并不一定需要对应连续的物理内存区域，因此也就无需进行内存碎片整理了。

### 潜在碎片

对比内存分段，内存分页选择用较多的较小且固定长度的内存区域代替较少的较大且长度不固定的内存区域。正因为如此，不会有页帧因为长度过小而产生内存碎片。

然而这只是 _表面上如此_，实际上依然存在着名为 _内部碎片_ 的隐蔽内存碎片，造成内部碎片的原因是并非每个内存区域都是分页单位的整数倍。比如一个程序需要101字节的内存，但它依然需要分配3个长度为50字节的页，最终造成了49字节的内存浪费，区别于内存分段造成的内存碎片，这种情况被称为 _内部碎片_。

内部碎片虽然也很可恶，但是无论如何也比内存分段造成的内存碎片要好得多，尽管其依然会浪费内存空间，但是无需碎片整理，且碎片数量是可预测的（每一个虚拟内存空间平均会造成半个页帧的内存浪费）。

### 页表

我们应当预见到，在操作系统开始运行后，会存在数以百万计的页-页帧映射关系，这些映射关系需要存储在某个地方。分段技术可以为每个活动的内存区域都指定一个段寄存器，但是分页技术不行，因为其使用到的页的数量实在是太多了，远多于寄存器数量，所以分页技术采用了一种叫做 _页表_ 的结构来存储映射信息。

以上面的应用场合为例，页表看起来是这样子的：

![Three page tables, one for each program instance. For instance 1 the mapping is 0->100, 50->150, 100->200. For instance 2 it is 0->300, 50->350, 100->400. For instance 3 it is 0->250, 50->450, 100->500.](paging-page-tables.svg)

我们可以看到每个程序实例都有其专有的页表，但当前正在活跃的页表指针会被存储到特定的CPU寄存器中，在 `x86` 架构中，该寄存器被称为 `CR3`。操作系统的任务之一，就是在程序运行前，把当前所使用的页表指针推进对应的寄存器中。

每次内存访问CPU都会从寄存器获取页表指针，并从页表中获取虚拟地址所对应的页帧，这一步操作完全由硬件完成，对于程序而言是完全透明的。为了加快地址转换的速度，许多CPU架构都加入了一个能够存储最后一次地址转换相关信息的特殊缓存。

根据架构实现的不同，页表也可以在 flags 字段存储一些额外的属性，如访问权限之类。在上面的场景下。 "r/w" 这个 flag 可以使该页同时能够读和写。

### 多级页表

上文中的简单页表在较大的地址空间下会有个问题：太浪费内存了。打个比方，一个程序需要使用4个虚拟内存页 `0`、`1_000_000`、`1_000_050` 和 `1_000_100`（假设以 `_` 为千位分隔符）：

![Page 0 mapped to frame 0 and pages `1_000_000`–`1_000_150` mapped to frames 100–250](single-level-page-table.svg)

尽管它仅仅会使用4个页帧，但是页表中已经百万级别的映射条目，而我们还不能释放那些空白的条目，因为这会对地址转换造成很大的风险（比如可能无法保证4号页依然对应4号页帧）。

没错，这样做对内存造成了很大的浪费，所以我们可以使用 **两级页表** 来解决这个问题，其基本思路就是定义了一个新的概念 **内存区域**，它可以通过一级页表间接映射到一段相对较长的内存区域中。

举个例子，我们先假设每个一级页表映射 `10_000` 字节的内存空间，在上文所述的应用场合下，此时的页表结构看上去是这样的：

![Page 0 points to entry 0 of the level 2 page table, which points to the level 1 page table T1. The first entry of T1 points to frame 0, the other entries are empty. Pages `1_000_000`–`1_000_150` point to the 100th entry of the level 2 page table, which points to a different level 1 page table T2. The first three entries of T2 point to frames 100–250, the other entries are empty.](multilevel-page-table.svg)

页 `0` 位于第一个 `10_000` 字节的内存区域内，位于内存区域 `0` 内，对应一级页表 `T1`，所以它所在的内存位置也可以被表述为 `页 0 帧 0`.

页 `1_000_000`、 `1_000_050` 和 `1_000_100` 均可以映射到第100个 `10_000` 字节的内存区域内，所以位于内存区域 `1_000_100` 中，该内存区域指向一级页表 T2。但这三个页分别对应该一级页表 T2 中的页帧 `100`、`150` 和 `200`，因为一级页表中是不存储内存区域偏移量的。

在这个场合中，二级页表中还是出现了100个被浪费的位置，不过无论如何也比之前数以百万计的浪费好多了，因为我们没有额外创建指向 `10_000` 到 `1_000_000` 这段内存区域的一级页表。

同理，两级页表的原理可以扩展到三级、四级甚至更多的级数。通常而言，可以让页表寄存器指向最高级数的表，然后一层一层向下寻址，直到抵达一级页表，获取页帧地址。这种技术就叫做 _多级_ 或 _多层_ 页表。

那么现在我们已经明白了内存分页和多级页表机制的工作原理，下面我们会探索一下在 x86_64 平台下内存分页机制是如何实现的（假设CPU运行在64位模式下）。

## x86_64中的分页

x86_64 平台使用4级页表，页大小为4KiB，无论层级，每个页表均具有512个条目，每个条目占用8字节，所以每个页表固定占用 512 * 8B = 4KiB，正好占满一个内存页。

每一级的页表索引号都可以通过虚拟地址推导出来：

![Bits 0–12 are the page offset, bits 12–21 the level 1 index, bits 21–30 the level 2 index, bits 30–39 the level 3 index, and bits 39–48 the level 4 index](x86_64-table-indices-from-address.svg)

我们可以看到，每个表索引号占据9个字节，这当然是有道理的，每个表都有 2^9 = 512 个条目，低12位用来表示内存页的偏移量（2^12 bytes = 4KiB，而上文提到页大小为4KiB）。第48-64位毫无用处，这也就意味着 x86_64 并非真正的64位，因为它实际上支持48位地址。

[5-level page table]: https://en.wikipedia.org/wiki/Intel_5-level_paging

尽管48-64位毫无用处，但依然不被允许随意赋值，而是必须将其设置为与47位相同的值以保证地址唯一性，由此留出未来对此进行扩展的可能性，如实现5级页表。该技术被称为 _符号扩展_，理由是它与 [二进制补码][sign extension in two's complement] 机制真的太相似了。当地址不符合该机制定义的规则时，CPU会抛出异常。

[sign extension in two's complement]: https://en.wikipedia.org/wiki/Two's_complement#Sign_extension

值得注意的是，英特尔最近发布了一款代号是冰湖的CPU，它的新功能之一就是可选支持能够将虚拟地址从48位扩展到57位的 [5级页表][5-level page tables]。但是针对一款特定的CPU做优化在现阶段并没有多少意义，所以本文仅会涉及标准的4级页表。

[5-level page tables]: https://en.wikipedia.org/wiki/Intel_5-level_paging

### 地址转换范例

请看下图，这就是一个典型的地址转换过程的范例：

![An example 4-level page hierarchy with each page table shown in physical memory](x86_64-page-table-translation.svg)

`CR3` 寄存器中存储着指向4级页表的物理地址，而在每一级的页表（除一级页表外）中，都存在着指向下一级页表的指针，1级页表则存放着直接指向页帧地址的指针。注意，这里的指针，都是指页表的物理地址，而非虚拟地址，否则CPU会因为需要进行额外的地址转换而陷入无限递归中。

最终，寻址结果是上图中的两个蓝色区域，根据页表查询结果，它们的虚拟地址分别是 `0x803FE7F000` 和 `0x803FE00000`，那么让我们看一看当程序尝试访问内存地址 `0x803FE7F5CE` 时会发生什么事情。首先我们需要把地址转换为二进制，然后确定该地址所对应的页表索引和页偏移量：

![The sign extension bits are all 0, the level 4 index is 1, the level 3 index is 0, the level 2 index is 511, the level 1 index is 127, and the page offset is 0x5ce](x86_64-page-table-translation-addresses.png)

通过这些索引，我们就可以通过依次查询多级页表来定位最终要指向的页帧：

- 首先，我们需要从 `CR3` 寄存器中读出4级页表的物理地址。
- 4级页表的索引号是1，所以我们可以看到3级页表的地址是16KiB。
- 载入3级页表，根据索引号0，确定2级页表的地址是24KiB。
- 载入2级页表，根据索引号511，确定1级页表的地址是32KiB。
- 载入1级页表，根据索引号127，确定该地址所对应的页帧地址为12KiB，使用Hex表达可写作 0x3000。
- 最终步骤就是将最后的页偏移量拼接到页帧地址上，即可得到物理地址，即 0x3000 + 0x5ce = 0x35ce。

![The same example 4-level page hierarchy with 5 additional arrows: "Step 0" from the CR3 register to the level 4 table, "Step 1" from the level 4 entry to the level 3 table, "Step 2" from the level 3 entry to the level 2 table, "Step 3" from the level 2 entry to the level 1 table, and "Step 4" from the level 1 table to the mapped frames.](x86_64-page-table-translation-steps.svg)

由上图可知，该页帧在一级页表中的权限被标记为 `r`，即只读，硬件层面已经确保当我们试图写入数据的时候会抛出异常。较高级别的页表的权限设定会覆盖较低级别的页表，如3级页表中设定为只读的区域，其所关联的所有下级页表对应的内存区域均会被认为是只读，低级别的页表本身的设定会被忽略。

注意，示例图片中为了简化显示，看起来每个页表都只有一个条目，但实际上，4级以下的页表每一层都可能存在多个实例，其数量上限如下：

- 1个4级页表
- 512个3级页表（因为4级页表可以有512个条目）
- 512*512个2级页表（因为每个3级页表可以有512个条目）
- 512*512*512个1级页表（因为每个2级页表可以有512个条目）

### 页表格式

在 x86_64 平台下，页表是一个具有512个条目的数组，于Rust而言就是这样：

```rust
#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}
```

`repr` 属性定义了内存页的大小，这里将其设定为了4KiB，该设置确保了页表总是能填满一整个内存页，并允许编译器进行一些优化，使其存储方式更加紧凑。

每个页表条目长度都是8字节（64比特），其内部结构如下：

Bit(s) | 名字                    | 含义
------ |-----------------------| -------
0 | present               | 该页目前在内存中
1 | writable              | 该页可写
2 | user accessible       | 如果没有设定，仅内核代码可以访问该页
3 | write through caching | 写操作直接应用到内存
4 | disable cache         | 对该页禁用缓存
5 | accessed              | 当该页正在被使用时，CPU设置该比特的值
6 | dirty                 | 当该页正在被写入时，CPU设置该比特的值
7 | huge page/null        | 在P1和P4状态时必须为0，在P3时创建一个1GiB的内存页，在P2时创建一个2MiB的内存页
8 | global                | 当地址空间切换时，该页尚未应用更新（CR4寄存器中的PGE比特位必须一同被设置）
9-11 | available             | 可被操作系统自由使用
12-51 | physical address      | 经过52比特对齐过的页帧地址，或下一级的页表地址
52-62 | available             | 可被操作系统自由使用
63 | no execute            | 禁止在该页中运行代码（EFER寄存器中的NXE比特位必须一同被设置）

我们可以看到，仅12–51位会用于存储页帧地址或页表地址，其余比特都用于存储标志位，或由操作系统自由使用。 
其原因就是，该地址总是指向一个4096比特对齐的地址、页表或者页帧的起始地址。
这也就意味着0-11位始终为0，没有必要存储这些东西，硬件层面在使用该地址之前，也会将这12位比特设置为0，52-63位同理，因为x86_64平台仅支持52位物理地址（类似于上文中提到的仅支持48位虚拟地址的原因）。

进一步说明一下可用的标志位：

- `present` 标志位并非是指未映射的页，而是指其对应的内存页由于物理内存已满而被交换到硬盘中，如果该页在换出之后再度被访问，则会抛出 _page fault_ 异常，此时操作系统应该将此页重新载入物理内存以继续执行程序。
- `writable` 和 `no execute` 标志位分别控制该页是否可写，以及是否包含可执行指令。
- `accessed` 和 `dirty` 标志位由CPU在读写该页时自动设置，该状态信息可用于辅助操作系统的内存控制，如判断哪些页可以换出，以及换出到硬盘后页里的内容是否已被修改。
- `write through caching` 和 `disable cache` 标志位可以单独控制每一个页对应的缓存。
- `user accessible` 标志位决定了页中是否包含用户态的代码，否则它仅当CPU处于核心态时可访问。该特性可用于在用户态程序运行时保持内核代码映射以加速[系统调用][system calls]。然而，[Spectre] 漏洞会允许用户态程序读取到此类页的数据。
- `global` 标志位决定了该页是否会在所有地址空间都存在，即使切换地址空间，也不会从地址转换缓存（参见下文中关于TLB的章节）中被移除。一般和 `user accessible` 标志位共同使用，在所有地址空间映射内核代码。
- `huge page` 标志位允许2级页表或3级页表直接指向页帧来分配一块更大的内存空间，该标志位被启用后，页大小会增加512倍。就结果而言，对于2级页表的条目，其会直接指向一个 2MiB = 512 * 4KiB 大小的大型页帧，而对于3级页表的条目，就会直接指向一个 1GiB = 512 * 2MiB 大小的巨型页帧。通常而言，这个功能会用于节省地址转换缓存的空间，以及降低逐层查找页表的耗时。

[system calls]: https://en.wikipedia.org/wiki/System_call
[Spectre]: https://en.wikipedia.org/wiki/Spectre_(security_vulnerability)

`x86_64` crate 为我们提供了 [page tables] 的结构封装，以及其内部条目 [entries]，所以我们无需自己实现具体的结构。

[page tables]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page_table/struct.PageTable.html
[entries]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page_table/struct.PageTableEntry.html

### 地址转换后备缓冲区（TLB）

显而易见，4级页表使地址转换过程变得有点慢，每次转换都需要进行4次内存访问。为了改善这个问题，x86_64平台将最后几次转换结果放在所谓的 _地址转换后备缓冲区_（TLB）中，这样对同样地址的连续重复转换就可以直接返回缓存中存储的结果。

不同于CPU缓存，TLB并非是完全对外透明的，它在页表变化时并不会自动更新或删除被缓存的结果。这也就是说，内核需要在页表发生变化时，自己来处理TLB的更新。针对这个需要，CPU也提供了一个用于从TLB删除特定页的缓存的指令 [`invlpg`] （“invalidate page”），调用该指令之后，下次访问该页就会重新生成缓存。不过还有一个更彻底的办法，通过手动写入 `CR3` 寄存器可以制造出模拟地址空间切换的效果，TLB也会被完全刷新。`x86_64` crate 中的 [`tlb` module] 提供了上面的两种手段，并封装了对应的函数。

[`invlpg`]: https://www.felixcloutier.com/x86/INVLPG.html
[`tlb` module]: https://docs.rs/x86_64/0.14.2/x86_64/instructions/tlb/index.html

请注意，在修改页表之后，同步修改TLB是十分十分重要的事情，不然CPU可能会返回一个错误的物理地址，因为这种原因造成的bug是非常难以追踪和调试的。

## 具体实现

有件事我们还没有提过：**我们的内核已经是在页上运行的**。在前文 ["最小内核"]["A minimal Rust Kernel"] 中，我们添加的bootloader已经搭建了一个4级页表结构，并将内核中使用的每个页都映射到了物理页帧上，其原因就是，在64位的 x86_64 平台下分页是被强制使用的。

["A minimal Rust kernel"]: @/edition-2/posts/02-minimal-rust-kernel/index.md#creating-a-bootimage

这也就是说，我们在内核中所使用的每一个内存地址其实都是虚拟地址，VGA缓冲区是唯一的例外，因为bootloader为这个地址使用了 _一致映射_，令其直接指向地址 `0xb8000`。所谓一致映射，就是能将虚拟页 `0xb8000` 直接映射到物理页帧 `0xb8000`。

使用分页技术后，我们的内核在某种意义上已经十分安全了，因为越界的内存访问会导致 page fault 异常而不是访问到一个随机物理地址。bootloader已经为每一个页都设置了正确的权限，比如仅代码页具有执行权限、仅数据页具有写权限。

### Page Faults

那么我们来通过内存越界访问手动触发一次 page fault，首先我们先写一个错误处理函数并注册到IDT中，这样我们就可以正常接收到这个异常，而非 [double fault] 了：

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

[`CR2`] 寄存器会在 page fault 发生时，被CPU自动写入导致异常的虚拟地址，我们可以用 `x86_64` crate 提供的 [`Cr2::read`] 函数来读取并打印该寄存器。[`PageFaultErrorCode`] 类型为我们提供了内存访问型异常的具体信息，比如究竟是因为读取还是写入操作，我们同样将其打印出来。并且不要忘记，在显式结束异常处理前，程序是不会恢复运行的，所以要在最后调用 [`hlt_loop`] 函数。

[`CR2`]: https://en.wikipedia.org/wiki/Control_register#CR2
[`Cr2::read`]: https://docs.rs/x86_64/0.14.2/x86_64/registers/control/struct.Cr2.html#method.read
[`PageFaultErrorCode`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.PageFaultErrorCode.html
[LLVM bug]: https://github.com/rust-lang/rust/issues/57270
[`hlt_loop`]: @/edition-2/posts/07-hardware-interrupts/index.md#the-hlt-instruction

那么可以开始触发内存越界访问了：

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

启动执行后，我们可以看到，page fault 的处理函数被触发了：

![EXCEPTION: Page Fault, Accessed Address: VirtAddr(0xdeadbeaf), Error Code: CAUSED_BY_WRITE, InterruptStackFrame: {…}](qemu-page-fault.png)

`CR2` 确实保存了导致异常的虚拟地址 `0xdeadbeaf`，而错误码 [`CAUSED_BY_WRITE`] 也说明了导致异常的操作是写入。甚至于可以通过 [未设置的比特位][`PageFaultErrorCode`] 看出更多的信息，例如 `PROTECTION_VIOLATION` 未被设置说明目标页根本就不存在。

[`CAUSED_BY_WRITE`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.PageFaultErrorCode.html#associatedconstant.CAUSED_BY_WRITE

并且我们可以看到当前指令指针是 `0x2031b2`，根据上文的知识，我们知道它应该属于一个代码页。而代码页被bootloader设定为只读权限，所以读取是正常的，但写入就会触发 page fault 异常。比如你可以试着将上面代码中的 `0xdeadbeaf` 换成 `0x2031b2`：

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

执行后，我们可以看到读取操作成功了，但写入操作抛出了 page fault 异常：

![QEMU with output: "read worked, EXCEPTION: Page Fault, Accessed Address: VirtAddr(0x2031b2), Error Code: PROTECTION_VIOLATION | CAUSED_BY_WRITE, InterruptStackFrame: {…}"](qemu-page-fault-protection.png)

我们可以看到 _"read worked"_ 这条日志，说明读操作没有出问题，而 _"write worked"_ 这条日志则没有被打印，起而代之的是一个异常日志。这一次 [`PROTECTION_VIOLATION`] 标志位的 [`CAUSED_BY_WRITE`] 比特位被设置，说明异常正是被非法写入操作引发的，因为我们之前为该页设置了只读权限。

[`PROTECTION_VIOLATION`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.PageFaultErrorCode.html#associatedconstant.PROTECTION_VIOLATION

### 访问页表

那么我们来看看内核中页表的存储方式：

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
`x86_64` crate 中的 [`Cr3::read`] 函数可以返回 `CR3` 寄存器中的当前使用的4级页表，它返回的是 [`PhysFrame`] 和 [`Cr3Flags`] 两个类型组成的元组结构。不过此时我们只关心页帧信息，所以第二个元素暂且不管。

[`Cr3::read`]: https://docs.rs/x86_64/0.14.2/x86_64/registers/control/struct.Cr3.html#method.read
[`PhysFrame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/frame/struct.PhysFrame.html
[`Cr3Flags`]: https://docs.rs/x86_64/0.14.2/x86_64/registers/control/struct.Cr3Flags.html

然后我们会看到如下输出：

```
Level 4 page table at: PhysAddr(0x1000)
```

所以当前的4级页表存储在 _物理地址_ `0x1000` 处，而且地址的外层数据结构是 [`PhysAddr`]，那么问题来了：我们如何在内核中直接访问这个页表？

[`PhysAddr`]: https://docs.rs/x86_64/0.14.2/x86_64/addr/struct.PhysAddr.html

当分页功能启用时，直接访问物理内存是被禁止的，否则程序就可以很轻易的侵入其他程序的内存，所以唯一的途径就是通过某些手段构建一个指向 `0x1000` 的虚拟页。那么问题就变成了如何手动创建页映射，但其实该功能在很多地方都会用到，例如内核在创建新的线程时需要额外创建栈，同样需要用到该功能。

我们将在下一篇文章中对此问题进行展开。

## 小结

本文介绍了两种内存保护技术：分段和分页。前者每次分配的内存区域大小是可变的，但会受到内存碎片的影响；而后者使用固定大小的页，并允许对访问权限进行精确控制。

分页技术将映射信息存储在一级或多级页表中，x86_64 平台使用4级页表和4KiB的页大小，硬件会自动逐级寻址并将地址转换结果存储在地址转换后备缓冲区（TLB）中，然而此缓冲区并非完全对用户透明，需要在页表发生变化时进行手动干预。

并且我们知道了内核已经被预定义了一个分页机制，内存越界访问会导致 page fault 异常。并且我们暂时无法访问当前正在使用的页表，因为 CR3 寄存器存储的地址无法在内核中直接访问。

## 下文预告

在下一篇文章中，我们会详细讲解如何在内核中实现对分页机制的支持，这会提供一种直接访问物理内存的特别手段，也就是说我们可以直接访问页表。由此，我们可以在程序中实现虚拟地址到物理地址的转换函数，也使得在页表中手动创建映射成为了可能。
