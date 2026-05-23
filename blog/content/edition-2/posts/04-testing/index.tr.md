+++
title = "Test Etme"
weight = 4
path = "tr/testing"
date = 2019-04-27

[extra]
chapter = "Bare Bones"
comments_search_term = 1009

# Please update this when updating the translation
translation_based_on_commit = "f4ae48bc95d9658bd32de85a57a9f82871ceeaa5"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

Bu yazı, `no_std` çalıştırılabilir dosyalarında birim (unit) ve entegrasyon (integration) testlerini inceliyor. Test fonksiyonlarını kernel'imizin içinde çalıştırmak için Rust'ın özel test çerçevelerine (custom test frameworks) yönelik desteğini kullanacağız. Sonuçları QEMU'dan dışarı raporlamak için QEMU'nun ve `bootimage` aracının farklı özelliklerini kullanacağız.

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-04`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-04

<!-- toc -->

## Gereksinimler

Bu yazı, (artık kullanımdan kaldırılmış olan) [_Birim Testi_][_Unit Testing_] ve [_Entegrasyon Testleri_][_Integration Tests_] yazılarının yerini alır. 2019-04-27 tarihinden sonra [_Minimal Bir Rust Kernel'i_][_A Minimal Rust Kernel_] yazısını takip etmiş olduğunuzu varsayar. Esas olarak, [varsayılan bir hedef belirleyen][sets a default target] ve [bir runner çalıştırılabilir dosyası tanımlayan][defines a runner executable] bir `.cargo/config.toml` dosyanızın olmasını gerektirir.

[_Unit Testing_]: @/edition-2/posts/deprecated/04-unit-testing/index.md
[_Integration Tests_]: @/edition-2/posts/deprecated/05-integration-tests/index.md
[_A Minimal Rust Kernel_]: @/edition-2/posts/02-minimal-rust-kernel/index.tr.md
[sets a default target]: @/edition-2/posts/02-minimal-rust-kernel/index.tr.md#set-a-default-target
[defines a runner executable]: @/edition-2/posts/02-minimal-rust-kernel/index.tr.md#using-cargo-run

## Rust'ta Test Etme

Rust'ın, herhangi bir kurulum yapmaya gerek kalmadan birim testleri çalıştırabilen [yerleşik bir test çerçevesi][built-in test framework] vardır. Yalnızca, bazı sonuçları assertion'lar (doğrulamalar) aracılığıyla kontrol eden bir fonksiyon oluşturmanız ve fonksiyon başlığına `#[test]` özniteliğini eklemeniz yeterlidir. Ardından `cargo test`, crate'inizdeki tüm test fonksiyonlarını otomatik olarak bulup çalıştırır.

[built-in test framework]: https://doc.rust-lang.org/book/ch11-00-testing.html

Kernel ikili dosyamız için testi etkinleştirmek üzere, Cargo.toml dosyasındaki `test` bayrağını `true` olarak ayarlayabiliriz:

```toml
# Cargo.toml içinde

[[bin]]
name = "blog_os"
test = true
bench = false
```

Bu [`[[bin]]` bölümü](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#configuring-a-target), `cargo`'nun `blog_os` çalıştırılabilir dosyamızı nasıl derlemesi gerektiğini belirtir.
`test` alanı, bu çalıştırılabilir dosya için testin desteklenip desteklenmediğini belirtir.
İlk yazıda [`rust-analyzer`'ı mutlu etmek için](@/edition-2/posts/01-freestanding-rust-binary/index.tr.md#making-rust-analyzer-happy) `test = false` ayarlamıştık, ancak şimdi testi etkinleştirmek istiyoruz, bu yüzden onu tekrar `true` olarak ayarlıyoruz.

Ne yazık ki, kernel'imiz gibi `no_std` uygulamaları için test biraz daha karmaşıktır. Sorun, Rust'ın test çerçevesinin örtük olarak, standart kütüphaneye bağımlı olan yerleşik [`test`] kütüphanesini kullanmasıdır. Bu, `#[no_std]` kernel'imiz için varsayılan test çerçevesini kullanamayacağımız anlamına gelir.

[`test`]: https://doc.rust-lang.org/test/index.html

Bunu, projemizde `cargo test` çalıştırmayı denediğimizde görebiliriz:

```
> cargo test
   Compiling blog_os v0.1.0 (/…/blog_os)
error[E0463]: can't find crate for `test`
```

`test` crate'i standart kütüphaneye bağımlı olduğundan, bare metal hedefimiz için kullanılamaz. `test` crate'ini bir `#[no_std]` bağlamına taşımak [mümkün olsa da][utest], bu son derece kararsızdır ve `panic` makrosunu yeniden tanımlamak gibi bazı hile'ler (hack) gerektirir.

[utest]: https://github.com/japaric/utest

### Özel Test Çerçeveleri

Neyse ki Rust, varsayılan test çerçevesinin kararsız [`custom_test_frameworks`] özelliği aracılığıyla değiştirilmesini destekler. Bu özellik herhangi bir dış kütüphane gerektirmez ve bu yüzden `#[no_std]` ortamlarında da çalışır. `#[test_case]` özniteliğiyle işaretlenmiş tüm fonksiyonları toplayarak ve ardından kullanıcı tarafından belirlenmiş bir runner fonksiyonunu, argüman olarak test listesiyle çağırarak çalışır. Böylece, uygulamaya test süreci üzerinde maksimum kontrol verir.

[`custom_test_frameworks`]: https://doc.rust-lang.org/unstable-book/language-features/custom-test-frameworks.html

Varsayılan test çerçevesine kıyasla dezavantajı, [`should_panic` testleri][`should_panic` tests] gibi pek çok gelişmiş özelliğin kullanılamamasıdır. Bunun yerine, gerekirse bu tür özellikleri sağlamak uygulamanın kendisine kalmıştır. Bu bizim için idealdir, çünkü çok özel bir çalıştırma ortamımız var ve bu tür gelişmiş özelliklerin varsayılan uygulamaları muhtemelen zaten çalışmazdı. Örneğin, `#[should_panic]` özniteliği panic'leri yakalamak için stack unwinding'e dayanır; biz ise bunu kernel'imiz için devre dışı bıraktık.

[`should_panic` tests]: https://doc.rust-lang.org/book/ch11-01-writing-tests.html#checking-for-panics-with-should_panic

Kernel'imiz için özel bir test çerçevesi uygulamak amacıyla, `main.rs` dosyamıza aşağıdakileri ekliyoruz:

```rust
// src/main.rs içinde

#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}
```

Runner'ımız yalnızca kısa bir hata ayıklama mesajı yazdırır ve ardından listedeki her test fonksiyonunu çağırır. `&[&dyn Fn()]` argüman tipi, [_Fn()_] trait'inin [_trait nesnesi (trait object)_][_trait object_] referanslarından oluşan bir [_dilim (slice)_][_slice_]'dir. Temel olarak, bir fonksiyon gibi çağrılabilen tiplere yapılan referansların bir listesidir. Bu fonksiyon test dışı çalıştırmalar için işe yaramaz olduğundan, onu yalnızca testler için dahil etmek üzere `#[cfg(test)]` özniteliğini kullanıyoruz.

[_slice_]: https://doc.rust-lang.org/std/primitive.slice.html
[_trait object_]: https://doc.rust-lang.org/1.30.0/book/first-edition/trait-objects.html
[_Fn()_]: https://doc.rust-lang.org/std/ops/trait.Fn.html

Şimdi `cargo test` çalıştırdığımızda, artık başarılı olduğunu görüyoruz (olmuyorsa, aşağıdaki nota bakın). Ancak hâlâ `test_runner`'ımızdan gelen mesaj yerine "Hello World"'ümüzü görüyoruz. Bunun nedeni, `_start` fonksiyonumuzun hâlâ giriş noktası olarak kullanılmasıdır. Özel test çerçeveleri özelliği, `test_runner`'ı çağıran bir `main` fonksiyonu üretir; ancak biz `#[no_main]` özniteliğini kullanıp kendi giriş noktamızı sağladığımız için bu fonksiyon yok sayılır.

<div class = "warning">

**Not:** Cargo'da şu anda, bazı durumlarda `cargo test`'te "duplicate lang item" hatalarına yol açan bir hata (bug) var. Bu hata, `Cargo.toml` dosyanızda bir profil için `panic = "abort"` ayarladığınızda oluşur. Onu kaldırmayı deneyin, sonra `cargo test` çalışmalı. Alternatif olarak, eğer bu işe yaramazsa, `.cargo/config.toml` dosyanızın `[unstable]` bölümüne `panic-abort-tests = true` ekleyin. Bu konuda daha fazla bilgi için [cargo issue](https://github.com/rust-lang/cargo/issues/7359)'ya bakın.

</div>

Bunu düzeltmek için, önce üretilen fonksiyonun adını `reexport_test_harness_main` özniteliği aracılığıyla `main`'den farklı bir şeye değiştirmemiz gerekir. Ardından, yeniden adlandırılan fonksiyonu `_start` fonksiyonumuzdan çağırabiliriz:

```rust
// src/main.rs içinde

#![reexport_test_harness_main = "test_main"]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}
```

Test çerçevesi giriş fonksiyonunun adını `test_main` olarak belirliyor ve onu `_start` giriş noktamızdan çağırıyoruz. `test_main` çağrısını yalnızca test bağlamlarına eklemek için [koşullu derleme (conditional compilation)][conditional compilation] kullanıyoruz, çünkü bu fonksiyon normal bir çalıştırmada üretilmez.

Şimdi `cargo test`'i çalıştırdığımızda, ekranda `test_runner`'ımızdan gelen "Running 0 tests" mesajını görüyoruz. Artık ilk test fonksiyonumuzu oluşturmaya hazırız:

```rust
// src/main.rs içinde

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}
```

Şimdi `cargo test` çalıştırdığımızda, aşağıdaki çıktıyı görüyoruz:

![QEMU'da "Hello World!", "Running 1 tests" ve "trivial assertion... [ok]" yazısı](qemu-test-runner-output.png)

`test_runner` fonksiyonumuza geçirilen `tests` dilimi artık `trivial_assertion` fonksiyonuna bir referans içeriyor. Ekrandaki `trivial assertion... [ok]` çıktısından, testin çağrıldığını ve başarılı olduğunu görüyoruz.

Testleri çalıştırdıktan sonra, `test_runner`'ımız `test_main` fonksiyonuna geri döner; o da sırayla `_start` giriş noktası fonksiyonumuza geri döner. `_start`'ın sonunda, giriş noktası fonksiyonunun geri dönmesine izin verilmediği için sonsuz bir döngüye giriyoruz. Bu bir sorundur, çünkü `cargo test`'in tüm testleri çalıştırdıktan sonra çıkmasını istiyoruz.

## QEMU'dan Çıkış

Şu anda `_start` fonksiyonumuzun sonunda sonsuz bir döngü var ve her `cargo test` çalıştırmasında QEMU'yu elle kapatmamız gerekiyor. Bu talihsiz bir durumdur, çünkü `cargo test`'i kullanıcı etkileşimi olmadan betiklerde de çalıştırmak istiyoruz. Bunun temiz çözümü, OS'umuzu kapatmanın düzgün bir yolunu uygulamak olurdu. Ne yazık ki bu nispeten karmaşıktır, çünkü [APM] veya [ACPI] güç yönetimi standardından birine yönelik destek uygulamayı gerektirir.

[APM]: https://wiki.osdev.org/APM
[ACPI]: https://wiki.osdev.org/ACPI

Neyse ki bir kaçış kapısı var: QEMU, misafir (guest) sistemden QEMU'dan çıkmanın kolay bir yolunu sağlayan özel bir `isa-debug-exit` cihazını destekler. Onu etkinleştirmek için QEMU'ya bir `-device` argümanı geçirmemiz gerekir. Bunu, `Cargo.toml` dosyamıza bir `package.metadata.bootimage.test-args` yapılandırma anahtarı ekleyerek yapabiliriz:

```toml
# Cargo.toml içinde

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
```

`bootimage runner`, tüm test çalıştırılabilir dosyaları için varsayılan QEMU komutuna `test-args`'ı ekler. Normal bir `cargo run` için argümanlar yok sayılır.

Cihaz adıyla (`isa-debug-exit`) birlikte, cihaza kernel'imizden ulaşılabilecek _G/Ç portunu (I/O port)_ belirten `iobase` ve `iosize` parametrelerini geçiriyoruz.

### G/Ç Portları {#i-o-ports}

x86'da CPU ile çevresel donanım arasında iletişim kurmak için iki farklı yaklaşım vardır: **belleğe eşlenmiş G/Ç (memory-mapped I/O)** ve **porta eşlenmiş G/Ç (port-mapped I/O)**. [VGA metin arabelleğine][VGA text buffer] `0xb8000` bellek adresi aracılığıyla erişmek için belleğe eşlenmiş G/Ç'yi zaten kullandık. Bu adres RAM'e değil, VGA cihazındaki bir belleğe eşlenmiştir.

[VGA text buffer]: @/edition-2/posts/03-vga-text-buffer/index.tr.md

Buna karşılık, porta eşlenmiş G/Ç iletişim için ayrı bir G/Ç veri yolu (bus) kullanır. Bağlı her çevresel cihazın bir veya daha fazla port numarası vardır. Böyle bir G/Ç portuyla iletişim kurmak için, bir port numarası ve bir veri baytı alan `in` ve `out` adlı özel CPU komutları vardır (bu komutların bir `u16` veya `u32` göndermeye olanak tanıyan varyasyonları da vardır).

`isa-debug-exit` cihazı porta eşlenmiş G/Ç kullanır. `iobase` parametresi cihazın hangi port adresinde bulunması gerektiğini belirtir (`0xf4`, x86'nın G/Ç veri yolunda [genellikle kullanılmayan][list of x86 I/O ports] bir porttur) ve `iosize` port boyutunu belirtir (`0x04`, dört bayt anlamına gelir).

[list of x86 I/O ports]: https://wiki.osdev.org/I/O_Ports#The_list

### Çıkış Cihazını Kullanmak

`isa-debug-exit` cihazının işlevselliği çok basittir. `iobase` tarafından belirtilen G/Ç portuna bir `value` (değer) yazıldığında, QEMU'nun `(value << 1) | 1` [çıkış durumuyla (exit status)][exit status] çıkmasına neden olur. Yani porta `0` yazdığımızda QEMU `(0 << 1) | 1 = 1` çıkış durumuyla çıkar; porta `1` yazdığımızda ise `(1 << 1) | 1 = 3` çıkış durumuyla çıkar.

[exit status]: https://en.wikipedia.org/wiki/Exit_status

`in` ve `out` assembly komutlarını elle çağırmak yerine, [`x86_64`] crate'inin sağladığı soyutlamaları kullanıyoruz. Bu crate'e bir bağımlılık eklemek için, onu `Cargo.toml` dosyamızdaki `dependencies` bölümüne ekliyoruz:

[`x86_64`]: https://docs.rs/x86_64/0.14.2/x86_64/

```toml
# Cargo.toml içinde

[dependencies]
x86_64 = "0.14.2"
```

Artık bir `exit_qemu` fonksiyonu oluşturmak için crate'in sağladığı [`Port`] tipini kullanabiliriz:

[`Port`]: https://docs.rs/x86_64/0.14.2/x86_64/instructions/port/struct.Port.html

```rust
// src/main.rs içinde

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

Fonksiyon, `isa-debug-exit` cihazının `iobase`'i olan `0xf4`'te yeni bir [`Port`] oluşturur. Ardından, geçirilen çıkış kodunu porta yazar. `isa-debug-exit` cihazının `iosize`'ını 4 bayt olarak belirttiğimiz için `u32` kullanıyoruz. Bir G/Ç portuna yazmak genellikle keyfi davranışla sonuçlanabileceğinden, her iki işlem de unsafe'tir.

Çıkış durumunu belirtmek için bir `QemuExitCode` enum'ı oluşturuyoruz. Fikir, tüm testler başarılı olursa başarı çıkış koduyla, aksi takdirde başarısızlık çıkış koduyla çıkmaktır. Enum, her varyantı bir `u32` tamsayıyla temsil etmek için `#[repr(u32)]` olarak işaretlenmiştir. Başarı için `0x10`, başarısızlık için `0x11` çıkış kodunu kullanıyoruz. QEMU'nun varsayılan çıkış kodlarıyla çakışmadıkları sürece, gerçek çıkış kodları pek önemli değildir. Örneğin, başarı için `0` çıkış kodunu kullanmak iyi bir fikir değildir, çünkü dönüşümden sonra `(0 << 1) | 1 = 1` olur ve bu, QEMU çalışmayı başaramadığında verdiği varsayılan çıkış kodudur. Yani bir QEMU hatasını başarılı bir test çalıştırmasından ayırt edemezdik.

Artık `test_runner`'ımızı, tüm testler çalıştıktan sonra QEMU'dan çıkacak şekilde güncelleyebiliriz:

```rust
// src/main.rs içinde

fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    /// yeni
    exit_qemu(QemuExitCode::Success);
}
```

Şimdi `cargo test` çalıştırdığımızda, QEMU'nun testleri çalıştırdıktan hemen sonra kapandığını görüyoruz. Sorun şu ki, `Success` çıkış kodumuzu geçirmemize rağmen `cargo test`, testi başarısız olmuş gibi yorumluyor:

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.03s
     Running target/x86_64-blog_os/debug/deps/blog_os-5804fc7d2dd4c9be
Building bootloader
   Compiling bootloader v0.5.3 (/home/philipp/Documents/bootloader)
    Finished release [optimized + debuginfo] target(s) in 1.07s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-5804fc7d2dd4c9be.bin -device isa-debug-exit,iobase=0xf4,
    iosize=0x04`
error: test failed, to rerun pass '--bin blog_os'
```

Sorun, `cargo test`'in `0` dışındaki tüm hata kodlarını başarısızlık olarak görmesidir.

### Başarı Çıkış Kodu

Bunu aşmak için `bootimage`, belirtilen bir çıkış kodunu `0` çıkış koduyla eşleyen bir `test-success-exit-code` yapılandırma anahtarı sağlar:

```toml
# Cargo.toml içinde

[package.metadata.bootimage]
test-args = […]
test-success-exit-code = 33         # (0x10 << 1) | 1
```

Bu yapılandırmayla `bootimage`, başarı çıkış kodumuzu çıkış kodu 0'a eşler; böylece `cargo test` başarı durumunu doğru bir şekilde tanır ve testi başarısız olarak saymaz.

Test runner'ımız artık QEMU'yu otomatik olarak kapatıyor ve test sonuçlarını doğru bir şekilde raporluyor. QEMU penceresinin hâlâ çok kısa bir süre açıldığını görüyoruz, ancak bu süre sonuçları okumaya yetmiyor. Bunun yerine test sonuçlarını konsola yazdırabilseydik güzel olurdu; böylece QEMU çıktıktan sonra da onları görebilirdik.

## Konsola Yazdırmak

Test çıktısını konsolda görmek için, verileri kernel'imizden host sistemine bir şekilde göndermemiz gerekiyor. Bunu başarmanın çeşitli yolları vardır; örneğin verileri bir TCP ağ arayüzü üzerinden göndermek gibi. Ancak bir ağ yığını (networking stack) kurmak oldukça karmaşık bir iştir, bu yüzden bunun yerine daha basit bir çözüm seçeceğiz.

### Seri Port

Verileri göndermenin basit bir yolu, modern bilgisayarlarda artık bulunmayan eski bir arayüz standardı olan [seri portu (serial port)][serial port] kullanmaktır. Programlanması kolaydır ve QEMU, seri port üzerinden gönderilen baytları host'un standart çıktısına veya bir dosyaya yönlendirebilir.

[serial port]: https://en.wikipedia.org/wiki/Serial_port

Bir seri arayüzü uygulayan çiplere [UART] denir. x86'da [pek çok UART modeli][lots of UART models] vardır, ancak neyse ki aralarındaki tek fark ihtiyaç duymadığımız bazı gelişmiş özelliklerdir. Günümüzdeki yaygın UART'ların hepsi [16550 UART] ile uyumludur, bu yüzden test çerçevemiz için o modeli kullanacağız.

[UART]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter
[lots of UART models]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter#Models
[16550 UART]: https://en.wikipedia.org/wiki/16550_UART

UART'ı başlatmak ve verileri seri port üzerinden göndermek için [`uart_16550`] crate'ini kullanacağız. Onu bir bağımlılık olarak eklemek için, `Cargo.toml` ve `main.rs` dosyalarımızı güncelliyoruz:

[`uart_16550`]: https://docs.rs/uart_16550

```toml
# Cargo.toml içinde

[dependencies]
uart_16550 = "0.6.0"
```

`uart_16550` crate'i, UART'ı [TTY](https://en.wikipedia.org/wiki/Teleprinter) modunda başlatan ve metni kolayca göndermemize olanak tanıyan bir [`Uart16550Tty`](https://docs.rs/uart_16550/latest/uart_16550/struct.Uart16550Tty.html) tipi içerir.

Bu tipi yeni bir `serial` modülünde kullanalım:

```rust
// src/main.rs içinde

mod serial;
```

```rust
// src/serial.rs içinde

use uart_16550::{Config, Uart16550Tty, backend::PioBackend};
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL1: Mutex<Uart16550Tty<PioBackend>> = Mutex::new(unsafe {
        Uart16550Tty::new_port(0x3F8, Config::default())
            .expect("failed to initialize UART")
    });
}
```

[VGA metin arabelleğinde][vga lazy-static] olduğu gibi, bir `static` writer örneği oluşturmak için `lazy_static` ve bir spinlock kullanıyoruz. `lazy_static` kullanarak, UART'ın ilk kullanımında tam olarak bir kez başlatılmasını sağlayabiliriz.

`isa-debug-exit` cihazı gibi, UART de port G/Ç'si kullanılarak programlanır; bu da [`PioBackend`](https://docs.rs/uart_16550/latest/uart_16550/backend/struct.PioBackend.html) parametresiyle belirtilir. UART daha karmaşık olduğu için, farklı cihaz register'larını programlamak amacıyla birden çok G/Ç portu kullanır. Unsafe `Uart16550Tty::new_port` fonksiyonu, argüman olarak UART'ın ilk G/Ç portunun adresini bekler; bu adresten gereken tüm portların adreslerini hesaplayabilir. Biz, ilk seri arayüz için standart port numarası olan `0x3F8` port adresini geçiriyoruz.

[vga lazy-static]: @/edition-2/posts/03-vga-text-buffer/index.tr.md#lazy-statics

Seri portu kolayca kullanılabilir hale getirmek için, `serial_print!` ve `serial_println!` makrolarını ekliyoruz:

```rust
// src/serial.rs içinde

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
}

/// Seri arayüz aracılığıyla host'a yazdırır.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Seri arayüz aracılığıyla host'a, sona bir yeni satır ekleyerek yazdırır.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
```

Uygulama, `print` ve `println` makrolarımızın uygulamasına çok benzer. `Uart16550Tty` tipi [`fmt::Write`] trait'ini zaten uyguladığı için, kendi uygulamamızı sağlamamıza gerek yok.

[`fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

Artık test kodumuzda VGA metin arabelleği yerine seri arayüze yazdırabiliriz:

```rust
// src/main.rs içinde

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    […]
}

#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

`#[macro_export]` özniteliğini kullandığımız için `serial_println` makrosunun doğrudan kök ad alanının altında bulunduğunu, bu yüzden onu `use crate::serial::serial_println` aracılığıyla içe aktarmanın çalışmayacağını unutmayın.

### QEMU Argümanları

QEMU'dan gelen seri çıktıyı görmek için, çıktıyı stdout'a yönlendirmek üzere `-serial` argümanını kullanmamız gerekir:

```toml
# Cargo.toml içinde

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio"
]
```

Şimdi `cargo test` çalıştırdığımızda, test çıktısını doğrudan konsolda görüyoruz:

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [ok]
```

Ancak bir test başarısız olduğunda, panic handler'ımız hâlâ `println` kullandığı için çıktıyı yine QEMU'nun içinde görüyoruz. Bunu canlandırmak için, `trivial_assertion` testimizdeki assertion'ı `assert_eq!(0, 1)` olarak değiştirebiliriz:

![QEMU'da "Hello World!" ve "panicked at 'assertion failed: `(left == right)`
    left: `0`, right: `1`', src/main.rs:55:5" yazısı](qemu-failed-test.png)

Panic mesajının hâlâ VGA arabelleğine, diğer test çıktısının ise seri porta yazdırıldığını görüyoruz. Panic mesajı oldukça yararlıdır, bu yüzden onu da konsolda görmek faydalı olurdu.

### Panic Anında Hata Mesajı Yazdırmak

Bir panic durumunda QEMU'dan bir hata mesajıyla çıkmak için, test modunda farklı bir panic handler kullanmak üzere [koşullu derleme][conditional compilation] kullanabiliriz:

[conditional compilation]: https://doc.rust-lang.org/1.30.0/book/first-edition/conditional-compilation.html

```rust
// src/main.rs içinde

// mevcut panic handler'ımız
#[cfg(not(test))] // yeni öznitelik
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

// test modundaki panic handler'ımız
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}
```

Test panic handler'ımız için `println` yerine `serial_println` kullanıyor ve ardından QEMU'dan bir başarısızlık çıkış koduyla çıkıyoruz. `exit_qemu` çağrısından sonra hâlâ sonsuz bir `loop`'a ihtiyacımız olduğunu unutmayın, çünkü derleyici `isa-debug-exit` cihazının bir program çıkışına neden olduğunu bilmez.

Artık QEMU başarısız testler için de çıkıyor ve konsola yararlı bir hata mesajı yazdırıyor:

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [failed]

Error: panicked at 'assertion failed: `(left == right)`
  left: `0`,
 right: `1`', src/main.rs:65:5
```

Artık tüm test çıktısını konsolda gördüğümüz için, kısa bir süre açılan QEMU penceresine artık ihtiyacımız yok. Bu yüzden onu tamamen gizleyebiliriz.

### QEMU'yu Gizlemek

Eksiksiz test sonuçlarını `isa-debug-exit` cihazını ve seri portu kullanarak dışarı raporladığımız için, artık QEMU penceresine ihtiyacımız yok. QEMU'ya `-display none` argümanını geçirerek onu gizleyebiliriz:

```toml
# Cargo.toml içinde

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio",
    "-display", "none"
]
```

Artık QEMU tamamen arka planda çalışıyor ve hiçbir pencere açılmıyor. Bu yalnızca daha az can sıkıcı olmakla kalmıyor, aynı zamanda test çerçevemizin CI hizmetleri veya [SSH] bağlantıları gibi grafiksel bir kullanıcı arayüzü olmayan ortamlarda çalışmasına da olanak tanıyor.

[SSH]: https://en.wikipedia.org/wiki/Secure_Shell

### Zaman Aşımları

`cargo test`, test runner çıkana kadar beklediği için, asla geri dönmeyen bir test, test runner'ı sonsuza dek bloklayabilir. Bu talihsiz bir durumdur, ancak sonsuz döngülerden kaçınmak genellikle kolay olduğu için pratikte büyük bir sorun değildir. Ancak bizim durumumuzda, sonsuz döngüler çeşitli durumlarda meydana gelebilir:

- Bootloader kernel'imizi yüklemeyi başaramaz, bu da sistemin sonsuza dek yeniden başlamasına neden olur.
- BIOS/UEFI firmware'i bootloader'ı yüklemeyi başaramaz, bu da aynı sonsuz yeniden başlatmaya neden olur.
- CPU, örneğin QEMU çıkış cihazı düzgün çalışmadığı için, bazı fonksiyonlarımızın sonunda bir `loop {}` ifadesine girer.
- Donanım bir sistem sıfırlamasına (reset) neden olur; örneğin bir CPU exception'ı yakalanmadığında (gelecekteki bir yazıda açıklanacak).

Sonsuz döngüler çok fazla durumda meydana gelebileceği için, `bootimage` aracı her test çalıştırılabilir dosyası için varsayılan olarak 5 dakikalık bir zaman aşımı belirler. Test bu süre içinde bitmezse, başarısız olarak işaretlenir ve konsola bir "Timed Out" hatası yazdırılır. Bu özellik, sonsuz bir döngüde takılı kalan testlerin `cargo test`'i sonsuza dek bloklamamasını sağlar.

Bunu `trivial_assertion` testine bir `loop {}` ifadesi ekleyerek kendiniz deneyebilirsiniz. `cargo test` çalıştırdığınızda, testin 5 dakika sonra zaman aşımına uğramış olarak işaretlendiğini görürsünüz. Zaman aşımı süresi, Cargo.toml'daki bir `test-timeout` anahtarı aracılığıyla [yapılandırılabilir][bootimage config]:

[bootimage config]: https://github.com/rust-osdev/bootimage#configuration

```toml
# Cargo.toml içinde

[package.metadata.bootimage]
test-timeout = 300          # (saniye cinsinden)
```

`trivial_assertion` testinin zaman aşımına uğraması için 5 dakika beklemek istemiyorsanız, yukarıdaki değeri geçici olarak azaltabilirsiniz.

### Yazdırmayı Otomatik Eklemek

`trivial_assertion` testimizin şu anda kendi durum bilgisini `serial_print!`/`serial_println!` kullanarak yazdırması gerekiyor:

```rust
#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

Yazdığımız her test için bu yazdırma ifadelerini elle eklemek zahmetlidir, bu yüzden `test_runner`'ımızı bu mesajları otomatik olarak yazdıracak şekilde güncelleyelim. Bunu yapmak için, yeni bir `Testable` trait'i oluşturmamız gerekir:

```rust
// src/main.rs içinde

pub trait Testable {
    fn run(&self) -> ();
}
```

Şimdi işin püf noktası, bu trait'i [`Fn()` trait'ini][`Fn()` trait] uygulayan tüm `T` tipleri için uygulamaktır:

[`Fn()` trait]: https://doc.rust-lang.org/stable/core/ops/trait.Fn.html

```rust
// src/main.rs içinde

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}
```

`run` fonksiyonunu, önce [`any::type_name`] fonksiyonunu kullanarak fonksiyon adını yazdırarak uyguluyoruz. Bu fonksiyon doğrudan derleyicide uygulanmıştır ve her tipin bir dize açıklamasını döndürür. Fonksiyonlar için tip, onların adıdır; yani bu durumda tam olarak istediğimiz şey budur. `\t` karakteri, `[ok]` mesajlarına biraz hizalama ekleyen [sekme karakteridir (tab character)][tab character].

[`any::type_name`]: https://doc.rust-lang.org/stable/core/any/fn.type_name.html
[tab character]: https://en.wikipedia.org/wiki/Tab_character

Fonksiyon adını yazdırdıktan sonra, test fonksiyonunu `self()` aracılığıyla çağırıyoruz. Bu yalnızca, `self`'in `Fn()` trait'ini uygulamasını gerektirdiğimiz için çalışır. Test fonksiyonu geri döndükten sonra, fonksiyonun panic yapmadığını belirtmek için `[ok]` yazdırıyoruz.

Son adım, `test_runner`'ımızı yeni `Testable` trait'ini kullanacak şekilde güncellemektir:

```rust
// src/main.rs içinde

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) { // yeni
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run(); // yeni
    }
    exit_qemu(QemuExitCode::Success);
}
```

Tek iki değişiklik, `tests` argümanının tipinin `&[&dyn Fn()]`'ten `&[&dyn Testable]`'a değişmesi ve artık `test()` yerine `test.run()` çağırmamızdır.

Artık `trivial_assertion` testimizden yazdırma ifadelerini kaldırabiliriz, çünkü artık otomatik olarak yazdırılıyorlar:

```rust
// src/main.rs içinde

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
```

`cargo test` çıktısı artık şöyle görünüyor:

```
Running 1 tests
blog_os::trivial_assertion...	[ok]
```

Fonksiyon adı artık fonksiyona giden tam yolu içeriyor; bu, farklı modüllerdeki test fonksiyonları aynı ada sahip olduğunda yararlıdır. Bunun dışında çıktı eskisiyle aynı görünüyor, ancak artık testlerimize yazdırma ifadelerini elle eklememize gerek yok.

## VGA Arabelleğini Test Etmek

Artık çalışan bir test çerçevemiz olduğuna göre, VGA arabellek uygulamamız için birkaç test oluşturabiliriz. İlk olarak, `println`'in panic yapmadan çalıştığını doğrulamak için çok basit bir test oluşturuyoruz:

```rust
// src/vga_buffer.rs içinde

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}
```

Test yalnızca VGA arabelleğine bir şeyler yazdırır. Panic yapmadan biterse, `println` çağrısının da panic yapmadığı anlamına gelir.

Çok sayıda satır yazdırılsa ve satırlar ekrandan dışarı kaydırılsa bile bir panic oluşmadığından emin olmak için, başka bir test oluşturabiliriz:

```rust
// src/vga_buffer.rs içinde

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}
```

Yazdırılan satırların gerçekten ekranda göründüğünü doğrulamak için bir test fonksiyonu da oluşturabiliriz:

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

Fonksiyon bir test dizesi tanımlar, onu `println` kullanarak yazdırır ve ardından VGA metin arabelleğini temsil eden statik `WRITER`'ın ekran karakterleri üzerinde iterasyon yapar. `println` son ekran satırına yazdırıp hemen ardından bir yeni satır eklediğinden, dize `BUFFER_HEIGHT - 2` satırında görünmelidir.

[`enumerate`]'i kullanarak, iterasyon sayısını `i` değişkeninde sayarız; bunu daha sonra `c`'ye karşılık gelen ekran karakterini yüklemek için kullanırız. Ekran karakterinin `ascii_character`'ını `c` ile karşılaştırarak, dizenin her karakterinin gerçekten VGA metin arabelleğinde göründüğünden emin oluruz.

[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate

Tahmin edebileceğiniz gibi, çok daha fazla test fonksiyonu oluşturabilirdik. Örneğin, çok uzun satırlar yazdırılırken bir panic oluşmadığını ve bunların doğru şekilde kaydırıldığını test eden bir fonksiyon; ya da yeni satırların, yazdırılamaz karakterlerin ve unicode olmayan karakterlerin doğru işlendiğini test eden bir fonksiyon.

Ancak bu yazının geri kalanında, farklı bileşenlerin birbiriyle etkileşimini test etmek için _entegrasyon testlerinin_ nasıl oluşturulacağını açıklayacağız.

## Entegrasyon Testleri

Rust'ta [entegrasyon testleri][integration tests] için gelenek, onları proje kök dizinindeki bir `tests` dizinine (yani `src` dizininin yanına) koymaktır. Hem varsayılan test çerçevesi hem de özel test çerçeveleri, bu dizindeki tüm testleri otomatik olarak bulup çalıştırır.

[integration tests]: https://doc.rust-lang.org/book/ch11-03-test-organization.html#integration-tests

Tüm entegrasyon testleri kendi çalıştırılabilir dosyalarıdır ve `main.rs`'imizden tamamen ayrıdır. Bu, her testin kendi giriş noktası fonksiyonunu tanımlaması gerektiği anlamına gelir. Ayrıntılı olarak nasıl çalıştığını görmek için `basic_boot` adında örnek bir entegrasyon testi oluşturalım:

```rust
// tests/basic_boot.rs içinde

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

#[unsafe(no_mangle)] // bu fonksiyonun adını parçalama (mangle etme)
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

fn test_runner(tests: &[&dyn Fn()]) {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
```

Entegrasyon testleri ayrı çalıştırılabilir dosyalar olduğundan, tüm crate özniteliklerini (`no_std`, `no_main`, `test_runner` vb.) yeniden sağlamamız gerekir. Ayrıca, test giriş noktası fonksiyonu `test_main`'i çağıran yeni bir giriş noktası fonksiyonu `_start` oluşturmamız gerekir. Entegrasyon testi çalıştırılabilir dosyaları asla test dışı modda derlenmediği için herhangi bir `cfg(test)` özniteliğine ihtiyacımız yok.

`test_runner` fonksiyonu için bir yer tutucu olarak her zaman panic yapan [`unimplemented`] makrosunu kullanıyor ve şimdilik `panic` handler'ında yalnızca `loop` yapıyoruz. İdeal olarak, bu fonksiyonları tam olarak `main.rs`'imizde yaptığımız gibi `serial_println` makrosunu ve `exit_qemu` fonksiyonunu kullanarak uygulamak istiyoruz. Sorun, testler `main.rs` çalıştırılabilir dosyamızdan tamamen ayrı olarak derlendiği için bu fonksiyonlara erişimimizin olmamasıdır.

[`unimplemented`]: https://doc.rust-lang.org/core/macro.unimplemented.html

Bu aşamada `cargo test` çalıştırırsanız, panic handler'ı sonsuza dek döngüye girdiği için sonsuz bir döngüyle karşılaşırsınız. QEMU'dan çıkmak için `ctrl+c` klavye kısayolunu kullanmanız gerekir.

### Bir Kütüphane Oluşturmak

Gereken fonksiyonları entegrasyon testimize kullanılabilir kılmak için, `main.rs`'imizden, diğer crate'ler ve entegrasyon testi çalıştırılabilir dosyaları tarafından dahil edilebilecek bir kütüphaneyi ayırmamız gerekir. Bunu yapmak için yeni bir `src/lib.rs` dosyası oluşturuyoruz:

```rust
// src/lib.rs

#![no_std]

```

`main.rs` gibi, `lib.rs` de cargo tarafından otomatik olarak tanınan özel bir dosyadır. Kütüphane ayrı bir derleme birimidir, bu yüzden `#![no_std]` özniteliğini yeniden belirtmemiz gerekir.

Kütüphanemizi `cargo test` ile çalışır hale getirmek için, test fonksiyonlarını ve özniteliklerini de `main.rs`'ten `lib.rs`'e taşımamız gerekir:

```rust
// src/lib.rs içinde

#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

/// `cargo test` için giriş noktası
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    test_main();
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
```

`test_runner`'ımızı çalıştırılabilir dosyalara ve entegrasyon testlerine kullanılabilir kılmak için, onu public yapıyor ve ona `cfg(test)` özniteliğini uygulamıyoruz. Ayrıca panic handler'ımızın uygulamasını public bir `test_panic_handler` fonksiyonuna ayırıyoruz; böylece o da çalıştırılabilir dosyalar için kullanılabilir oluyor.

`lib.rs`'imiz `main.rs`'imizden bağımsız olarak test edildiğinden, kütüphane test modunda derlendiğinde bir `_start` giriş noktası ve bir panic handler eklememiz gerekir. [`cfg_attr`] crate özniteliğini kullanarak, bu durumda `no_main` özniteliğini koşullu olarak etkinleştiriyoruz.

[`cfg_attr`]: https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute

`QemuExitCode` enum'ını ve `exit_qemu` fonksiyonunu da taşıyıp public yapıyoruz:

```rust
// src/lib.rs içinde

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

Artık çalıştırılabilir dosyalar ve entegrasyon testleri bu fonksiyonları kütüphaneden içe aktarabilir ve kendi uygulamalarını tanımlamaları gerekmez. `println` ve `serial_println`'i de kullanılabilir kılmak için, modül bildirimlerini de taşıyoruz:

```rust
// src/lib.rs içinde

pub mod serial;
pub mod vga_buffer;
```

Modülleri, kütüphanemizin dışında kullanılabilir kılmak için public yapıyoruz. Bu, `println` ve `serial_println` makrolarımızı kullanılabilir kılmak için de gereklidir, çünkü onlar modüllerin `_print` fonksiyonlarını kullanır.

Artık `main.rs`'imizi kütüphaneyi kullanacak şekilde güncelleyebiliriz:

```rust
// src/main.rs içinde

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use blog_os::println;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}

/// Bu fonksiyon panic anında çağrılır.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

Kütüphane, normal bir dış crate gibi kullanılabilir. Tıpkı crate'imiz gibi, adı `blog_os`'tur. Yukarıdaki kod, `test_runner` özniteliğinde `blog_os::test_runner` fonksiyonunu ve `cfg(test)` panic handler'ımızda `blog_os::test_panic_handler` fonksiyonunu kullanır. Ayrıca, `_start` ve `panic` fonksiyonlarımıza kullanılabilir kılmak için `println` makrosunu içe aktarır.

Bu noktada `cargo run` ve `cargo test` yeniden çalışmalı. Tabii ki `cargo test` hâlâ sonsuza dek döngüye giriyor (`ctrl+c` ile çıkabilirsiniz). Bunu, gereken kütüphane fonksiyonlarını entegrasyon testimizde kullanarak düzeltelim.

### Entegrasyon Testini Tamamlamak

`src/main.rs`'imiz gibi, `tests/basic_boot.rs` çalıştırılabilir dosyamız da yeni kütüphanemizden tipleri içe aktarabilir. Bu, testimizi tamamlamak için eksik bileşenleri içe aktarmamıza olanak tanır:

```rust
// tests/basic_boot.rs içinde

#![test_runner(blog_os::test_runner)]

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

Test runner'ı yeniden uygulamak yerine, `#![test_runner(crate::test_runner)]` özniteliğini `#![test_runner(blog_os::test_runner)]` olarak değiştirerek kütüphanemizdeki `test_runner` fonksiyonunu kullanıyoruz. Böylece `basic_boot.rs`'teki `test_runner` taslak (stub) fonksiyonuna artık ihtiyacımız kalmaz, onu kaldırabiliriz. `panic` handler'ımız için ise, tıpkı `main.rs`'imizde yaptığımız gibi `blog_os::test_panic_handler` fonksiyonunu çağırıyoruz.

Artık `cargo test` yeniden normal bir şekilde çıkıyor. Onu çalıştırdığınızda, `lib.rs`, `main.rs` ve `basic_boot.rs` için testleri art arda ayrı ayrı derleyip çalıştırdığını görürsünüz. `main.rs` ve `basic_boot` entegrasyon testleri için, bu dosyalarda `#[test_case]` ile işaretlenmiş hiçbir fonksiyon olmadığı için "Running 0 tests" raporlar.

Artık `basic_boot.rs`'imize testler ekleyebiliriz. Örneğin, VGA arabellek testlerinde yaptığımız gibi, `println`'in panic yapmadan çalıştığını test edebiliriz:

```rust
// tests/basic_boot.rs içinde

use blog_os::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}
```

Şimdi `cargo test` çalıştırdığımızda, test fonksiyonunu bulup çalıştırdığını görüyoruz.

Test şu anda biraz işe yaramaz görünebilir, çünkü VGA arabellek testlerinden biriyle neredeyse aynı. Ancak gelecekte, `main.rs` ve `lib.rs`'imizin `_start` fonksiyonları büyüyebilir ve `test_main` fonksiyonunu çalıştırmadan önce çeşitli başlatma rutinlerini çağırabilir; böylece iki test çok farklı ortamlarda çalıştırılır.

`println`'i, `_start`'ta herhangi bir başlatma rutini çağırmadan bir `basic_boot` ortamında test ederek, `println`'in önyüklemeden hemen sonra çalıştığından emin olabiliriz. Bu önemlidir, çünkü ona örneğin panic mesajlarını yazdırmak için güveniyoruz.

### Gelecekteki Testler

Entegrasyon testlerinin gücü, tamamen ayrı çalıştırılabilir dosyalar olarak ele alınmalarıdır. Bu onlara ortam üzerinde tam kontrol verir; bu da kodun CPU veya donanım cihazlarıyla doğru şekilde etkileşip etkileşmediğini test etmeyi mümkün kılar.

`basic_boot` testimiz, bir entegrasyon testinin çok basit bir örneğidir. Gelecekte kernel'imiz çok daha fazla özellik kazanacak ve donanımla çeşitli şekillerde etkileşecek. Entegrasyon testleri ekleyerek, bu etkileşimlerin beklendiği gibi çalıştığından (ve çalışmaya devam ettiğinden) emin olabiliriz. Olası gelecekteki testler için bazı fikirler:

- **CPU Exception'ları**: Kod geçersiz işlemler gerçekleştirdiğinde (örneğin sıfıra bölme), CPU bir exception fırlatır. Kernel, bu tür exception'lar için handler fonksiyonları kaydedebilir. Bir entegrasyon testi, bir CPU exception'ı oluştuğunda doğru exception handler'ının çağrıldığını veya çözülebilir bir exception'dan sonra çalıştırmanın doğru şekilde devam ettiğini doğrulayabilir.
- **Sayfa Tabloları (Page Tables)**: Sayfa tabloları, hangi bellek bölgelerinin geçerli ve erişilebilir olduğunu tanımlar. Sayfa tablolarını değiştirerek, örneğin programları başlatırken yeni bellek bölgeleri ayırmak mümkündür. Bir entegrasyon testi, `_start` fonksiyonunda sayfa tablolarını değiştirebilir ve değişikliklerin `#[test_case]` fonksiyonlarında istenen etkilere sahip olduğunu doğrulayabilir.
- **Kullanıcı Alanı (Userspace) Programları**: Kullanıcı alanı programları, sistemin kaynaklarına sınırlı erişimi olan programlardır. Örneğin, kernel veri yapılarına veya diğer programların belleğine erişimleri yoktur. Bir entegrasyon testi, yasak işlemler gerçekleştiren kullanıcı alanı programları başlatabilir ve kernel'in hepsini engellediğini doğrulayabilir.

Tahmin edebileceğiniz gibi, çok daha fazla test mümkündür. Bu tür testler ekleyerek, kernel'imize yeni özellikler eklediğimizde veya kodumuzu yeniden düzenlediğimizde (refactor) onları yanlışlıkla bozmadığımızdan emin olabiliriz. Bu, özellikle kernel'imiz daha büyük ve daha karmaşık hale geldiğinde önemlidir.

### Panic Etmesi Gereken Testler

Standart kütüphanenin test çerçevesi, başarısız olması gereken testler oluşturmaya olanak tanıyan bir [`#[should_panic]` özniteliğini][should_panic] destekler. Bu, örneğin geçersiz bir argüman geçirildiğinde bir fonksiyonun başarısız olduğunu doğrulamak için yararlıdır. Ne yazık ki bu öznitelik, standart kütüphaneden destek gerektirdiği için `#[no_std]` crate'lerinde desteklenmez.

[should_panic]: https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html#testing-panics

`#[should_panic]` özniteliğini kernel'imizde kullanamasak da, panic handler'dan başarı hata koduyla çıkan bir entegrasyon testi oluşturarak benzer bir davranış elde edebiliriz. `should_panic` adıyla böyle bir test oluşturmaya başlayalım:

```rust
// tests/should_panic.rs içinde

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use blog_os::{QemuExitCode, exit_qemu, serial_println};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

Bu test, henüz bir `_start` fonksiyonu veya özel test runner özniteliklerinden herhangi birini tanımlamadığı için hâlâ eksiktir. Eksik kısımları ekleyelim:

```rust
// tests/should_panic.rs içinde

#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test();
        serial_println!("[test did not panic]");
        exit_qemu(QemuExitCode::Failed);
    }
    exit_qemu(QemuExitCode::Success);
}
```

`lib.rs`'imizdeki `test_runner`'ı yeniden kullanmak yerine, test kendi `test_runner` fonksiyonunu tanımlar; bu fonksiyon, bir test panic yapmadan geri döndüğünde bir başarısızlık çıkış koduyla çıkar (testlerimizin panic yapmasını istiyoruz). Hiçbir test fonksiyonu tanımlanmamışsa, runner bir başarı hata koduyla çıkar. Runner her zaman tek bir testi çalıştırdıktan sonra çıktığı için, birden fazla `#[test_case]` fonksiyonu tanımlamak mantıklı değildir.

Artık başarısız olması gereken bir test oluşturabiliriz:

```rust
// tests/should_panic.rs içinde

use blog_os::serial_print;

#[test_case]
fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}
```

Test, `0` ile `1`'in eşit olduğunu doğrulamak için `assert_eq` kullanır. Tabii ki bu başarısız olur, bu yüzden testimiz istendiği gibi panic yapar. Burada `Testable` trait'ini kullanmadığımız için fonksiyon adını `serial_print!` kullanarak elle yazdırmamız gerektiğine dikkat edin.

Testi `cargo test --test should_panic` aracılığıyla çalıştırdığımızda, beklendiği gibi panic yaptığı için başarılı olduğunu görüyoruz. Assertion'ı yorum satırı haline getirip testi tekrar çalıştırdığımızda, gerçekten _"test did not panic"_ mesajıyla başarısız olduğunu görüyoruz.

Bu yaklaşımın önemli bir dezavantajı, yalnızca tek bir test fonksiyonu için çalışmasıdır. Birden çok `#[test_case]` fonksiyonu olduğunda, panic handler çağrıldıktan sonra çalıştırma devam edemediği için yalnızca ilk fonksiyon çalıştırılır. Şu anda bu sorunu çözmenin iyi bir yolunu bilmiyorum, bu yüzden bir fikriniz varsa bana bildirin!

### Harness'sız Testler {#no-harness-tests}

Yalnızca tek bir test fonksiyonu olan entegrasyon testleri için (`should_panic` testimiz gibi), test runner'a gerçekten gerek yoktur. Bunun gibi durumlarda, test runner'ı tamamen devre dışı bırakabilir ve testimizi doğrudan `_start` fonksiyonunda çalıştırabiliriz.

Bunun anahtarı, testin `Cargo.toml`'daki `harness` bayrağını devre dışı bırakmaktır; bu bayrak, bir entegrasyon testi için bir test runner kullanılıp kullanılmayacağını tanımlar. `false` olarak ayarlandığında, hem varsayılan test runner hem de özel test runner özelliği devre dışı bırakılır; böylece test normal bir çalıştırılabilir dosya gibi ele alınır.

`should_panic` testimiz için `harness` bayrağını devre dışı bırakalım:

```toml
# Cargo.toml içinde

[[test]]
name = "should_panic"
harness = false
```

Şimdi `should_panic` testimizi, `test_runner` ile ilgili kodu kaldırarak büyük ölçüde basitleştiriyoruz. Sonuç şöyle görünür:

```rust
// tests/should_panic.rs içinde

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    loop{}
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

Artık `should_fail` fonksiyonunu doğrudan `_start` fonksiyonumuzdan çağırıyor ve geri dönerse bir başarısızlık çıkış koduyla çıkıyoruz. Şimdi `cargo test --test should_panic` çalıştırdığımızda, testin tam olarak öncekiyle aynı davrandığını görüyoruz.

`should_panic` testleri oluşturmanın yanı sıra, `harness` özniteliğini devre dışı bırakmak karmaşık entegrasyon testleri için de yararlı olabilir; örneğin tek tek test fonksiyonlarının yan etkileri olduğunda ve belirli bir sırada çalıştırılmaları gerektiğinde.

## Özet

Test etme, belirli bileşenlerin istenen davranışa sahip olduğundan emin olmak için çok yararlı bir tekniktir. Hataların yokluğunu gösteremeseler bile, onları bulmak ve özellikle gerilemeleri (regression) önlemek için yine de yararlı bir araçtır.

Bu yazı, Rust kernel'imiz için bir test çerçevesinin nasıl kurulacağını açıkladı. Bare metal ortamımızda basit bir `#[test_case]` özniteliğine yönelik destek uygulamak için Rust'ın özel test çerçeveleri özelliğini kullandık. QEMU'nun `isa-debug-exit` cihazını kullanarak, test runner'ımız testleri çalıştırdıktan sonra QEMU'dan çıkabilir ve test durumunu raporlayabilir. Hata mesajlarını VGA arabelleği yerine konsola yazdırmak için, seri port için temel bir sürücü oluşturduk.

`println` makromuz için bazı testler oluşturduktan sonra, yazının ikinci yarısında entegrasyon testlerini inceledik. Onların `tests` dizininde bulunduğunu ve tamamen ayrı çalıştırılabilir dosyalar olarak ele alındığını öğrendik. Onlara `exit_qemu` fonksiyonuna ve `serial_println` makrosuna erişim vermek için, kodumuzun çoğunu tüm çalıştırılabilir dosyalar ve entegrasyon testleri tarafından içe aktarılabilen bir kütüphaneye taşıdık. Entegrasyon testleri kendi ayrı ortamlarında çalıştığı için, donanımla etkileşimleri test etmeyi veya panic etmesi gereken testler oluşturmayı mümkün kılarlar.

Artık QEMU'nun içinde gerçekçi bir ortamda çalışan bir test çerçevemiz var. Gelecekteki yazılarda daha fazla test oluşturarak, kernel'imiz daha karmaşık hale geldiğinde onu sürdürülebilir tutabiliriz.

## Sırada ne var?

Bir sonraki yazıda _CPU exception'larını_ inceleyeceğiz. Bu exception'lar, sıfıra bölme veya eşlenmemiş bir bellek sayfasına erişim (sözde bir "page fault") gibi yasa dışı bir şey olduğunda CPU tarafından fırlatılır. Bu exception'ları yakalayabilmek ve inceleyebilmek, gelecekteki hataların ayıklanması için çok önemlidir. Exception işleme, klavye desteği için gereken donanım interrupt'larının işlenmesine de çok benzer.
