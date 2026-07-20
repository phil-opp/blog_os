+++
title = "Donanım Interrupt'ları"
weight = 7
path = "tr/hardware-interrupts"
date = 2018-10-22

[extra]
chapter = "Interrupts"

# Please update this when updating the translation
translation_based_on_commit = "1132d7a3835dc6c0b3fd8f6b45c9295a9bc1f837"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

Bu yazıda, donanım interrupt'larını CPU'ya doğru şekilde iletmek için programlanabilir interrupt controller'ı kuruyoruz. Bu interrupt'ları işlemek için, tıpkı exception handler'larımız için yaptığımız gibi, interrupt descriptor table'ımıza yeni girdiler ekliyoruz. Periyodik timer interrupt'larını nasıl alacağımızı ve klavyeden nasıl girdi alacağımızı öğreneceğiz.

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-07`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-07

<!-- toc -->

## Genel Bakış

Interrupt'lar, bağlı donanım cihazlarından CPU'yu bilgilendirmenin bir yolunu sağlar. Yani, kernel'in klavyeyi yeni karakterler için periyodik olarak kontrol etmesine ([_yoklama (polling)_][_polling_] adı verilen bir süreç) izin vermek yerine, klavye her tuş basışını kernel'e bildirebilir. Bu çok daha verimlidir, çünkü kernel'in yalnızca bir şey olduğunda harekete geçmesi gerekir. Ayrıca, kernel yalnızca bir sonraki yoklamada değil, hemen tepki verebileceği için daha hızlı tepki sürelerine de olanak tanır.

[_polling_]: https://en.wikipedia.org/wiki/Polling_(computer_science)

Tüm donanım cihazlarını doğrudan CPU'ya bağlamak mümkün değildir. Bunun yerine, ayrı bir _interrupt controller_ tüm cihazlardan gelen interrupt'ları toplar ve ardından CPU'yu bilgilendirir:

```
                                    ____________             _____
               Timer ------------> |            |           |     |
               Keyboard ---------> | Interrupt  |---------> | CPU |
               Other Hardware ---> | Controller |           |_____|
               Etc. -------------> |____________|

```

Çoğu interrupt controller programlanabilirdir; bu da interrupt'lar için farklı öncelik seviyelerini destekledikleri anlamına gelir. Örneğin bu, doğru zaman tutmayı sağlamak için timer interrupt'larına klavye interrupt'larından daha yüksek bir öncelik vermeye olanak tanır.

Exception'ların aksine, donanım interrupt'ları _asenkron_ olarak meydana gelir. Bu, çalıştırılan koddan tamamen bağımsız oldukları ve herhangi bir zamanda meydana gelebilecekleri anlamına gelir. Böylece, kernel'imizde aniden tüm potansiyel eşzamanlılıkla (concurrency) ilgili hatalarıyla birlikte bir tür eşzamanlılık ortaya çıkar. Rust'ın katı sahiplik modeli burada bize yardımcı olur, çünkü değiştirilebilir global durumu yasaklar. Ancak deadlock'lar hâlâ mümkündür; bu yazının ilerleyen kısımlarında göreceğimiz gibi.

## 8259 PIC {#the-8259-pic}

[Intel 8259], 1976'da tanıtılan programlanabilir bir interrupt controller'dır (PIC). Uzun süre önce yerini daha yeni [APIC] almıştır, ancak arayüzü geriye dönük uyumluluk nedenleriyle güncel sistemlerde hâlâ desteklenir. 8259 PIC'in kurulumu APIC'ten önemli ölçüde daha kolaydır, bu yüzden daha sonraki bir yazıda APIC'e geçmeden önce interrupt'larla tanışmak için onu kullanacağız.

[APIC]: https://en.wikipedia.org/wiki/Intel_APIC_Architecture

8259'un sekiz interrupt hattı ve CPU ile iletişim kurmak için çeşitli hatları vardır. O zamanların tipik sistemleri, 8259 PIC'in iki örneğiyle donatılmıştı; biri birincil (primary), biri ikincil (secondary) PIC olmak üzere, ikincisi birincilin interrupt hatlarından birine bağlıydı:

[Intel 8259]: https://en.wikipedia.org/wiki/Intel_8259

```
                     ____________                          ____________
Real Time Clock --> |            |   Timer -------------> |            |
ACPI -------------> |            |   Keyboard-----------> |            |      _____
Available --------> | Secondary  |----------------------> | Primary    |     |     |
Available --------> | Interrupt  |   Serial Port 2 -----> | Interrupt  |---> | CPU |
Mouse ------------> | Controller |   Serial Port 1 -----> | Controller |     |_____|
Co-Processor -----> |            |   Parallel Port 2/3 -> |            |
Primary ATA ------> |            |   Floppy disk -------> |            |
Secondary ATA ----> |____________|   Parallel Port 1----> |____________|

```

Bu grafik, interrupt hatlarının tipik atamasını gösterir. 15 hattın çoğunun sabit bir eşlemesi olduğunu görüyoruz; örneğin, ikincil PIC'in 4. hattı fareye atanmıştır.

Her controller iki [I/O portu][I/O ports] aracılığıyla yapılandırılabilir: bir "komut" portu ve bir "veri" portu. Birincil controller için bu portlar `0x20` (komut) ve `0x21`'dir (veri). İkincil controller için ise `0xa0` (komut) ve `0xa1`'dir (veri). PIC'lerin nasıl yapılandırılabileceği hakkında daha fazla bilgi için [osdev.org'daki makaleye][article on osdev.org] bakın.

[I/O ports]: @/edition-2/posts/04-testing/index.tr.md#i-o-ports
[article on osdev.org]: https://wiki.osdev.org/8259_PIC

### Uygulama

PIC'lerin varsayılan yapılandırması kullanılabilir değildir, çünkü CPU'ya 0–15 aralığında interrupt vektör numaraları gönderir. Bu numaralar zaten CPU exception'ları tarafından işgal edilmiştir. Örneğin, 8 numarası bir double fault'a karşılık gelir. Bu çakışma sorununu düzeltmek için, PIC interrupt'larını farklı numaralara yeniden eşlememiz (remap) gerekir. Exception'larla çakışmadığı sürece gerçek aralık önemli değildir, ancak tipik olarak 32–47 aralığı seçilir; çünkü bunlar 32 exception yuvasından sonraki ilk boş numaralardır.

Yapılandırma, PIC'lerin komut ve veri portlarına özel değerler yazılarak gerçekleşir. Neyse ki, [`pic8259`] adında zaten bir crate var, bu yüzden başlatma dizisini kendimiz yazmamıza gerek yok. Ancak nasıl çalıştığıyla ilgileniyorsanız, [kaynak koduna][pic crate source] göz atın. Oldukça küçük ve iyi belgelenmiştir.

[pic crate source]: https://docs.rs/crate/pic8259/0.10.1/source/src/lib.rs

Crate'i bir bağımlılık olarak eklemek için, projemize aşağıdakini ekliyoruz:

[`pic8259`]: https://docs.rs/pic8259/0.11.0/pic8259/

```toml
# Cargo.toml içinde

[dependencies]
pic8259 = "0.11.0"
```

Crate'in sağladığı ana soyutlama, yukarıda gördüğümüz birincil/ikincil PIC düzenini temsil eden [`ChainedPics`] struct'ıdır. Aşağıdaki şekilde kullanılmak üzere tasarlanmıştır:

[`ChainedPics`]: https://docs.rs/pic8259/0.11.0/pic8259/struct.ChainedPics.html

```rust
// src/interrupts.rs içinde

use pic8259::ChainedPics;
use spin;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });
```

Yukarıda belirtildiği gibi, PIC'lerin ofsetlerini 32–47 aralığına ayarlıyoruz. `ChainedPics` struct'ını bir `Mutex` içine sararak, bir sonraki adımda ihtiyaç duyacağımız güvenli değiştirilebilir erişimi ([`lock` metodu][spin mutex lock] aracılığıyla) elde edebiliriz. `ChainedPics::new` fonksiyonu unsafe'tir, çünkü yanlış ofsetler tanımsız davranışa neden olabilir.

[spin mutex lock]: https://docs.rs/spin/0.5.2/spin/struct.Mutex.html#method.lock

Artık 8259 PIC'i `init` fonksiyonumuzda başlatabiliriz:

```rust
// src/lib.rs içinde

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() }; // yeni
}
```

PIC başlatmasını gerçekleştirmek için [`initialize`] fonksiyonunu kullanıyoruz. `ChainedPics::new` fonksiyonu gibi, bu fonksiyon da unsafe'tir; çünkü PIC yanlış yapılandırılırsa tanımsız davranışa neden olabilir.

[`initialize`]: https://docs.rs/pic8259/0.11.0/pic8259/struct.ChainedPics.html#method.initialize

Her şey yolunda giderse, `cargo run` çalıştırdığımızda "It did not crash" mesajını görmeye devam etmeliyiz.

## Interrupt'ları Etkinleştirmek

Şimdiye kadar hiçbir şey olmadı, çünkü interrupt'lar CPU yapılandırmasında hâlâ devre dışı. Bu, CPU'nun interrupt controller'ı hiç dinlemediği anlamına gelir, bu yüzden hiçbir interrupt CPU'ya ulaşamaz. Bunu değiştirelim:

```rust
// src/lib.rs içinde

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();     // yeni
}
```

`x86_64` crate'inin `interrupts::enable` fonksiyonu, harici interrupt'ları etkinleştirmek için özel `sti` komutunu ("set interrupts") çalıştırır. Şimdi `cargo run` denediğimizde, bir double fault'un meydana geldiğini görüyoruz:

![Donanım timer'ı nedeniyle `EXCEPTION: DOUBLE FAULT` yazdıran QEMU](qemu-hardware-timer-double-fault.png)

Bu double fault'un nedeni, donanım timer'ının (tam olarak [Intel 8253]) varsayılan olarak etkin olmasıdır; bu yüzden interrupt'ları etkinleştirir etkinleştirmez timer interrupt'ları almaya başlarız. Onun için henüz bir handler fonksiyonu tanımlamadığımız için, double fault handler'ımız çağrılır.

[Intel 8253]: https://en.wikipedia.org/wiki/Intel_8253

## Timer Interrupt'larını İşlemek

[Yukarıdaki](#the-8259-pic) grafikten gördüğümüz gibi, timer birincil PIC'in 0. hattını kullanır. Bu, CPU'ya interrupt 32 (0 + ofset 32) olarak ulaştığı anlamına gelir. İndeks 32'yi koda gömmek (hardcode) yerine, onu bir `InterruptIndex` enum'ında saklıyoruz:

```rust
// src/interrupts.rs içinde

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
}
```

Enum, her varyant için indeksi doğrudan belirtebilmemiz için bir [C benzeri enum][C-like enum]'dır. `repr(u8)` özniteliği, her varyantın bir `u8` olarak temsil edildiğini belirtir. Gelecekte diğer interrupt'lar için daha fazla varyant ekleyeceğiz.

[C-like enum]: https://doc.rust-lang.org/reference/items/enumerations.html#custom-discriminant-values-for-fieldless-enumerations

Artık timer interrupt'ı için bir handler fonksiyonu ekleyebiliriz:

```rust
// src/interrupts.rs içinde

use crate::print;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        idt[InterruptIndex::Timer.as_u8()]
            .set_handler_fn(timer_interrupt_handler); // yeni

        idt
    };
}

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!(".");
}
```

`timer_interrupt_handler`'ımız, exception handler'larımızla aynı imzaya sahiptir; çünkü CPU exception'lara ve harici interrupt'lara aynı şekilde tepki verir (tek fark, bazı exception'ların bir hata kodu push'lamasıdır). [`InterruptDescriptorTable`] struct'ı [`IndexMut`] trait'ini uygular, bu yüzden tek tek girdilere dizi indeksleme söz dizimi aracılığıyla erişebiliriz.

[`InterruptDescriptorTable`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/idt/struct.InterruptDescriptorTable.html
[`IndexMut`]: https://doc.rust-lang.org/core/ops/trait.IndexMut.html

Timer interrupt handler'ımızda, ekrana bir nokta yazdırıyoruz. Timer interrupt'ı periyodik olarak gerçekleştiği için, her timer tıkında bir nokta belirmesini bekleriz. Ancak onu çalıştırdığımızda, yalnızca tek bir noktanın yazdırıldığını görüyoruz:

![Donanım timer'ı için yalnızca tek bir nokta yazdıran QEMU](qemu-single-dot-printed.png)

### Interrupt Sonu (End of Interrupt)

Bunun nedeni, PIC'in interrupt handler'ımızdan açık bir "interrupt sonu" (end of interrupt, EOI) sinyali beklemesidir. Bu sinyal controller'a, interrupt'ın işlendiğini ve sistemin bir sonraki interrupt'ı almaya hazır olduğunu söyler. Yani PIC, hâlâ ilk timer interrupt'ını işlemekle meşgul olduğumuzu düşünür ve bir sonrakini göndermeden önce sabırla EOI sinyalini bekler.

EOI'yi göndermek için yine statik `PICS` struct'ımızı kullanıyoruz:

```rust
// src/interrupts.rs içinde

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!(".");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}
```

`notify_end_of_interrupt`, interrupt'ı birincil mi yoksa ikincil PIC'in mi gönderdiğini bulur ve ardından ilgili controller'lara bir EOI sinyali göndermek için `command` ve `data` portlarını kullanır. İnterrupt'ı ikincil PIC gönderdiyse, ikincil PIC birincil PIC'in bir giriş hattına bağlı olduğu için her iki PIC'in de bilgilendirilmesi gerekir.

Doğru interrupt vektör numarasını kullanmaya dikkat etmeliyiz; aksi takdirde yanlışlıkla önemli, gönderilmemiş bir interrupt'ı silebilir veya sistemimizin asılı kalmasına neden olabiliriz. Fonksiyonun unsafe olmasının nedeni budur.

Şimdi `cargo run` çalıştırdığımızda, ekranda periyodik olarak beliren noktalar görüyoruz:

![Donanım timer'ını gösteren ardışık noktalar yazdıran QEMU](qemu-hardware-timer-dots.gif)

### Timer'ı Yapılandırmak

Kullandığımız donanım timer'ının adı kısaca _Programmable Interval Timer_ ya da PIT'tir. Adının da söylediği gibi, iki interrupt arasındaki aralığı yapılandırmak mümkündür. Yakında [APIC timer'a][APIC timer] geçeceğimiz için burada ayrıntılara girmeyeceğiz, ancak OSDev wiki'sinde [PIT'i yapılandırma][configuring the PIT] hakkında kapsamlı bir makale var.

[APIC timer]: https://wiki.osdev.org/APIC_timer
[configuring the PIT]: https://wiki.osdev.org/Programmable_Interval_Timer

## Deadlock'lar

Artık kernel'imizde bir tür eşzamanlılığımız var: Timer interrupt'ları asenkron olarak meydana gelir, bu yüzden `_start` fonksiyonumuzu herhangi bir zamanda kesebilirler. Neyse ki, Rust'ın sahiplik sistemi eşzamanlılıkla ilgili pek çok tür hatayı derleme zamanında önler. Dikkate değer bir istisna deadlock'lardır. Deadlock'lar, bir thread asla serbest kalmayacak bir kilidi (lock) almaya çalıştığında meydana gelir. Böylece thread süresiz olarak asılı kalır.

Kernel'imizde halihazırda bir deadlock'a yol açabiliriz. Hatırlayın, `println` makromuz `vga_buffer::_print` fonksiyonunu çağırır; bu fonksiyon, bir spinlock kullanarak [global bir `WRITER`'ı kilitler][vga spinlock]:

[vga spinlock]: @/edition-2/posts/03-vga-text-buffer/index.tr.md#spinlocks

```rust
// src/vga_buffer.rs içinde

[…]

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}
```

`WRITER`'ı kilitler, onun üzerinde `write_fmt` çağırır ve fonksiyonun sonunda örtük olarak kilidini açar. Şimdi, `WRITER` kilitliyken bir interrupt meydana geldiğini ve interrupt handler'ının da bir şeyler yazdırmaya çalıştığını hayal edin:

| Zaman adımı | _start                  | interrupt_handler                                       |
| ----------- | ----------------------- | ------------------------------------------------------- |
| 0           | `println!` çağırır      | &nbsp;                                                  |
| 1           | `print`, `WRITER`'ı kilitler | &nbsp;                                             |
| 2           |                         | **interrupt meydana gelir**, handler çalışmaya başlar   |
| 3           |                         | `println!` çağırır                                      |
| 4           |                         | `print`, `WRITER`'ı kilitlemeye çalışır (zaten kilitli) |
| 5           |                         | `print`, `WRITER`'ı kilitlemeye çalışır (zaten kilitli) |
| …           |                         | …                                                       |
| _asla_      | _`WRITER`'ın kilidini aç_ |

`WRITER` kilitlidir, bu yüzden interrupt handler'ı serbest kalana kadar bekler. Ancak bu asla gerçekleşmez, çünkü `_start` fonksiyonu yalnızca interrupt handler'ı geri döndükten sonra çalışmaya devam eder. Böylece tüm sistem asılı kalır.

### Bir Deadlock'a Yol Açmak

`_start` fonksiyonumuzun sonundaki döngüde bir şeyler yazdırarak kernel'imizde kolayca böyle bir deadlock'a yol açabiliriz:

```rust
// src/main.rs içinde

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    […]
    loop {
        use blog_os::print;
        print!("-");        // yeni
    }
}
```

Onu QEMU'da çalıştırdığımızda, şu biçimde bir çıktı alıyoruz:

![Birçok satır tire içeren ve hiç nokta içermeyen QEMU çıktısı](./qemu-deadlock.png)

İlk timer interrupt'ı meydana gelene kadar yalnızca sınırlı sayıda tirenin yazdırıldığını görüyoruz. Sonra sistem asılı kalır, çünkü timer interrupt handler'ı bir nokta yazdırmaya çalıştığında deadlock'a girer. Yukarıdaki çıktıda hiç nokta görmememizin nedeni budur.

Tirelerin gerçek sayısı çalıştırmalar arasında değişir, çünkü timer interrupt'ı asenkron olarak meydana gelir. Bu belirsizlik (non-determinism), eşzamanlılıkla ilgili hataların hata ayıklamasını bu kadar zorlaştıran şeydir.

### Deadlock'u Düzeltmek

Bu deadlock'tan kaçınmak için, `Mutex` kilitli olduğu sürece interrupt'ları devre dışı bırakabiliriz:

```rust
// src/vga_buffer.rs içinde

/// Verilen biçimlendirilmiş dizeyi, global `WRITER` örneği aracılığıyla
/// VGA metin arabelleğine yazdırır.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;   // yeni

    interrupts::without_interrupts(|| {     // yeni
        WRITER.lock().write_fmt(args).unwrap();
    });
}
```

[`without_interrupts`] fonksiyonu bir [closure] alır ve onu interrupt'sız bir ortamda çalıştırır. Onu, `Mutex` kilitli olduğu sürece hiçbir interrupt'ın meydana gelememesini sağlamak için kullanıyoruz. Kernel'imizi şimdi çalıştırdığımızda, asılı kalmadan çalışmaya devam ettiğini görüyoruz. (Hâlâ herhangi bir nokta fark etmiyoruz, ancak bunun nedeni çok hızlı kaymalarıdır. Yazdırmayı yavaşlatmayı deneyin; örneğin döngünün içine bir `for _ in 0..10000 {}` koyarak.)

[`without_interrupts`]: https://docs.rs/x86_64/0.15.5/x86_64/instructions/interrupts/fn.without_interrupts.html
[closure]: https://doc.rust-lang.org/book/ch13-01-closures.html

Aynı değişikliği, onunla da deadlock'ların oluşmamasını sağlamak için seri yazdırma fonksiyonumuza da uygulayabiliriz:

```rust
// src/serial.rs içinde

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;       // yeni

    interrupts::without_interrupts(|| {         // yeni
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}
```

İnterrupt'ları devre dışı bırakmanın genel bir çözüm olmaması gerektiğine dikkat edin. Sorun, en kötü durum interrupt gecikmesini, yani sistemin bir interrupt'a tepki verene kadar geçen süreyi artırmasıdır. Bu nedenle, interrupt'lar yalnızca çok kısa bir süre için devre dışı bırakılmalıdır.

## Bir Race Condition'ı Düzeltmek

`cargo test` çalıştırırsanız, `test_println_output` testinin başarısız olduğunu görebilirsiniz:

```
> cargo test --lib
[…]
Running 4 tests
test_breakpoint_exception...[ok]
test_println... [ok]
test_println_many... [ok]
test_println_output... [failed]

Error: panicked at 'assertion failed: `(left == right)`
  left: `'.'`,
 right: `'S'`', src/vga_buffer.rs:205:9
```

Bunun nedeni, test ile timer handler'ımız arasındaki bir _race condition_'dır. Hatırlayın, test şöyle görünür:

```rust
// src/vga_buffer.rs içinde

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

Test, VGA arabelleğine bir dize yazdırır ve ardından `buffer_chars` dizisi üzerinde elle iterasyon yaparak çıktıyı kontrol eder. Race condition, timer interrupt handler'ının `println` ile ekran karakterlerinin okunması arasında çalışabilmesi nedeniyle meydana gelir. Bunun, Rust'ın derleme zamanında tamamen önlediği tehlikeli bir _veri yarışı (data race)_ olmadığını unutmayın. Ayrıntılar için [_Rustonomicon_'a][nomicon-races] bakın.

[nomicon-races]: https://doc.rust-lang.org/nomicon/races.html

Bunu düzeltmek için, `WRITER`'ı testin tüm süresi boyunca kilitli tutmamız gerekir; böylece timer handler'ı arada ekrana bir `.` yazamaz. Düzeltilmiş test şöyle görünür:

```rust
// src/vga_buffer.rs içinde

#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}
```

Aşağıdaki değişiklikleri yaptık:

- `lock()` metodunu açıkça kullanarak writer'ı testin tamamı boyunca kilitli tutuyoruz. `println` yerine, zaten kilitli bir writer'a yazdırmaya olanak tanıyan [`writeln`] makrosunu kullanıyoruz.
- Başka bir deadlock'tan kaçınmak için, testin süresi boyunca interrupt'ları devre dışı bırakıyoruz. Aksi takdirde, writer hâlâ kilitliyken test kesintiye uğrayabilir.
- Timer interrupt handler'ı testten önce hâlâ çalışabileceği için, `s` dizesini yazdırmadan önce ek bir yeni satır `\n` yazdırıyoruz. Bu sayede, timer handler'ı mevcut satıra zaten bazı `.` karakterleri yazdırmışsa test başarısızlığından kaçınırız.

[`writeln`]: https://doc.rust-lang.org/core/macro.writeln.html

Yukarıdaki değişikliklerle, `cargo test` artık deterministik olarak yeniden başarılı oluyor.

Bu, yalnızca bir test başarısızlığına neden olan çok zararsız bir race condition'dı. Tahmin edebileceğiniz gibi, diğer race condition'lar belirsiz (non-deterministic) doğaları nedeniyle hata ayıklaması çok daha zor olabilir. Neyse ki Rust, race condition'ların en ciddi sınıfı olan veri yarışlarından bizi korur; çünkü bunlar sistem çökmeleri ve sessiz bellek bozulmaları dahil her türlü tanımsız davranışa neden olabilir.

## `hlt` Komutu {#the-hlt-instruction}

Şimdiye kadar, `_start` ve `panic` fonksiyonlarımızın sonunda basit, boş bir döngü ifadesi kullandık. Bu, CPU'nun sonsuza dek dönmesine neden olur ve böylece beklendiği gibi çalışır. Ancak aynı zamanda çok verimsizdir, çünkü yapacak iş olmamasına rağmen CPU tam hızda çalışmaya devam eder. Kernel'inizi çalıştırdığınızda bu sorunu görev yöneticinizde görebilirsiniz: QEMU süreci her zaman %100'e yakın CPU'ya ihtiyaç duyar.

Gerçekten yapmak istediğimiz şey, bir sonraki interrupt gelene kadar CPU'yu durdurmaktır (halt). Bu, CPU'nun çok daha az enerji tükettiği bir uyku durumuna girmesine olanak tanır. [`hlt` komutu][`hlt` instruction] tam olarak bunu yapar. Enerji açısından verimli bir sonsuz döngü oluşturmak için bu komutu kullanalım:

[`hlt` instruction]: https://en.wikipedia.org/wiki/HLT_(x86_instruction)

```rust
// src/lib.rs içinde

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
```

`instructions::hlt` fonksiyonu, yalnızca assembly komutunun etrafındaki [ince bir sarmalayıcıdır (thin wrapper)][thin wrapper]. Güvenlidir, çünkü bellek güvenliğini tehlikeye atmasının hiçbir yolu yoktur.

[thin wrapper]: https://github.com/rust-osdev/x86_64/blob/5e8e218381c5205f5777cb50da3ecac5d7e3b1ab/src/instructions/mod.rs#L16-L22

Artık bu `hlt_loop`'u `_start` ve `panic` fonksiyonlarımızdaki sonsuz döngüler yerine kullanabiliriz:

```rust
// src/main.rs içinde

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    […]

    println!("It did not crash!");
    blog_os::hlt_loop();            // yeni
}


#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    blog_os::hlt_loop();            // yeni
}

```

`lib.rs`'imizi de güncelleyelim:

```rust
// src/lib.rs içinde

/// `cargo test` için giriş noktası
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    init();
    test_main();
    hlt_loop();         // yeni
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();         // yeni
}
```

Kernel'imizi şimdi QEMU'da çalıştırdığımızda, çok daha düşük bir CPU kullanımı görüyoruz.

## Klavye Girişi

Artık harici cihazlardan gelen interrupt'ları işleyebildiğimize göre, nihayet klavye girişi için destek ekleyebiliriz. Bu, kernel'imizle ilk kez etkileşim kurmamıza olanak tanıyacak.

<aside class="post_aside">

Burada yalnızca [PS/2] klavyelerin nasıl işleneceğini açıkladığımızı, USB klavyeleri değil, unutmayın. Ancak anakart, eski yazılımı desteklemek için USB klavyeleri PS/2 cihazları olarak öykünür (emulate), bu yüzden kernel'imizde USB desteğine sahip olana kadar USB klavyeleri güvenle göz ardı edebiliriz.

</aside>

[PS/2]: https://en.wikipedia.org/wiki/PS/2_port

Donanım timer'ı gibi, klavye controller'ı da varsayılan olarak zaten etkindir. Yani bir tuşa bastığınızda, klavye controller'ı PIC'e bir interrupt gönderir; o da onu CPU'ya iletir. CPU IDT'de bir handler fonksiyonu arar, ancak karşılık gelen girdi boştur. Bu nedenle bir double fault meydana gelir.

O halde klavye interrupt'ı için bir handler fonksiyonu ekleyelim. Timer interrupt'ı için handler'ı tanımlama şeklimize oldukça benzer; yalnızca farklı bir interrupt numarası kullanır:

```rust
// src/interrupts.rs içinde

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // yeni
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        // yeni
        idt[InterruptIndex::Keyboard.as_u8()]
            .set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!("k");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

[Yukarıdaki](#the-8259-pic) grafikten gördüğümüz gibi, klavye birincil PIC'in 1. hattını kullanır. Bu, CPU'ya interrupt 33 (1 + ofset 32) olarak ulaştığı anlamına gelir. Bu indeksi `InterruptIndex` enum'ına yeni bir `Keyboard` varyantı olarak ekliyoruz. Değeri açıkça belirtmemize gerek yok, çünkü varsayılan olarak bir önceki değerin bir fazlasıdır; bu da yine 33'tür. Interrupt handler'ında bir `k` yazdırıyor ve interrupt controller'a interrupt sonu sinyalini gönderiyoruz.

Şimdi, bir tuşa bastığımızda ekranda bir `k` belirdiğini görüyoruz. Ancak bu yalnızca bastığımız ilk tuş için çalışıyor. Tuşlara basmaya devam etsek bile, ekranda daha fazla `k` belirmiyor. Bunun nedeni, basılan tuşun _scancode_ adı verilen kodunu okuyana kadar klavye controller'ının başka bir interrupt göndermeyecek olmasıdır.

### Scancode'ları Okumak

_Hangi_ tuşa basıldığını öğrenmek için, klavye controller'ını sorgulamamız gerekir. Bunu, PS/2 controller'ının veri portundan, yani `0x60` numaralı [I/O portundan][I/O port] okuyarak yaparız:

[I/O port]: @/edition-2/posts/04-testing/index.tr.md#i-o-ports

```rust
// src/interrupts.rs içinde

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    print!("{}", scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

Klavyenin veri portundan bir bayt okumak için `x86_64` crate'inin [`Port`] tipini kullanıyoruz. Bu bayta [_scancode_] denir ve tuş basışını/bırakışını temsil eder. Scancode ile şimdilik, onu ekrana yazdırmak dışında bir şey yapmıyoruz:

[`Port`]: https://docs.rs/x86_64/0.15.5/x86_64/instructions/port/type.Port.html
[_scancode_]: https://en.wikipedia.org/wiki/Scancode

![Tuşlara basıldığında ekrana scancode'ları yazdıran QEMU](qemu-printing-scancodes.gif)

Yukarıdaki görsel, yavaşça "123" yazdığımı gösteriyor. Bitişik tuşların bitişik scancode'lara sahip olduğunu ve bir tuşa basmanın, onu bırakmaktan farklı bir scancode'a neden olduğunu görüyoruz. Peki scancode'ları gerçek tuş eylemlerine tam olarak nasıl çeviririz?

### Scancode'ları Yorumlamak {#interpreting-the-scancodes}
Scancode'lar ile tuşlar arasındaki eşleme için üç farklı standart vardır; bunlara _scancode setleri_ denir. Üçü de erken IBM bilgisayarlarının klavyelerine dayanır: [IBM XT], [IBM 3270 PC] ve [IBM AT]. Sonraki bilgisayarlar neyse ki yeni scancode setleri tanımlama eğilimini sürdürmedi, bunun yerine mevcut setleri öykünüp (emulate) genişletti. Günümüzde, çoğu klavye üç setten herhangi birini öykünecek şekilde yapılandırılabilir.

[IBM XT]: https://en.wikipedia.org/wiki/IBM_Personal_Computer_XT
[IBM 3270 PC]: https://en.wikipedia.org/wiki/IBM_3270_PC
[IBM AT]: https://en.wikipedia.org/wiki/IBM_Personal_Computer/AT

Varsayılan olarak, PS/2 klavyeler scancode seti 1'i ("XT") öykünür. Bu sette, bir scancode baytının alt 7 biti tuşu tanımlar ve en anlamlı bit, bunun bir basış ("0") mı yoksa bir bırakış ("1") mı olduğunu tanımlar. Orijinal [IBM XT] klavyesinde bulunmayan tuşlar, örneğin tuş takımındaki enter tuşu, art arda iki scancode üretir: bir `0xe0` kaçış (escape) baytı ve ardından tuşu temsil eden bir bayt. Tüm set 1 scancode'larının ve karşılık gelen tuşlarının listesi için [OSDev Wiki'sine][scancode set 1] göz atın.

[scancode set 1]: https://wiki.osdev.org/Keyboard#Scan_Code_Set_1

Scancode'ları tuşlara çevirmek için bir `match` ifadesi kullanabiliriz:

```rust
// src/interrupts.rs içinde

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    // yeni
    let key = match scancode {
        0x02 => Some('1'),
        0x03 => Some('2'),
        0x04 => Some('3'),
        0x05 => Some('4'),
        0x06 => Some('5'),
        0x07 => Some('6'),
        0x08 => Some('7'),
        0x09 => Some('8'),
        0x0a => Some('9'),
        0x0b => Some('0'),
        _ => None,
    };
    if let Some(key) = key {
        print!("{}", key);
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

Yukarıdaki kod, 0-9 sayı tuşlarının basışlarını çevirir ve diğer tüm tuşları yok sayar. Her scancode'a bir karakter veya `None` atamak için bir [match] ifadesi kullanır. Ardından, isteğe bağlı `key`'i yapısöküme (destructure) uğratmak için [`if let`] kullanır. Pattern'de aynı `key` değişken adını kullanarak, önceki bildirimi [gölgeleriz (shadow)][shadow]; bu, Rust'ta `Option` tiplerini yapısöküme uğratmak için yaygın bir örüntüdür.

[match]: https://doc.rust-lang.org/book/ch06-02-match.html
[`if let`]: https://doc.rust-lang.org/book/ch19-01-all-the-places-for-patterns.html#conditional-if-let-expressions
[shadow]: https://doc.rust-lang.org/book/ch03-01-variables-and-mutability.html#shadowing

Artık sayılar yazabiliriz:

![Ekrana sayılar yazdıran QEMU](qemu-printing-numbers.gif)

Diğer tuşları çevirmek de aynı şekilde çalışır. Neyse ki, scancode seti 1 ve 2'nin scancode'larını çevirmek için [`pc-keyboard`] adında bir crate var, bu yüzden bunu kendimiz uygulamamıza gerek yok. Crate'i kullanmak için, onu `Cargo.toml`'umuza ekliyor ve `lib.rs`'imizde içe aktarıyoruz:

[`pc-keyboard`]: https://docs.rs/pc-keyboard/0.7.0/pc_keyboard/

```toml
# Cargo.toml içinde

[dependencies]
pc-keyboard = "0.7.0"
```

Artık bu crate'i `keyboard_interrupt_handler`'ımızı yeniden yazmak için kullanabiliriz:

```rust
// src/interrupts.rs içinde

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    static KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
        Mutex::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        ));

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

Bir Mutex tarafından korunan statik bir [`Keyboard`] nesnesi oluşturuyoruz. `Keyboard::new` ve `ScancodeSet1::new` yapıcıları `const fn` olduğundan, `KEYBOARD` statik'i derleme zamanında başlatılabilir; bu yüzden burada `lazy_static` makrosuna ihtiyacımız yok. `Keyboard`'u bir ABD klavye düzeni ve scancode seti 1 ile başlatıyoruz. [`HandleControl`] parametresi, `ctrl+[a-z]`'yi `U+0001`'den `U+001A`'ya kadar Unicode karakterlerine eşlemeye olanak tanır. Bunu yapmak istemiyoruz, bu yüzden `ctrl`'yi normal tuşlar gibi ele almak için `Ignore` seçeneğini kullanıyoruz.

[`HandleControl`]: https://docs.rs/pc-keyboard/0.7.0/pc_keyboard/enum.HandleControl.html

Her interrupt'ta Mutex'i kilitliyor, scancode'u klavye controller'ından okuyor ve onu, scancode'u bir `Option<KeyEvent>`'e çeviren [`add_byte`] metoduna geçiriyoruz. [`KeyEvent`], olaya neden olan tuşu ve bunun bir basış mı yoksa bırakış olayı mı olduğunu içerir.

[`Keyboard`]: https://docs.rs/pc-keyboard/0.7.0/pc_keyboard/struct.Keyboard.html
[`add_byte`]: https://docs.rs/pc-keyboard/0.7.0/pc_keyboard/struct.Keyboard.html#method.add_byte
[`KeyEvent`]: https://docs.rs/pc-keyboard/0.7.0/pc_keyboard/struct.KeyEvent.html

Bu tuş olayını yorumlamak için, onu mümkünse tuş olayını bir karaktere çeviren [`process_keyevent`] metoduna geçiriyoruz. Örneğin, `A` tuşunun bir basış olayını, shift tuşuna basılıp basılmadığına bağlı olarak küçük harf `a` veya büyük harf `A` karakterine çevirir.

[`process_keyevent`]: https://docs.rs/pc-keyboard/0.7.0/pc_keyboard/struct.Keyboard.html#method.process_keyevent

Bu değiştirilmiş interrupt handler'ıyla, artık metin yazabiliriz:

![QEMU'da "Hello World" yazmak](qemu-typing.gif)

### Klavyeyi Yapılandırmak

Bir PS/2 klavyenin bazı yönlerini yapılandırmak mümkündür; örneğin hangi scancode setini kullanması gerektiğini. Bu yazı zaten yeterince uzun olduğu için bunu burada ele almayacağız, ancak OSDev Wiki'sinde olası [yapılandırma komutlarına][configuration commands] genel bir bakış var.

[configuration commands]: https://wiki.osdev.org/PS/2_Keyboard#Commands

## Özet

Bu yazı, harici interrupt'ların nasıl etkinleştirileceğini ve işleneceğini açıkladı. 8259 PIC'i ve onun birincil/ikincil düzenini, interrupt numaralarının yeniden eşlenmesini ve "interrupt sonu" sinyalini öğrendik. Donanım timer'ı ve klavye için handler'lar uyguladık ve CPU'yu bir sonraki interrupt'a kadar durduran `hlt` komutunu öğrendik.

Artık kernel'imizle etkileşim kurabiliyoruz ve küçük bir kabuk (shell) veya basit oyunlar oluşturmak için bazı temel yapı taşlarına sahibiz.

## Sırada ne var?

Timer interrupt'ları bir işletim sistemi için olmazsa olmazdır, çünkü çalışan süreci periyodik olarak kesip kernel'in kontrolü yeniden ele geçirmesine olanak tanırlar. Kernel daha sonra farklı bir sürece geçebilir ve paralel olarak çalışan birden çok süreç yanılsaması oluşturabilir.

Ancak süreçler veya thread'ler oluşturmadan önce, onlar için bellek ayırmanın bir yoluna ihtiyacımız var. Sonraki yazılar, bu temel yapı taşını sağlamak için bellek yönetimini inceleyecek.
