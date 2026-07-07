+++
title = "Bağımsız Bir Rust İkili Dosyası"
weight = 1
path = "tr/freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "Bare Bones"

# Please update this when updating the translation
translation_based_on_commit = "cf117267301e2876f6cffe54db0597184a6d82a0"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

Kendi işletim sistemi kernel'imizi oluşturmanın ilk adımı, standart kütüphaneyi bağlamayan (link etmeyen) bir Rust çalıştırılabilir dosyası oluşturmaktır. Bu sayede, alttaki bir işletim sistemi olmadan Rust kodunu doğrudan [bare metal] üzerinde çalıştırmak mümkün hale gelir.

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-01`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## Giriş
Bir işletim sistemi kernel'i yazmak için, herhangi bir işletim sistemi özelliğine bağlı olmayan koda ihtiyacımız var. Bu, thread'leri, dosyaları, heap belleğini, ağı, rastgele sayıları, standart çıktıyı veya OS soyutlamaları ya da belirli bir donanımı gerektiren başka herhangi bir özelliği kullanamayacağımız anlamına gelir. Kendi işletim sistemimizi ve kendi sürücülerimizi yazmaya çalıştığımız için bu mantıklıdır.

Bu, [Rust standart kütüphanesinin][Rust standard library] büyük bir kısmını kullanamayacağımız anlamına gelir; ancak _kullanabileceğimiz_ pek çok Rust özelliği vardır. Örneğin [iterator'ları][iterators], [closure'ları][closures], [pattern matching][pattern matching]'i, [option] ile [result]'ı, [string biçimlendirmeyi][string formatting] ve elbette [sahiplik (ownership) sistemini][ownership system] kullanabiliriz. Bu özellikler, [tanımsız davranış (undefined behavior)][undefined behavior] veya [bellek güvenliği (memory safety)][memory safety] konusunda endişelenmeden bir kernel'i son derece ifade gücü yüksek ve üst düzey bir şekilde yazmayı mümkün kılar.

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

Rust ile bir OS kernel'i oluşturmak için, alttaki bir işletim sistemi olmadan çalıştırılabilen bir çalıştırılabilir dosya oluşturmamız gerekir. Böyle bir çalıştırılabilir dosyaya genellikle “freestanding” (bağımsız) veya “bare-metal” çalıştırılabilir dosya denir.

Bu yazı, bağımsız bir Rust ikili dosyası oluşturmak için gereken adımları açıklar ve bu adımların neden gerekli olduğunu anlatır. Yalnızca minimal bir örnekle ilgileniyorsanız, doğrudan **[özete geçebilirsiniz](#summary)**.

## Standart Kütüphaneyi Devre Dışı Bırakmak
Varsayılan olarak tüm Rust crate'leri, thread'ler, dosyalar veya ağ gibi özellikler için işletim sistemine bağımlı olan [standart kütüphaneyi][standard library] bağlar. Ayrıca, OS hizmetleriyle yakından etkileşen C standart kütüphanesi `libc`'ye de bağımlıdır. Planımız bir işletim sistemi yazmak olduğundan, OS'a bağımlı hiçbir kütüphaneyi kullanamayız. Bu yüzden standart kütüphanenin otomatik olarak dahil edilmesini [`no_std` özniteliği][`no_std` attribute] aracılığıyla devre dışı bırakmamız gerekir.

[standard library]: https://doc.rust-lang.org/std/
[`no_std` attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

İşe yeni bir cargo uygulama projesi oluşturarak başlıyoruz. Bunu yapmanın en kolay yolu komut satırıdır:

```
cargo new blog_os --bin --edition 2024
```

Projeyi `blog_os` olarak adlandırdım, fakat elbette kendi isminizi seçebilirsiniz. `--bin` bayrağı, (bir kütüphanenin aksine) çalıştırılabilir bir ikili dosya oluşturmak istediğimizi belirtir; `--edition 2024` bayrağı ise crate'imiz için Rust'ın [2024 sürümünü][2024 edition] kullanmak istediğimizi belirtir. Bu komutu çalıştırdığımızda, cargo bizim için aşağıdaki dizin yapısını oluşturur:

[2024 edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2024/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

`Cargo.toml` dosyası, crate yapılandırmasını içerir; örneğin crate adı, yazar, [semantik sürüm][semantic version] numarası ve bağımlılıklar. `src/main.rs` dosyası ise crate'imizin kök modülünü ve `main` fonksiyonumuzu içerir. Crate'inizi `cargo build` ile derleyebilir ve ardından derlenmiş `blog_os` ikili dosyasını `target/debug` alt klasöründe çalıştırabilirsiniz.

[semantic version]: https://semver.org/

### `no_std` Özniteliği

Şu anda crate'imiz örtük olarak standart kütüphaneyi bağlıyor. [`no_std` özniteliğini][`no_std` attribute] ekleyerek bunu devre dışı bırakmayı deneyelim:

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

Şimdi bunu derlemeye çalıştığımızda (`cargo build` komutunu çalıştırarak), aşağıdaki hata oluşur:

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

Bu hatanın nedeni, [`println` makrosunun][`println` macro] artık dahil etmediğimiz standart kütüphanenin bir parçası olmasıdır. Dolayısıyla artık bir şeyler yazdıramayız. Bu mantıklıdır, çünkü `println`, işletim sistemi tarafından sağlanan özel bir dosya tanımlayıcısı olan [standart çıktıya][standard output] yazar.

[`println` macro]: https://doc.rust-lang.org/std/macro.println.html
[standard output]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

O halde yazdırma kısmını kaldıralım ve boş bir main fonksiyonuyla tekrar deneyelim:

```rust
// main.rs

#![no_std]

fn main() {}
```

```
> cargo build
error: `#[panic_handler]` function required, but not found
error: unwinding panics are not supported without std
```

Artık derleyici bir `#[panic_handler]` fonksiyonunu bulamıyor ve _unwinding_'in standart kütüphane olmadan mümkün olmadığından şikâyet ediyor. Her iki hataya da aşağıdaki bölümlerde bakacağız.

<div class="note">

Eski Rust toolchain'lerinde ikinci hata bunun yerine `language item required, but not found: eh_personality` şeklinde görünür; bu, `unwinding panics are not supported without std` hatasıyla aynı nedene sahiptir.

</div>

## Panic Implementasyonu

`panic_handler` özniteliği, bir [panic] gerçekleştiğinde derleyicinin çağırması gereken fonksiyonu tanımlar. Standart kütüphane kendi panic handler fonksiyonunu sağlar; ancak bir `no_std` ortamında bunu kendimiz tanımlamamız gerekir:

[panic]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// main.rs içinde

use core::panic::PanicInfo;

/// Bu fonksiyon panic anında çağrılır.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

[`PanicInfo` parametresi][PanicInfo], panic'in gerçekleştiği dosyayı ve satırı, ayrıca isteğe bağlı panic mesajını içerir. Bu fonksiyon asla geri dönmemelidir, bu yüzden [“never” tipini][“never” type] `!` döndürerek [ıraksayan fonksiyon (diverging function)][diverging function] olarak işaretlenmiştir. Şimdilik bu fonksiyonda yapabileceğimiz pek bir şey yok, bu yüzden sadece sonsuza dek döngüye giriyoruz.

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[“never” type]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## Unwinding

Rust, bir [panic] durumunda canlı olan tüm stack değişkenlerinin destructor'larını çalıştırmak için varsayılan olarak [stack unwinding] kullanır. Bu, kullanılan tüm belleğin serbest bırakılmasını sağlar ve üst thread'in panic'i yakalayıp çalışmaya devam etmesine olanak tanır. Ancak unwinding karmaşık bir süreçtir ve bazı OS'a özgü kütüphaneler gerektirir (örneğin Linux'ta [libunwind] veya Windows'ta [structured exception handling]); bunlar da Rust standart kütüphanesini gerektirir. Bunun sonucu olarak, `no_std` işletim sistemi kernel'imiz için unwinding kullanamayız.

<div class="note">

Eski Rust toolchain'lerindeki `language item required, but not found: eh_personality` hatası da unwinding'e işaret eder. [`eh_personality` dil öğesi][`eh_personality` language item], stack unwinding uygulamak için kullanılması gereken fonksiyonu işaretler. Dil öğeleri (language items), derleyicinin dahili olarak ihtiyaç duyduğu özel öğelerdir (trait'ler, fonksiyonlar, tipler vb.). Yeni Rust toolchain'lerinde, bu uygulama detayından bahsetmekten kaçınmak için hata mesajı iyileştirildi.

</div>

[`eh_personality` language item]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: https://www.nongnu.org/libunwind/
[structured exception handling]: https://docs.microsoft.com/en-us/windows/win32/debug/structured-exception-handling

### Unwinding'i Devre Dışı Bırakmak

Unwinding'in istenmediği başka kullanım senaryoları da vardır, bu yüzden Rust bunun yerine [panic'te abort etme][abort on panic] seçeneği sunar. Bu, unwinding sembol bilgisinin üretilmesini devre dışı bırakır ve böylece ikili dosya boyutunu önemli ölçüde azaltır. Unwinding'i devre dışı bırakabileceğimiz birden fazla yer vardır. En kolay yol, aşağıdaki satırları `Cargo.toml` dosyamıza eklemektir:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

Bu, hem (`cargo build` için kullanılan) `dev` profili hem de (`cargo build --release` için kullanılan) `release` profili için panic stratejisini `abort` olarak ayarlar. Artık `unwinding panics are not supported without std` hatası düzelmiş olmalı.

[abort on panic]: https://github.com/rust-lang/rust/pull/32900

Şimdi derlemeye çalışırsak, yeni bir hata oluşur:

```
> cargo build
error: using `fn main` requires the standard library
```

<div class="note">

Eski Rust toolchain'leri bunu bunun yerine `error: requires start lang_item` olarak bildirir. `start` dil öğesi, daha sonra `main` fonksiyonunu çağıran alttaki giriş noktasını tanımlar. Dolayısıyla bu hata, yeni toolchain'lerdeki ``using `fn main` requires the standard library`` hatasıyla aynı nedene sahiptir.

</div>

## Giriş Noktası

`main` fonksiyonunun, programın "giriş noktası" (entry point), yani bir programı çalıştırdığınızda çağrılan ilk fonksiyon olduğu düşünülebilir. Ancak çoğu dilin, çöp toplama (örneğin Java'da) veya yazılımsal thread'ler (örneğin Go'daki goroutine'ler) gibi şeylerden sorumlu bir [runtime sistemi][runtime system] vardır. Bu runtime'ın `main`'den önce çağrılması gerekir, çünkü kendisini başlatması gerekir.

[runtime system]: https://en.wikipedia.org/wiki/Runtime_system

Standart kütüphaneyi bağlayan tipik bir Rust ikili dosyasında yürütme, bir C uygulaması için ortamı hazırlayan `crt0` (“C runtime zero”) adlı bir C runtime kütüphanesinde başlar. Buna bir stack oluşturmak ve argümanları doğru register'lara yerleştirmek dahildir. C runtime daha sonra `start` dil öğesiyle işaretlenmiş olan [Rust runtime'ının giriş noktasını][rt::lang_start] çağırır. Rust'ın yalnızca çok minimal bir runtime'ı vardır; bu runtime, stack taşması koruyucularını ayarlamak veya panic anında bir backtrace yazdırmak gibi birkaç küçük işle ilgilenir. Runtime en sonunda `main` fonksiyonunu çağırır.

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

Bağımsız çalıştırılabilir dosyamızın Rust runtime'ına ve `crt0`'a erişimi yoktur, bu yüzden kendi giriş noktamızı tanımlamamız gerekir ve yalnızca bir `main` fonksiyonu tanımlayamayız.

### Giriş Noktasının Üzerine Yazmak
Rust derleyicisine normal giriş noktası zincirini kullanmak istemediğimizi bildirmek için `#![no_main]` özniteliğini ekliyoruz.

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// Bu fonksiyon panic anında çağrılır.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

`main` fonksiyonunu kaldırdığımızı fark etmiş olabilirsiniz. Bunun nedeni, onu çağıran alttaki bir runtime olmadan `main`'in anlamlı olmamasıdır. Bunun yerine, artık işletim sistemi giriş noktasının üzerine kendi `_start` fonksiyonumuzu yazıyoruz:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

`#[unsafe(no_mangle)]` özniteliğini kullanarak, Rust derleyicisinin gerçekten `_start` adında bir fonksiyon üretmesini sağlamak için [isim parçalamayı (name mangling)][name mangling] devre dışı bırakıyoruz. Bu öznitelik olmadan derleyici, her fonksiyona benzersiz bir isim vermek için `_ZN3blog_os4_start7hb173fedf945531caE` gibi şifreli bir sembol üretirdi. Bu öznitelik gereklidir, çünkü bir sonraki adımda giriş noktası fonksiyonunun adını linker'a bildirmemiz gerekir.

Ayrıca fonksiyonu, derleyiciye bu fonksiyon için (belirtilmemiş Rust çağırma kuralı yerine) [C çağırma kuralını (calling convention)][C calling convention] kullanması gerektiğini bildirmek için `extern "C"` olarak işaretlemeliyiz. Fonksiyona `_start` adını vermemizin nedeni, bunun çoğu sistem için varsayılan giriş noktası adı olmasıdır.

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[C calling convention]: https://en.wikipedia.org/wiki/Calling_convention

`!` dönüş tipi, fonksiyonun ıraksayan olduğu, yani asla geri dönmesine izin verilmediği anlamına gelir. Bu gereklidir, çünkü giriş noktası herhangi bir fonksiyon tarafından değil, doğrudan işletim sistemi veya bootloader tarafından çağrılır. Bu yüzden giriş noktası, geri dönmek yerine örneğin işletim sisteminin [`exit` sistem çağrısını][`exit` system call] çağırmalıdır. Bizim durumumuzda, bağımsız bir ikili dosya geri döndüğünde yapacak bir şey kalmadığı için makineyi kapatmak makul bir eylem olabilir. Şimdilik gereksinimi sonsuz bir döngüyle yerine getiriyoruz.

[`exit` system call]: https://en.wikipedia.org/wiki/Exit_(system_call)

Şimdi `cargo build` komutunu çalıştırdığımızda, çirkin bir _linker_ hatası alırız.

## Linker Hataları

Linker, üretilen kodu bir çalıştırılabilir dosyada birleştiren bir programdır. Çalıştırılabilir dosya biçimi Linux, Windows ve macOS arasında farklılık gösterdiğinden, her sistemin farklı bir hata veren kendi linker'ı vardır. Hataların temel nedeni aynıdır: linker'ın varsayılan yapılandırması, programımızın C runtime'a bağımlı olduğunu varsayar; ama aslında değildir.

Hataları çözmek için, linker'a C runtime'ı dahil etmemesi gerektiğini söylememiz gerekir. Bunu, ya linker'a belirli bir argüman kümesi geçirerek ya da bir bare metal hedefi (target) için derleyerek yapabiliriz.

### Bare Metal Hedefi İçin Derlemek

Varsayılan olarak Rust, mevcut sistem ortamınızda çalışabilen bir çalıştırılabilir dosya oluşturmaya çalışır. Örneğin, `x86_64` üzerinde Windows kullanıyorsanız, Rust `x86_64` komutlarını kullanan bir `.exe` Windows çalıştırılabilir dosyası oluşturmaya çalışır. Bu ortama "host" (ana) sisteminiz denir.

Farklı ortamları tanımlamak için Rust, [_target triple_] adı verilen bir dize kullanır. Host sisteminiz için target triple'ı `rustc --version --verbose` komutunu çalıştırarak görebilirsiniz:

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

Yukarıdaki çıktı bir `x86_64` Linux sistemine aittir. `host` triple'ının; CPU mimarisini (`x86_64`), satıcıyı (`unknown`), işletim sistemini (`linux`) ve [ABI]'yi (`gnu`) içeren `x86_64-unknown-linux-gnu` olduğunu görüyoruz.

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

Host triple'ımız için derlediğimizde, Rust derleyicisi ve linker, varsayılan olarak C runtime'ı kullanan Linux veya Windows gibi alttaki bir işletim sistemi olduğunu varsayar; bu da linker hatalarına neden olur. Dolayısıyla, linker hatalarından kaçınmak için alttaki bir işletim sistemi olmayan farklı bir ortam için derleyebiliriz.

Böyle bir bare metal ortamına örnek olarak, [gömülü (embedded)][embedded] bir [ARM] sistemini tanımlayan `thumbv7em-none-eabihf` target triple'ı verilebilir. Ayrıntılar önemli değil; önemli olan tek şey, target triple'da `none` ile belirtildiği gibi alttaki bir işletim sisteminin olmamasıdır. Bu hedef için derleyebilmek üzere onu rustup'a eklememiz gerekir:

[embedded]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture

```
rustup target add thumbv7em-none-eabihf
```

Bu komut, sistem için standart (ve core) kütüphanenin bir kopyasını indirir. Artık bağımsız çalıştırılabilir dosyamızı bu hedef için derleyebiliriz:

```
cargo build --target thumbv7em-none-eabihf
```

Bir `--target` argümanı geçirerek, çalıştırılabilir dosyamızı bir bare metal hedef sistemi için [çapraz derleriz (cross compile)][cross compile]. Hedef sistemin işletim sistemi olmadığı için, linker C runtime'ı bağlamaya çalışmaz ve derlememiz herhangi bir linker hatası olmadan başarılı olur.

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

OS kernel'imizi derlemek için kullanacağımız yaklaşım budur. `thumbv7em-none-eabihf` yerine, bir `x86_64` bare metal ortamını tanımlayan [özel bir hedef (custom target)][custom target] kullanacağız. Ayrıntılar bir sonraki yazıda açıklanacaktır.

[custom target]: https://doc.rust-lang.org/rustc/targets/custom.html

### Linker Argümanları

Bir bare metal sistemi için derlemek yerine, linker'a belirli bir argüman kümesi geçirerek de linker hatalarını çözmek mümkündür. Bu, kernel'imiz için kullanacağımız yaklaşım değildir; bu yüzden bu bölüm isteğe bağlıdır ve yalnızca bütünlük için sunulmuştur. İsteğe bağlı içeriği göstermek için aşağıdaki _"Linker Argümanları"_na tıklayın.

<details>

<summary>Linker Argümanları</summary>

Bu bölümde Linux, Windows ve macOS'ta oluşan linker hatalarını ele alıyor ve bunları linker'a ek argümanlar geçirerek nasıl çözeceğimizi açıklıyoruz. Çalıştırılabilir dosya biçiminin ve linker'ın işletim sistemleri arasında farklılık gösterdiğini, bu yüzden her sistem için farklı bir argüman kümesinin gerekli olduğunu unutmayın.

#### Linux

Linux'ta aşağıdaki linker hatası oluşur (kısaltılmış):

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

Sorun şu ki, linker varsayılan olarak C runtime'ın, yine `_start` adı verilen başlangıç rutinini dahil eder. Bu rutin, `no_std` özniteliği nedeniyle dahil etmediğimiz C standart kütüphanesi `libc`'nin bazı sembollerine ihtiyaç duyar, bu yüzden linker bu referansları çözemez. Bunu çözmek için, `-nostartfiles` bayrağını geçirerek linker'a C başlangıç rutinini bağlamaması gerektiğini söyleyebiliriz.

Linker özniteliklerini cargo aracılığıyla geçirmenin bir yolu `cargo rustc` komutudur. Bu komut tam olarak `cargo build` gibi davranır, ancak alttaki Rust derleyicisi `rustc`'ye seçenek geçirmeye olanak tanır. `rustc`'nin, linker'a bir argüman geçiren `-C link-arg` bayrağı vardır. Bunları birleştirdiğimizde yeni derleme komutumuz şöyle görünür:

```
cargo rustc -- -C link-arg=-nostartfiles
```

Artık crate'imiz Linux'ta bağımsız bir çalıştırılabilir dosya olarak derleniyor!

Giriş noktası fonksiyonumuzun adını açıkça belirtmemize gerek kalmadı, çünkü linker varsayılan olarak `_start` adında bir fonksiyon arar.

#### Windows

Windows'ta farklı bir linker hatası oluşur (kısaltılmış):

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

"entry point must be defined" (giriş noktası tanımlanmalıdır) hatası, linker'ın giriş noktasını bulamadığı anlamına gelir. Windows'ta varsayılan giriş noktası adı [kullanılan alt sisteme bağlıdır][windows-subsystems]. `CONSOLE` alt sistemi için linker `mainCRTStartup` adında bir fonksiyon arar; `WINDOWS` alt sistemi için ise `WinMainCRTStartup` adında bir fonksiyon arar. Varsayılanı geçersiz kılmak ve linker'a bunun yerine `_start` fonksiyonumuzu aramasını söylemek için, linker'a bir `/ENTRY` argümanı geçirebiliriz:

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

Farklı argüman biçiminden, Windows linker'ının Linux linker'ından tamamen farklı bir program olduğunu açıkça görüyoruz.

Şimdi farklı bir linker hatası oluşuyor:

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

Bu hata, Windows çalıştırılabilir dosyalarının farklı [alt sistemler][windows-subsystems] kullanabilmesi nedeniyle oluşur. Normal programlar için alt sistem, giriş noktası adına bağlı olarak çıkarsanır: Giriş noktası `main` olarak adlandırılmışsa `CONSOLE` alt sistemi, `WinMain` olarak adlandırılmışsa `WINDOWS` alt sistemi kullanılır. `_start` fonksiyonumuzun farklı bir adı olduğu için, alt sistemi açıkça belirtmemiz gerekir:

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

Burada `CONSOLE` alt sistemini kullanıyoruz, ancak `WINDOWS` alt sistemi de işe yarardı. `-C link-arg`'ı birden çok kez geçirmek yerine, boşlukla ayrılmış bir argüman listesi alan `-C link-args`'ı kullanıyoruz.

Bu komutla, çalıştırılabilir dosyamız Windows'ta başarıyla derlenmelidir.

#### macOS

macOS'ta aşağıdaki linker hatası oluşur (kısaltılmış):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

Bu hata mesajı bize, linker'ın varsayılan `main` adıyla bir giriş noktası fonksiyonu bulamadığını söylüyor (bir nedenden ötürü, macOS'ta tüm fonksiyonların önüne `_` eklenir). Giriş noktasını `_start` fonksiyonumuza ayarlamak için `-e` linker argümanını geçiriyoruz:

```
cargo rustc -- -C link-args="-e __start"
```

`-e` bayrağı, giriş noktası fonksiyonunun adını belirtir. macOS'ta tüm fonksiyonların ek bir `_` öneki olduğu için, giriş noktasını `_start` yerine `__start` olarak ayarlamamız gerekir.

Şimdi aşağıdaki linker hatası oluşuyor:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS [statik olarak bağlanmış ikili dosyaları resmi olarak desteklemez][does not officially support statically linked binaries] ve programların varsayılan olarak `libSystem` kütüphanesini bağlamasını gerektirir. Bunu geçersiz kılmak ve statik bir ikili dosya bağlamak için, linker'a `-static` bayrağını geçiriyoruz:

[does not officially support statically linked binaries]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

Bu hâlâ yeterli değil, çünkü üçüncü bir linker hatası oluşuyor:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

Bu hata, macOS'taki programların varsayılan olarak `crt0`'a (“C runtime zero”) bağlanması nedeniyle oluşur. Bu, Linux'ta karşılaştığımız hataya benzer ve `-nostartfiles` linker argümanı eklenerek de çözülebilir:

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Artık programımız macOS'ta başarıyla derlenmelidir.

#### Derleme Komutlarını Birleştirmek

Şu anda host platforma bağlı olarak farklı derleme komutlarımız var; bu ideal değil. Bundan kaçınmak için, platforma özgü argümanları içeren `.cargo/config.toml` adında bir dosya oluşturabiliriz:

```toml
# .cargo/config.toml içinde

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

`rustflags` anahtarı, her `rustc` çağrısına otomatik olarak eklenen argümanları içerir. `.cargo/config.toml` dosyası hakkında daha fazla bilgi için [resmi belgelere](https://doc.rust-lang.org/cargo/reference/config.html) göz atın.

Artık programımız üç platformun hepsinde basit bir `cargo build` ile derlenebilir olmalı.

#### Bunu Yapmalı mısınız?

Linux, Windows ve macOS için bağımsız bir çalıştırılabilir dosya oluşturmak mümkün olsa da, bu muhtemelen iyi bir fikir değildir. Bunun nedeni, çalıştırılabilir dosyamızın hâlâ çeşitli şeyleri beklemesidir; örneğin `_start` fonksiyonu çağrıldığında bir stack'in başlatılmış olmasını. C runtime olmadan, bu gereksinimlerin bazıları karşılanmayabilir; bu da programımızın örneğin bir segmentasyon hatası (segmentation fault) nedeniyle başarısız olmasına neden olabilir.

Mevcut bir işletim sisteminin üzerinde çalışan minimal bir ikili dosya oluşturmak istiyorsanız, `libc`'yi dahil etmek ve `#[start]` özniteliğini [burada](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html) açıklandığı gibi ayarlamak muhtemelen daha iyi bir fikirdir.

</details>

## Özet {#summary}

Minimal bir bağımsız Rust ikili dosyası şöyle görünür:

`src/main.rs`:

```rust
#![no_std] // Rust standart kütüphanesini bağlama
#![no_main] // tüm Rust seviyesindeki giriş noktalarını devre dışı bırak

use core::panic::PanicInfo;

#[unsafe(no_mangle)] // bu fonksiyonun adını parçalama (mangle etme)
pub extern "C" fn _start() -> ! {
    // bu fonksiyon giriş noktasıdır, çünkü linker varsayılan olarak
    // `_start` adında bir fonksiyon arar
    loop {}
}

/// Bu fonksiyon panic anında çağrılır.
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

# `cargo build` için kullanılan profil
[profile.dev]
panic = "abort" # panic anında stack unwinding'i devre dışı bırak

# `cargo build --release` için kullanılan profil
[profile.release]
panic = "abort" # panic anında stack unwinding'i devre dışı bırak
```

Bu ikili dosyayı derlemek için, `thumbv7em-none-eabihf` gibi bir bare metal hedefi için derlememiz gerekir:

```
cargo build --target thumbv7em-none-eabihf
```

Alternatif olarak, ek linker argümanları geçirerek onu host sistem için derleyebiliriz:

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Bunun, bağımsız bir Rust ikili dosyasının yalnızca minimal bir örneği olduğunu unutmayın. Bu ikili dosya çeşitli şeyleri bekler; örneğin `_start` fonksiyonu çağrıldığında bir stack'in başlatılmış olmasını. **Bu yüzden böyle bir ikili dosyanın herhangi bir gerçek kullanımı için daha fazla adım gereklidir**.

## `rust-analyzer`'ı Mutlu Etmek {#making-rust-analyzer-happy}

[`rust-analyzer`](https://rust-analyzer.github.io/) projesi, editörünüzde Rust kodu için kod tamamlama ve "tanıma git" (go to definition) desteği (ve daha pek çok özellik) elde etmenin harika bir yoludur.
`#![no_std]` projeleri için de gerçekten iyi çalışır, bu yüzden kernel geliştirme için onu kullanmanızı öneririm!

`rust-analyzer`'ın [`checkOnSave`](https://rust-analyzer.github.io/book/configuration.html#checkOnSave) özelliğini (varsayılan olarak etkin) kullanıyorsanız, kernel'imizin panic fonksiyonu için bir hata bildirebilir:

```
found duplicate lang item `panic_impl`
```

Bu hatanın nedeni, `rust-analyzer`'ın varsayılan olarak `cargo check --all-targets` komutunu çalıştırması ve bunun ikili dosyayı [test](https://doc.rust-lang.org/book/ch11-01-writing-tests.html) ve [benchmark](https://doc.rust-lang.org/rustc/tests/index.html#benchmarks) modunda da derlemeye çalışmasıdır.

<div class="note">

### "target" Kelimesinin İki Anlamı

`--all-targets` bayrağının `--target` argümanıyla hiçbir ilgisi yoktur.
`cargo`'da "target" teriminin iki farklı anlamı vardır:

- `--target` bayrağı, `rustc` derleyicisine geçirilmesi gereken **[_derleme hedefini (compilation target)_][_compilation target_]** belirtir. Bu, kodumuzu çalıştıracak makinenin [target triple]'ına ayarlanmalıdır.
- `--all-targets` bayrağı, Cargo'nun **[_paket hedefine (package target)_][_package target_]** atıfta bulunur. Cargo paketleri aynı anda hem kütüphane hem ikili dosya olabilir, bu yüzden crate'inizi hangi şekilde derlemek istediğinizi belirtebilirsiniz. Buna ek olarak Cargo'nun ayrıca [örnekler (examples)](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#examples), [testler (tests)](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#tests) ve [benchmark'lar](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#benchmarks) için de paket hedefleri vardır. Bu paket hedefleri bir arada bulunabilir, böylece aynı crate'i örneğin kütüphane veya test modunda derleyebilir/denetleyebilirsiniz.

[_compilation target_]: https://doc.rust-lang.org/rustc/targets/index.html
[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[_package target_]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html

</div>

Varsayılan olarak `cargo check` yalnızca _kütüphane_ ve _ikili dosya_ paket hedeflerini derler.
Ancak `rust-analyzer`, [`checkOnSave`](https://rust-analyzer.github.io/book/configuration.html#checkOnSave) etkin olduğunda varsayılan olarak tüm paket hedeflerini denetlemeyi seçer.
`rust-analyzer`'ın, `cargo check`'te görmediğimiz yukarıdaki `lang item` hatasını bildirmesinin nedeni budur.
`cargo check --all-targets` komutunu çalıştırırsak, biz de hatayı görürüz:

```
error[E0152]: found duplicate lang item `panic_impl`
  --> src/main.rs:13:1
   |
13 | / fn panic(_info: &PanicInfo) -> ! {
14 | |     loop {}
15 | | }
   | |_^
   |
   = note: the lang item is first defined in crate `std` (which `test` depends on)
   = note: first definition in `std` loaded from /home/[...]/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/lib/libstd-8df6be531efb3fd0.rlib
   = note: second definition in the local crate (`blog_os`)
```

İlk `note`, panic dil öğesinin, `test` crate'inin bir bağımlılığı olan `std` crate'inde zaten tanımlı olduğunu söyler.
`test` crate'i, bir crate [test modunda](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#tests) derlenirken otomatik olarak dahil edilir.
Bu, bare metal üzerinde standart kütüphaneyi desteklemenin bir yolu olmadığı için `#![no_std]` kernel'imiz açısından anlamlı değildir.
Dolayısıyla bu hata projemizle ilgili değildir ve onu güvenle göz ardı edebiliriz.

Bu hatadan kaçınmanın doğru yolu, `Cargo.toml` dosyamızda ikili dosyamızın `test` ve `bench` modlarında derlemeyi desteklemediğini belirtmektir.
Bunu, ikili dosyamızın [derlemesini yapılandırmak](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#configuring-a-target) için `Cargo.toml` dosyamıza bir `[[bin]]` bölümü ekleyerek yapabiliriz:

```toml
# Cargo.toml içinde

[[bin]]
name = "blog_os"
test = false
bench = false
```

`bin` etrafındaki çift köşeli parantezler bir hata değildir; TOML biçimi, birden çok kez görünebilen anahtarları bu şekilde tanımlar.
Bir crate birden çok ikili dosyaya sahip olabileceğinden, `[[bin]]` bölümü de `Cargo.toml` içinde birden çok kez görünebilir.
Bu, zorunlu `name` alanının da nedenidir; bu alanın ikili dosyanın adıyla eşleşmesi gerekir (böylece `cargo`, hangi ayarların hangi ikili dosyaya uygulanacağını bilir).

[`test`](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-test-field) ve [`bench`](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-bench-field) alanlarını `false` olarak ayarlayarak, `cargo`'ya ikili dosyamızı test veya benchmark modunda derlememesini söyleriz.
Artık `cargo check --all-targets` herhangi bir hata vermemeli ve `rust-analyzer`'ın `checkOnSave` uygulaması da mutlu olmalı.

## Sırada Ne Var?

[Sonraki yazı][next post], bağımsız ikili dosyamızı minimal bir işletim sistemi kernel'ine dönüştürmek için gereken adımları açıklar. Buna özel bir hedef oluşturmak, çalıştırılabilir dosyamızı bir bootloader ile birleştirmek ve ekrana bir şeyler yazdırmayı öğrenmek dahildir.

[next post]: @/edition-2/posts/02-minimal-rust-kernel/index.tr.md
