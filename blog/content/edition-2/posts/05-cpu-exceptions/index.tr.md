+++
title = "CPU Exception'ları"
weight = 5
path = "tr/cpu-exceptions"
date  = 2018-06-17

[extra]
chapter = "Interrupts"

# Please update this when updating the translation
translation_based_on_commit = "211f460251cd332905225c93eb66b1aff9f4aefd"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

CPU exception'ları çeşitli hatalı durumlarda meydana gelir; örneğin geçersiz bir bellek adresine erişilirken veya sıfıra bölünürken. Onlara tepki verebilmek için, handler fonksiyonları sağlayan bir _interrupt descriptor table_ kurmamız gerekir. Bu yazının sonunda, kernel'imiz [breakpoint exception'larını][breakpoint exceptions] yakalayabilecek ve sonrasında normal çalıştırmaya devam edebilecek.

[breakpoint exceptions]: https://wiki.osdev.org/Exceptions#Breakpoint

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-05`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-05

<!-- toc -->

## Genel Bakış
Bir exception, mevcut komutla ilgili bir şeyin yanlış olduğunu bildirir. Örneğin, mevcut komut 0'a bölmeye çalışırsa CPU bir exception verir. Bir exception meydana geldiğinde, CPU mevcut işini keser ve exception tipine bağlı olarak hemen belirli bir exception handler fonksiyonunu çağırır.

x86'da yaklaşık 20 farklı CPU exception tipi vardır. En önemlileri şunlardır:

- **Page Fault**: Bir page fault, yasa dışı bellek erişimlerinde meydana gelir. Örneğin, mevcut komut eşlenmemiş bir sayfadan okumaya veya salt okunur bir sayfaya yazmaya çalışırsa.
- **Invalid Opcode**: Bu exception, mevcut komut geçersiz olduğunda meydana gelir; örneğin, yeni [SSE komutlarını][SSE instructions] bunları desteklemeyen eski bir CPU'da kullanmaya çalıştığımızda.
- **General Protection Fault**: Bu, en geniş neden yelpazesine sahip exception'dır. Çeşitli erişim ihlallerinde meydana gelir; örneğin kullanıcı seviyesindeki kodda ayrıcalıklı bir komut çalıştırmaya çalışmak veya yapılandırma register'larındaki ayrılmış alanlara yazmak gibi.
- **Double Fault**: Bir exception meydana geldiğinde, CPU karşılık gelen handler fonksiyonunu çağırmaya çalışır. _Exception handler çağrılırken_ başka bir exception meydana gelirse, CPU bir double fault exception'ı yükseltir. Bu exception, bir exception için kayıtlı bir handler fonksiyonu olmadığında da meydana gelir.
- **Triple Fault**: CPU double fault handler fonksiyonunu çağırmaya çalışırken bir exception meydana gelirse, ölümcül bir _triple fault_ verir. Bir triple fault'u yakalayamaz veya işleyemeyiz. Çoğu işlemci buna kendini sıfırlayarak ve işletim sistemini yeniden başlatarak tepki verir.

[SSE instructions]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions

Exception'ların tam listesi için [OSDev wiki][exceptions]'sine göz atın.

[exceptions]: https://wiki.osdev.org/Exceptions

### Interrupt Descriptor Table {#the-interrupt-descriptor-table}
Exception'ları yakalamak ve işlemek için, _Interrupt Descriptor Table_ (IDT) adı verilen bir tablo kurmamız gerekir. Bu tabloda, her CPU exception'ı için bir handler fonksiyonu belirtebiliriz. Donanım bu tabloyu doğrudan kullanır, bu yüzden önceden tanımlanmış bir biçimi takip etmemiz gerekir. Her girdi aşağıdaki 16 baytlık yapıya sahip olmalıdır:

| Tip  | Ad                       | Açıklama                                                          |
| ---- | ------------------------ | ----------------------------------------------------------------- |
| u16  | Function Pointer [0:15]  | Handler fonksiyonuna işaretçinin alt bitleri.                     |
| u16  | GDT seçicisi             | [Global descriptor table]'daki bir kod segmentinin seçicisi.      |
| u16  | Options                  | (aşağıya bakın)                                                   |
| u16  | Function Pointer [16:31] | Handler fonksiyonuna işaretçinin orta bitleri.                    |
| u32  | Function Pointer [32:63] | Handler fonksiyonuna işaretçinin kalan bitleri.                   |
| u32  | Reserved                 |

[global descriptor table]: https://en.wikipedia.org/wiki/Global_Descriptor_Table

Options alanı aşağıdaki biçime sahiptir:

| Bitler | Ad                               | Açıklama                                                                                                                       |
| ------ | -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| 0-2    | Interrupt Stack Table Index      | 0: Stack'leri değiştirme, 1-7: Bu handler çağrıldığında Interrupt Stack Table'daki n. stack'e geç.                              |
| 3-7    | Reserved                         |
| 8      | 0: Interrupt Gate, 1: Trap Gate  | Bu bit 0 ise, bu handler çağrıldığında interrupt'lar devre dışı bırakılır.                                                     |
| 9-11   | bir olmalı                       |
| 12     | sıfır olmalı                     |
| 13‑14  | Descriptor Privilege Level (DPL) | Bu handler'ı çağırmak için gereken minimum ayrıcalık seviyesi.                                                                  |
| 15     | Present                          |

Her exception'ın önceden tanımlanmış bir IDT indeksi vardır. Örneğin, invalid opcode exception'ının tablo indeksi 6, page fault exception'ının ise tablo indeksi 14'tür. Böylece donanım, her exception için karşılık gelen IDT girdisini otomatik olarak yükleyebilir. OSDev wiki'sindeki [Exception Table][exceptions], tüm exception'ların IDT indekslerini "Vector nr." sütununda gösterir.

Bir exception meydana geldiğinde, CPU kabaca şunları yapar:

1. Komut işaretçisi (instruction pointer) ve [RFLAGS] register'ı dahil olmak üzere bazı register'ları stack'e push'lar. (Bu değerleri bu yazının ilerleyen kısmında kullanacağız.)
2. Interrupt Descriptor Table'dan (IDT) karşılık gelen girdiyi okur. Örneğin, bir page fault meydana geldiğinde CPU 14. girdiyi okur.
3. Girdinin mevcut olup olmadığını kontrol eder ve değilse bir double fault yükseltir.
4. Girdi bir interrupt gate ise (bit 40 ayarlanmamışsa) donanım interrupt'larını devre dışı bırakır.
5. Belirtilen [GDT] seçicisini CS'ye (kod segmenti) yükler.
6. Belirtilen handler fonksiyonuna atlar.

[RFLAGS]: https://en.wikipedia.org/wiki/FLAGS_register
[GDT]: https://en.wikipedia.org/wiki/Global_Descriptor_Table

Şimdilik 4. ve 5. adımlar için endişelenmeyin; global descriptor table'ı ve donanım interrupt'larını gelecekteki yazılarda öğreneceğiz.

## Bir IDT Tipi
Kendi IDT tipimizi oluşturmak yerine, `x86_64` crate'inin şöyle görünen [`InterruptDescriptorTable` struct'ını][`InterruptDescriptorTable` struct] kullanacağız:

[`InterruptDescriptorTable` struct]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html

```rust
#[repr(C)]
pub struct InterruptDescriptorTable {
    pub divide_by_zero: Entry<HandlerFunc>,
    pub debug: Entry<HandlerFunc>,
    pub non_maskable_interrupt: Entry<HandlerFunc>,
    pub breakpoint: Entry<HandlerFunc>,
    pub overflow: Entry<HandlerFunc>,
    pub bound_range_exceeded: Entry<HandlerFunc>,
    pub invalid_opcode: Entry<HandlerFunc>,
    pub device_not_available: Entry<HandlerFunc>,
    pub double_fault: Entry<HandlerFuncWithErrCode>,
    pub invalid_tss: Entry<HandlerFuncWithErrCode>,
    pub segment_not_present: Entry<HandlerFuncWithErrCode>,
    pub stack_segment_fault: Entry<HandlerFuncWithErrCode>,
    pub general_protection_fault: Entry<HandlerFuncWithErrCode>,
    pub page_fault: Entry<PageFaultHandlerFunc>,
    pub x87_floating_point: Entry<HandlerFunc>,
    pub alignment_check: Entry<HandlerFuncWithErrCode>,
    pub machine_check: Entry<HandlerFunc>,
    pub simd_floating_point: Entry<HandlerFunc>,
    pub virtualization: Entry<HandlerFunc>,
    pub security_exception: Entry<HandlerFuncWithErrCode>,
    // bazı alanlar atlandı
}
```

Alanlar, bir IDT girdisinin alanlarını temsil eden bir struct olan [`idt::Entry<F>`] tipine sahiptir (yukarıdaki tabloya bakın). Tip parametresi `F`, beklenen handler fonksiyon tipini tanımlar. Bazı girdilerin bir [`HandlerFunc`], bazı girdilerin ise bir [`HandlerFuncWithErrCode`] gerektirdiğini görüyoruz. Page fault'un kendine ait özel bir tipi bile var: [`PageFaultHandlerFunc`].

[`idt::Entry<F>`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.Entry.html
[`HandlerFunc`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.HandlerFunc.html
[`HandlerFuncWithErrCode`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.HandlerFuncWithErrCode.html
[`PageFaultHandlerFunc`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.PageFaultHandlerFunc.html

Önce `HandlerFunc` tipine bakalım:

```rust
type HandlerFunc = extern "x86-interrupt" fn(_: InterruptStackFrame);
```

Bu, bir `extern "x86-interrupt" fn` tipi için bir [tip takma adıdır (type alias)][type alias]. `extern` anahtar kelimesi, [yabancı bir çağırma kuralına (foreign calling convention)][foreign calling convention] sahip bir fonksiyon tanımlar ve genellikle C koduyla iletişim kurmak için kullanılır (`extern "C" fn`). Peki `x86-interrupt` çağırma kuralı nedir?

[type alias]: https://doc.rust-lang.org/book/ch20-03-advanced-types.html#creating-type-synonyms-with-type-aliases
[foreign calling convention]: https://doc.rust-lang.org/nomicon/ffi.html#foreign-calling-conventions

## Interrupt Çağırma Kuralı
Exception'lar fonksiyon çağrılarına oldukça benzer: CPU, çağrılan fonksiyonun ilk komutuna atlar ve onu çalıştırır. Sonrasında CPU dönüş adresine atlar ve üst fonksiyonun çalıştırılmasına devam eder.

Ancak exception'lar ile fonksiyon çağrıları arasında büyük bir fark vardır: Bir fonksiyon çağrısı, derleyici tarafından eklenen bir `call` komutuyla gönüllü olarak başlatılır; bir exception ise _herhangi_ bir komutta meydana gelebilir. Bu farkın sonuçlarını anlamak için, fonksiyon çağrılarını daha ayrıntılı incelememiz gerekir.

[Çağırma kuralları (calling conventions)][Calling conventions] bir fonksiyon çağrısının ayrıntılarını belirtir. Örneğin, fonksiyon parametrelerinin nereye yerleştirildiğini (örneğin register'lara mı yoksa stack'e mi) ve sonuçların nasıl döndürüldüğünü belirtirler. x86_64 Linux'ta, C fonksiyonları için aşağıdaki kurallar geçerlidir ([System V ABI]'de belirtilmiştir):

[Calling conventions]: https://en.wikipedia.org/wiki/Calling_convention
[System V ABI]: https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf

- ilk altı tamsayı argümanı `rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9` register'larında geçirilir
- ek argümanlar stack'te geçirilir
- sonuçlar `rax` ve `rdx`'te döndürülür

Rust'ın C ABI'sini takip etmediğini unutmayın (aslında [henüz bir Rust ABI'si bile yok][rust abi]), bu yüzden bu kurallar yalnızca `extern "C" fn` olarak bildirilen fonksiyonlar için geçerlidir.

[rust abi]: https://github.com/rust-lang/rfcs/issues/600

### Korunan ve Scratch Register'lar
Çağırma kuralı register'ları iki bölüme ayırır: _korunan (preserved)_ ve _scratch_ register'lar.

_Korunan_ register'ların değerleri fonksiyon çağrıları boyunca değişmeden kalmalıdır. Yani çağrılan bir fonksiyonun (_"callee"_) bu register'ların üzerine yazmasına yalnızca, geri dönmeden önce orijinal değerlerini geri yüklemesi koşuluyla izin verilir. Bu nedenle bu register'lara _"callee-saved"_ denir. Yaygın bir örüntü, bu register'ları fonksiyonun başında stack'e kaydetmek ve geri dönmeden hemen önce geri yüklemektir.

Buna karşılık, çağrılan bir fonksiyonun _scratch_ register'ların üzerine kısıtlama olmadan yazmasına izin verilir. Çağıran fonksiyon (caller), bir scratch register'ının değerini bir fonksiyon çağrısı boyunca korumak isterse, onu fonksiyon çağrısından önce yedeklemesi ve geri yüklemesi gerekir (örneğin onu stack'e push'layarak). Yani scratch register'lar _caller-saved_'dır.

x86_64'te, C çağırma kuralı aşağıdaki korunan ve scratch register'ları belirtir:

| korunan register'lar                            | scratch register'lar                                        |
| ----------------------------------------------- | ----------------------------------------------------------- |
| `rbp`, `rbx`, `rsp`, `r12`, `r13`, `r14`, `r15` | `rax`, `rcx`, `rdx`, `rsi`, `rdi`, `r8`, `r9`, `r10`, `r11` |
| _callee-saved_                                  | _caller-saved_                                              |

Derleyici bu kuralları bilir, bu yüzden kodu buna göre üretir. Örneğin, çoğu fonksiyon `rbp`'yi stack'e yedekleyen bir `push rbp` ile başlar (çünkü o bir callee-saved register'dır).

### Tüm Register'ları Korumak
Fonksiyon çağrılarının aksine, exception'lar _herhangi_ bir komutta meydana gelebilir. Çoğu durumda, üretilen kodun bir exception'a neden olup olmayacağını derleme zamanında bile bilemeyiz. Örneğin, derleyici bir komutun stack taşmasına mı yoksa page fault'a mı neden olduğunu bilemez.

Bir exception'ın ne zaman meydana geleceğini bilmediğimiz için, öncesinde hiçbir register'ı yedekleyemeyiz. Bu, exception handler'lar için caller-saved register'lara dayanan bir çağırma kuralı kullanamayacağımız anlamına gelir. Bunun yerine, _tüm register'ları_ koruyan bir çağırma kuralına ihtiyacımız var. `x86-interrupt` çağırma kuralı böyle bir çağırma kuralıdır, bu yüzden fonksiyon dönüşünde tüm register değerlerinin orijinal değerlerine geri yüklenmesini garanti eder.

Bunun, tüm register'ların fonksiyon girişinde stack'e kaydedildiği anlamına gelmediğini unutmayın. Bunun yerine, derleyici yalnızca fonksiyon tarafından üzerine yazılan register'ları yedekler. Bu sayede, yalnızca birkaç register kullanan kısa fonksiyonlar için çok verimli kod üretilebilir.

### Interrupt Stack Frame {#the-interrupt-stack-frame}
Normal bir fonksiyon çağrısında (`call` komutu kullanılarak), CPU hedef fonksiyona atlamadan önce dönüş adresini push'lar. Fonksiyon dönüşünde (`ret` komutu kullanılarak), CPU bu dönüş adresini pop'lar ve ona atlar. Yani normal bir fonksiyon çağrısının stack frame'i şöyle görünür:

![fonksiyon stack frame'i](function-stack-frame.svg)

Ancak exception ve interrupt handler'ları için, yalnızca bir dönüş adresi push'lamak yeterli olmazdı; çünkü interrupt handler'ları genellikle farklı bir bağlamda (stack pointer, CPU bayrakları vb.) çalışır. Bunun yerine, bir interrupt meydana geldiğinde CPU aşağıdaki adımları gerçekleştirir:

0. **Eski stack pointer'ı kaydetmek**: CPU, stack pointer (`rsp`) ve stack segment (`ss`) register değerlerini okur ve onları dahili bir arabellekte hatırlar.
1. **Stack pointer'ı hizalamak**: Bir interrupt herhangi bir komutta meydana gelebilir, bu yüzden stack pointer da herhangi bir değere sahip olabilir. Ancak bazı CPU komutları (örneğin bazı SSE komutları) stack pointer'ın 16 baytlık bir sınırda hizalanmasını gerektirir, bu yüzden CPU interrupt'tan hemen sonra böyle bir hizalama gerçekleştirir.
2. **Stack'leri değiştirmek** (bazı durumlarda): CPU ayrıcalık seviyesi değiştiğinde bir stack değişimi meydana gelir; örneğin, bir kullanıcı modu programında bir CPU exception'ı oluştuğunda. _Interrupt Stack Table_ adı verilen şey kullanılarak (bir sonraki yazıda açıklanacak) belirli interrupt'lar için stack değişimleri yapılandırmak da mümkündür.
3. **Eski stack pointer'ı push'lamak**: CPU, 0. adımdaki `rsp` ve `ss` değerlerini stack'e push'lar. Bu, bir interrupt handler'dan dönerken orijinal stack pointer'ın geri yüklenmesini mümkün kılar.
4. **`RFLAGS` register'ını push'lamak ve güncellemek**: [`RFLAGS`] register'ı çeşitli kontrol ve durum bitleri içerir. Interrupt girişinde, CPU bazı bitleri değiştirir ve eski değeri push'lar.
5. **Komut işaretçisini push'lamak**: Interrupt handler fonksiyonuna atlamadan önce, CPU komut işaretçisini (`rip`) ve kod segmentini (`cs`) push'lar. Bu, normal bir fonksiyon çağrısının dönüş adresi push'lamasına benzer.
6. **Bir hata kodu push'lamak** (bazı exception'lar için): Page fault gibi bazı belirli exception'lar için, CPU exception'ın nedenini açıklayan bir hata kodu push'lar.
7. **Interrupt handler'ı çağırmak**: CPU, interrupt handler fonksiyonunun adresini ve segment tanımlayıcısını IDT'deki karşılık gelen alandan okur. Ardından, bu değerleri `rip` ve `cs` register'larına yükleyerek bu handler'ı çağırır.

[`RFLAGS`]: https://en.wikipedia.org/wiki/FLAGS_register

Yani _interrupt stack frame_ şöyle görünür:

![interrupt stack frame'i](exception-stack-frame.svg)

`x86_64` crate'inde, interrupt stack frame [`InterruptStackFrame`] struct'ı ile temsil edilir. Interrupt handler'larına `&mut` olarak geçirilir ve exception'ın nedeni hakkında ek bilgi almak için kullanılabilir. Yalnızca birkaç exception bir hata kodu push'ladığından, struct'ta hata kodu alanı yoktur. Bu exception'lar, ek bir `error_code` argümanına sahip olan ayrı [`HandlerFuncWithErrCode`] fonksiyon tipini kullanır.

[`InterruptStackFrame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptStackFrame.html

### Perde Arkası
`x86-interrupt` çağırma kuralı, exception işleme sürecinin neredeyse tüm dağınık ayrıntılarını gizleyen güçlü bir soyutlamadır. Ancak bazen perde arkasında ne olduğunu bilmek yararlıdır. İşte `x86-interrupt` çağırma kuralının hallettiği şeylerin kısa bir özeti:

- **Argümanları almak**: Çoğu çağırma kuralı, argümanların register'larda geçirilmesini bekler. Bu, exception handler'lar için mümkün değildir, çünkü register değerlerini stack'e yedeklemeden önce hiçbirinin üzerine yazmamalıyız. Bunun yerine, `x86-interrupt` çağırma kuralı, argümanların belirli bir ofsette zaten stack'te bulunduğunun farkındadır.
- **`iretq` kullanarak geri dönmek**: Interrupt stack frame, normal fonksiyon çağrılarının stack frame'lerinden tamamen farklı olduğundan, handler fonksiyonlarından normal `ret` komutuyla geri dönemeyiz. Bu yüzden bunun yerine `iretq` komutunun kullanılması gerekir.
- **Hata kodunu işlemek**: Bazı exception'lar için push'lanan hata kodu işleri çok daha karmaşık hale getirir. Stack hizalamasını değiştirir (sonraki maddeye bakın) ve geri dönmeden önce stack'ten pop'lanması gerekir. `x86-interrupt` çağırma kuralı tüm bu karmaşıklığı halleder. Ancak hangi handler fonksiyonunun hangi exception için kullanıldığını bilmez, bu yüzden bu bilgiyi fonksiyon argümanlarının sayısından çıkarması gerekir. Bu da, her exception için doğru fonksiyon tipini kullanmaktan programcının hâlâ sorumlu olduğu anlamına gelir. Neyse ki, `x86_64` crate'i tarafından tanımlanan `InterruptDescriptorTable` tipi doğru fonksiyon tiplerinin kullanılmasını sağlar.
- **Stack'i hizalamak**: Bazı komutlar (özellikle SSE komutları) 16 baytlık bir stack hizalaması gerektirir. CPU, bir exception meydana geldiğinde bu hizalamayı sağlar, ancak bazı exception'lar için bir hata kodu push'ladığında onu daha sonra tekrar bozar. `x86-interrupt` çağırma kuralı, bu durumda stack'i yeniden hizalayarak bununla ilgilenir.

Daha fazla ayrıntıyla ilgileniyorsanız, exception işlemeyi [naked fonksiyonlar][naked functions] kullanarak açıklayan ve [bu yazının sonunda][too-much-magic] bağlantısı verilen bir yazı dizimiz de var.

[naked functions]: https://github.com/rust-lang/rfcs/blob/master/text/1201-naked-fns.md
[too-much-magic]: #too-much-magic

## Uygulama {#implementation}
Artık teoriyi anladığımıza göre, kernel'imizde CPU exception'larını işlemenin zamanı geldi. `src/interrupts.rs` dosyasında, önce yeni bir `InterruptDescriptorTable` oluşturan bir `init_idt` fonksiyonu oluşturan yeni bir interrupts modülü oluşturarak başlayacağız:

```rust
// src/lib.rs içinde

pub mod interrupts;

// src/interrupts.rs içinde

use x86_64::structures::idt::InterruptDescriptorTable;

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
}
```

Artık handler fonksiyonları ekleyebiliriz. [Breakpoint exception][breakpoint exception] için bir handler ekleyerek başlıyoruz. Breakpoint exception'ı, exception işlemeyi test etmek için mükemmel bir exception'dır. Tek amacı, `int3` breakpoint komutu çalıştırıldığında bir programı geçici olarak duraklatmaktır.

[breakpoint exception]: https://wiki.osdev.org/Exceptions#Breakpoint

Breakpoint exception'ı genellikle hata ayıklayıcılarda (debugger) kullanılır: Kullanıcı bir breakpoint belirlediğinde, hata ayıklayıcı karşılık gelen komutun üzerine `int3` komutunu yazar; böylece CPU o satıra ulaştığında breakpoint exception'ını fırlatır. Kullanıcı programa devam etmek istediğinde, hata ayıklayıcı `int3` komutunu tekrar orijinal komutla değiştirir ve programa devam eder. Daha fazla ayrıntı için ["_Hata ayıklayıcılar nasıl çalışır_"]["_How debuggers work_"] dizisine bakın.

["_How debuggers work_"]: https://eli.thegreenplace.net/2011/01/27/how-debuggers-work-part-2-breakpoints

Bizim kullanım senaryomuz için hiçbir komutun üzerine yazmamıza gerek yok. Bunun yerine, yalnızca breakpoint komutu çalıştırıldığında bir mesaj yazdırmak ve ardından programa devam etmek istiyoruz. O halde basit bir `breakpoint_handler` fonksiyonu oluşturup onu IDT'mize ekleyelim:

```rust
// src/interrupts.rs içinde

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::println;

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}
```

Handler'ımız yalnızca bir mesaj çıktılar ve interrupt stack frame'i okunaklı biçimde yazdırır.

Onu derlemeye çalıştığımızda, aşağıdaki hata oluşur:

```
error[E0658]: x86-interrupt ABI is experimental and subject to change (see issue #40180)
  --> src/main.rs:53:1
   |
53 | / extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
54 | |     println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
55 | | }
   | |_^
   |
   = help: add #![feature(abi_x86_interrupt)] to the crate attributes to enable
```

Bu hata, `x86-interrupt` çağırma kuralının hâlâ kararsız olması nedeniyle oluşur. Yine de onu kullanmak için, `lib.rs` dosyamızın başına `#![feature(abi_x86_interrupt)]` ekleyerek onu açıkça etkinleştirmemiz gerekir.

### IDT'yi Yüklemek {#loading-the-idt}
CPU'nun yeni interrupt descriptor table'ımızı kullanabilmesi için, onu [`lidt`] komutunu kullanarak yüklememiz gerekir. `x86_64` crate'inin `InterruptDescriptorTable` struct'ı bunun için bir [`load`][InterruptDescriptorTable::load] metodu sağlar. Onu kullanmayı deneyelim:

[`lidt`]: https://www.felixcloutier.com/x86/lgdt:lidt
[InterruptDescriptorTable::load]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html#method.load

```rust
// src/interrupts.rs içinde

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.load();
}
```

Onu şimdi derlemeye çalıştığımızda, aşağıdaki hata oluşur:

```
error: `idt` does not live long enough
  --> src/interrupts/mod.rs:43:5
   |
43 |     idt.load();
   |     ^^^ does not live long enough
44 | }
   | - borrowed value only lives until here
   |
   = note: borrowed value must be valid for the static lifetime...
```

Yani `load` metodu bir `&'static self` bekler; yani programın tüm çalışma süresi boyunca geçerli olan bir referans. Bunun nedeni, biz farklı bir IDT yükleyene kadar CPU'nun bu tabloya her interrupt'ta erişecek olmasıdır. Dolayısıyla `'static`'ten daha kısa bir ömür kullanmak, use-after-free hatalarına yol açabilir.

Aslında burada tam olarak olan budur. `idt`'miz stack'te oluşturulur, bu yüzden yalnızca `init` fonksiyonunun içinde geçerlidir. Sonrasında, stack belleği başka fonksiyonlar için yeniden kullanılır, bu yüzden CPU rastgele stack belleğini IDT olarak yorumlardı. Neyse ki, `InterruptDescriptorTable::load` metodu bu ömür gereksinimini fonksiyon tanımında kodlar; böylece Rust derleyicisi bu olası hatayı derleme zamanında önleyebilir.

Bu sorunu düzeltmek için, `idt`'mizi `'static` ömrüne sahip olduğu bir yerde saklamamız gerekir. Bunu başarmak için, IDT'mizi [`Box`] kullanarak heap'te ayırabilir ve ardından onu bir `'static` referansına dönüştürebilirdik; ancak biz bir OS kernel'i yazıyoruz ve bu yüzden (henüz) bir heap'imiz yok.

[`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html


Bir alternatif olarak, IDT'yi bir `static` olarak saklamayı deneyebilirdik:

```rust
static IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
    IDT.breakpoint.set_handler_fn(breakpoint_handler);
    IDT.load();
}
```

Ancak bir sorun var: Statics değiştirilemez (immutable), bu yüzden `init` fonksiyonumuzdan breakpoint girdisini değiştiremeyiz. Bu sorunu bir [`static mut`] kullanarak çözebilirdik:

[`static mut`]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable

```rust
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
    unsafe {
        IDT.breakpoint.set_handler_fn(breakpoint_handler);
        IDT.load();
    }
}
```

Bu varyant hatasız derlenir, ancak deyimsel (idiomatic) olmaktan çok uzaktır. `static mut`'lar veri yarışlarına çok yatkındır, bu yüzden her erişimde bir [`unsafe` bloğuna][`unsafe` block] ihtiyacımız var.

[`unsafe` block]: https://doc.rust-lang.org/1.30.0/book/second-edition/ch19-01-unsafe-rust.html#unsafe-superpowers

#### İmdada Lazy Statics Yetişiyor
Neyse ki, `lazy_static` makrosu var. Bir `static`'i derleme zamanında değerlendirmek yerine, makro başlatmayı `static`'e ilk kez referans verildiğinde gerçekleştirir. Böylece, başlatma bloğunda neredeyse her şeyi yapabilir, hatta çalışma zamanı değerlerini bile okuyabiliriz.

[VGA metin arabelleği için bir soyutlama oluşturduğumuzda][vga text buffer lazy static] `lazy_static` crate'ini zaten içe aktarmıştık. Bu yüzden statik IDT'mizi oluşturmak için doğrudan `lazy_static!` makrosunu kullanabiliriz:

[vga text buffer lazy static]: @/edition-2/posts/03-vga-text-buffer/index.tr.md#lazy-statics

```rust
// src/interrupts.rs içinde

use lazy_static::lazy_static;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}
```

Bu çözümün hiçbir `unsafe` bloğu gerektirmediğine dikkat edin. `lazy_static!` makrosu perde arkasında `unsafe` kullanır, ancak bu güvenli bir arayüzde soyutlanmıştır.

### Çalıştırmak

Kernel'imizde exception'ları çalışır hale getirmenin son adımı, `init_idt` fonksiyonunu `main.rs`'imizden çağırmaktır. Onu doğrudan çağırmak yerine, `lib.rs`'imizde genel bir `init` fonksiyonu sunuyoruz:

```rust
// src/lib.rs içinde

pub fn init() {
    interrupts::init_idt();
}
```

Bu fonksiyonla, artık `main.rs`, `lib.rs` ve entegrasyon testlerimizdeki farklı `_start` fonksiyonları arasında paylaşılabilecek başlatma rutinleri için merkezi bir yerimiz var.

Artık `main.rs`'imizin `_start` fonksiyonunu, `init`'i çağıracak ve ardından bir breakpoint exception'ı tetikleyecek şekilde güncelleyebiliriz:

```rust
// src/main.rs içinde

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init(); // yeni

    // bir breakpoint exception'ı tetikle
    x86_64::instructions::interrupts::int3(); // yeni

    // önceki gibi
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    loop {}
}
```

Onu şimdi QEMU'da çalıştırdığımızda (`cargo run` kullanarak), şunu görüyoruz:

![`EXCEPTION: BREAKPOINT` ve interrupt stack frame'i yazdıran QEMU](qemu-breakpoint-exception.png)

Çalışıyor! CPU, breakpoint handler'ımızı başarıyla çağırıyor; o mesajı yazdırıyor ve ardından `_start` fonksiyonuna geri dönüyor; burada `It did not crash!` mesajı yazdırılıyor.

Interrupt stack frame'inin bize, exception meydana geldiği andaki komut ve stack işaretçilerini söylediğini görüyoruz. Bu bilgi, beklenmeyen exception'ların hata ayıklamasında çok yararlıdır.

### Bir Test Eklemek

Yukarıdakilerin çalışmaya devam ettiğinden emin olan bir test oluşturalım. İlk olarak, `_start` fonksiyonunu `init`'i de çağıracak şekilde güncelliyoruz:

```rust
// src/lib.rs içinde

/// `cargo test` için giriş noktası
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    init();      // yeni
    test_main();
    loop {}
}
```

Unutmayın, bu `_start` fonksiyonu `cargo test --lib` çalıştırılırken kullanılır, çünkü Rust `lib.rs`'i `main.rs`'ten tamamen bağımsız olarak test eder. Testleri çalıştırmadan önce bir IDT kurmak için burada `init`'i çağırmamız gerekir.

Artık bir `test_breakpoint_exception` testi oluşturabiliriz:

```rust
// src/interrupts.rs içinde

#[test_case]
fn test_breakpoint_exception() {
    // bir breakpoint exception'ı tetikle
    x86_64::instructions::interrupts::int3();
}
```

Test, bir breakpoint exception'ı tetiklemek için `int3` fonksiyonunu çağırır. Çalıştırmanın sonrasında devam ettiğini kontrol ederek, breakpoint handler'ımızın doğru çalıştığını doğrularız.

Bu yeni testi `cargo test` (tüm testler) veya `cargo test --lib` (yalnızca `lib.rs`'in ve modüllerinin testleri) çalıştırarak deneyebilirsiniz. Çıktıda şunu görmelisiniz:

```
blog_os::interrupts::test_breakpoint_exception...	[ok]
```

## Fazla mı Sihir? {#too-much-magic}
`x86-interrupt` çağırma kuralı ve [`InterruptDescriptorTable`] tipi, exception işleme sürecini nispeten basit ve sancısız hale getirdi. Bu sizin için fazla sihir olduysa ve exception işlemenin tüm kanlı ayrıntılarını öğrenmek istiyorsanız, sizi düşündük: [“Naked Fonksiyonlarla Exception İşleme”][“Handling Exceptions with Naked Functions”] dizimiz, exception'ların `x86-interrupt` çağırma kuralı olmadan nasıl işleneceğini gösterir ve ayrıca kendi IDT tipini oluşturur. Tarihsel olarak, bu yazılar `x86-interrupt` çağırma kuralı ve `x86_64` crate'i var olmadan önceki ana exception işleme yazılarıydı. Bu yazıların bu blogun [birinci sürümüne][first edition] dayandığını ve güncel olmayabileceğini unutmayın.

[“Handling Exceptions with Naked Functions”]: @/edition-1/extra/naked-exceptions/_index.md
[`InterruptDescriptorTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html
[first edition]: @/edition-1/_index.md

## Sırada ne var?
İlk exception'ımızı başarıyla yakaladık ve ondan geri döndük! Sonraki adım, tüm exception'ları yakaladığımızdan emin olmaktır; çünkü yakalanmamış bir exception, sistem sıfırlamasına yol açan ölümcül bir [triple fault]'a neden olur. Bir sonraki yazı, [double fault'ları][double faults] doğru şekilde yakalayarak bunu nasıl önleyebileceğimizi açıklar.

[triple fault]: https://wiki.osdev.org/Triple_Fault
[double faults]: https://wiki.osdev.org/Double_Fault#Double_Fault
