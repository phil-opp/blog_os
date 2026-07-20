+++
title = "Minimal Bir Rust Kernel'i"
weight = 2
path = "tr/minimal-rust-kernel"
date = 2018-02-10

[extra]
chapter = "Bare Bones"

# Please update this when updating the translation
translation_based_on_commit = "1132d7a3835dc6c0b3fd8f6b45c9295a9bc1f837"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

Bu yazıda, x86 mimarisi için minimal bir 64-bit Rust kernel'i oluşturuyoruz. Ekrana bir şeyler yazdıran, önyüklenebilir bir disk imajı oluşturmak için önceki yazıdaki [bağımsız Rust ikili dosyasının][freestanding Rust binary] üzerine inşa ediyoruz.

[freestanding Rust binary]: @/edition-2/posts/01-freestanding-rust-binary/index.tr.md

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-02`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-02

<!-- toc -->

## Önyükleme Süreci {#the-boot-process}
Bir bilgisayarı açtığınızda, anakart [ROM]'unda saklanan firmware kodunu çalıştırmaya başlar. Bu kod bir [açılış öz testi (power-on self-test)] gerçekleştirir, mevcut RAM'i tespit eder ve CPU ile donanımı ön başlatma işlemine tabi tutar. Ardından önyüklenebilir bir disk arar ve işletim sistemi kernel'ini önyüklemeye başlar.

[ROM]: https://en.wikipedia.org/wiki/Read-only_memory
[açılış öz testi (power-on self-test)]: https://en.wikipedia.org/wiki/Power-on_self-test

x86'da iki firmware standardı vardır: “Basic Input/Output System” (**[BIOS]**) ve daha yeni olan “Unified Extensible Firmware Interface” (**[UEFI]**). BIOS standardı eski ve modası geçmiştir, ancak basittir ve 1980'lerden beri herhangi bir x86 makinesinde iyi desteklenir. UEFI ise buna karşılık daha moderndir ve çok daha fazla özelliğe sahiptir, ancak kurulumu daha karmaşıktır (en azından bana göre).

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[UEFI]: https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

Şu anda yalnızca BIOS desteği sağlıyoruz, ancak UEFI desteği de planlanıyor. Bu konuda bize yardım etmek isterseniz, [Github issue](https://github.com/phil-opp/blog_os/issues/349)'ya göz atın.

### BIOS Önyüklemesi
Neredeyse tüm x86 sistemleri, öykünülmüş (emulated) bir BIOS kullanan daha yeni UEFI tabanlı makineler de dahil olmak üzere BIOS önyüklemesini destekler. Bu harika bir şey, çünkü geçen yüzyıldan kalma tüm makineler arasında aynı önyükleme mantığını kullanabilirsiniz. Ancak bu geniş uyumluluk aynı zamanda BIOS önyüklemesinin en büyük dezavantajıdır, çünkü bu, 1980'lerden kalma çok eski bootloader'ların hâlâ çalışabilmesi için, önyüklemeden önce CPU'nun [real mode] adı verilen 16-bit bir uyumluluk moduna sokulması anlamına gelir.

Ama en baştan başlayalım:

Bir bilgisayarı açtığınızda, BIOS'u anakart üzerinde bulunan özel bir flash bellekten yükler. BIOS, donanımın öz testi ve başlatma rutinlerini çalıştırır, ardından önyüklenebilir diskler arar. Bir tane bulursa, kontrol onun _bootloader_'ına aktarılır; bu, diskin başında saklanan 512 baytlık bir çalıştırılabilir kod parçasıdır. Çoğu bootloader 512 bayttan büyüktür, bu yüzden bootloader'lar genellikle 512 bayta sığan küçük bir birinci aşamaya ve birinci aşama tarafından sonradan yüklenen bir ikinci aşamaya bölünür.

Bootloader'ın, disk üzerindeki kernel imajının konumunu belirlemesi ve onu belleğe yüklemesi gerekir. Ayrıca CPU'yu önce 16-bit [real mode]'dan 32-bit [protected mode]'a, ardından 64-bit register'ların ve tüm ana belleğin kullanılabilir olduğu 64-bit [long mode]'a geçirmesi gerekir. Üçüncü görevi ise BIOS'tan belirli bilgileri (örneğin bir bellek haritası) sorgulayıp bunları OS kernel'ine iletmektir.

[real mode]: https://en.wikipedia.org/wiki/Real_mode
[protected mode]: https://en.wikipedia.org/wiki/Protected_mode
[long mode]: https://en.wikipedia.org/wiki/Long_mode
[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation

Bir bootloader yazmak biraz zahmetlidir, çünkü assembly dili ve “şu sihirli değeri şu işlemci register'ına yaz” gibi pek çok aydınlatıcı olmayan adım gerektirir. Bu yüzden bu yazıda bootloader oluşturmayı ele almıyoruz; bunun yerine, kernel'inize otomatik olarak bir bootloader ekleyen [bootimage] adlı bir araç sağlıyoruz.

[bootimage]: https://github.com/rust-osdev/bootimage

Kendi bootloader'ınızı oluşturmakla ilgileniyorsanız: Takipte kalın, bu konuda bir dizi yazı şimdiden planlanmış durumda! <!-- , check out our “_[Writing a Bootloader]_” posts, where we explain in detail how a bootloader is built. -->

#### Multiboot Standardı
Her işletim sisteminin yalnızca tek bir OS ile uyumlu olan kendi bootloader'ını uygulamasını önlemek için, [Free Software Foundation] 1995 yılında [Multiboot] adlı açık bir bootloader standardı oluşturdu. Standart, bootloader ile işletim sistemi arasında bir arayüz tanımlar; böylece Multiboot uyumlu herhangi bir bootloader, Multiboot uyumlu herhangi bir işletim sistemini yükleyebilir. Referans uygulama, Linux sistemleri için en popüler bootloader olan [GNU GRUB]'dur.

[Free Software Foundation]: https://en.wikipedia.org/wiki/Free_Software_Foundation
[Multiboot]: https://wiki.osdev.org/Multiboot
[GNU GRUB]: https://en.wikipedia.org/wiki/GNU_GRUB

Bir kernel'i Multiboot uyumlu hale getirmek için, kernel dosyasının başına [Multiboot header] adı verilen bir başlık eklemek yeterlidir. Bu, bir OS'u GRUB'tan önyüklemeyi çok kolaylaştırır. Ancak GRUB ve Multiboot standardının bazı sorunları da vardır:

[Multiboot header]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format

- Yalnızca 32-bit protected mode'u desteklerler. Bu, 64-bit long mode'a geçmek için yine de CPU yapılandırmasını yapmanız gerektiği anlamına gelir.
- Kernel'i değil, bootloader'ı basit kılacak şekilde tasarlanmışlardır. Örneğin kernel'in, [ayarlanmış bir varsayılan sayfa boyutuyla][adjusted default page size] bağlanması gerekir, çünkü aksi takdirde GRUB Multiboot header'ını bulamaz. Başka bir örnek de kernel'e iletilen [önyükleme bilgisinin][boot information], temiz soyutlamalar sunmak yerine pek çok mimariye bağımlı yapı içermesidir.
- Hem GRUB hem de Multiboot standardı yalnızca yetersiz biçimde belgelenmiştir.
- Kernel dosyasından önyüklenebilir bir disk imajı oluşturmak için GRUB'un host sisteme kurulması gerekir. Bu, Windows veya Mac üzerinde geliştirmeyi zorlaştırır.

[adjusted default page size]: https://wiki.osdev.org/Multiboot#Multiboot_2
[boot information]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format

Bu dezavantajlar nedeniyle GRUB veya Multiboot standardını kullanmamaya karar verdik. Ancak [bootimage] aracımıza Multiboot desteği eklemeyi planlıyoruz; böylece kernel'inizi bir GRUB sisteminde de yüklemek mümkün olacak. Multiboot uyumlu bir kernel yazmakla ilgileniyorsanız, bu blog serisinin [birinci sürümüne][first edition] göz atın.

[first edition]: @/edition-1/_index.md

### UEFI

(Şu anda UEFI desteği sağlamıyoruz, ancak sağlamayı çok isteriz! Yardım etmek isterseniz lütfen [Github issue](https://github.com/phil-opp/blog_os/issues/349)'da bize haber verin.)

## Minimal Bir Kernel
Artık bir bilgisayarın kabaca nasıl önyüklendiğini bildiğimize göre, kendi minimal kernel'imizi oluşturmanın zamanı geldi. Hedefimiz, önyüklendiğinde ekrana bir “Hello World!” yazdıran bir disk imajı oluşturmak. Bunu, önceki yazının [bağımsız Rust ikili dosyasını][freestanding Rust binary] genişleterek yapıyoruz.

Hatırlayabileceğiniz gibi, bağımsız ikili dosyayı `cargo` aracılığıyla derledik; ancak işletim sistemine bağlı olarak farklı giriş noktası adlarına ve derleme bayraklarına ihtiyacımız oldu. Bunun nedeni, `cargo`'nun varsayılan olarak _host sistem_ için, yani üzerinde çalıştığınız sistem için derleme yapmasıdır. Bu, kernel'imiz için istediğimiz bir şey değildir; çünkü örneğin Windows'un üzerinde çalışan bir kernel pek anlamlı değildir. Bunun yerine, açıkça tanımlanmış bir _hedef sistem (target system)_ için derleme yapmak istiyoruz.

### Rust Nightly Kurulumu {#installing-rust-nightly}
Rust'ın üç yayın kanalı vardır: _stable_, _beta_ ve _nightly_. Rust Book bu kanallar arasındaki farkı gerçekten iyi açıklar, bu yüzden bir dakikanızı ayırıp [göz atın](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html#choo-choo-release-channels-and-riding-the-trains). Bir işletim sistemi oluşturmak için, yalnızca nightly kanalında bulunan bazı deneysel özelliklere ihtiyacımız olacak, bu yüzden Rust'ın bir nightly sürümünü kurmamız gerekiyor.

Rust kurulumlarını yönetmek için kesinlikle [rustup]'ı tavsiye ederim. Nightly, beta ve stable derleyicileri yan yana kurmanıza olanak tanır ve bunları güncellemeyi kolaylaştırır. rustup ile, `rustup override set nightly` komutunu çalıştırarak mevcut dizin için bir nightly derleyici kullanabilirsiniz. Alternatif olarak, projenin kök dizinine `nightly` içeriğine sahip `rust-toolchain` adında bir dosya ekleyebilirsiniz. Kurulu bir nightly sürümünüz olduğunu `rustc --version` komutunu çalıştırarak kontrol edebilirsiniz: Sürüm numarası sonunda `-nightly` içermelidir.

[rustup]: https://www.rustup.rs/

Nightly derleyici, dosyamızın başında _özellik bayrakları (feature flags)_ adı verilen şeyleri kullanarak çeşitli deneysel özellikleri etkinleştirmemize olanak tanır. Örneğin, `main.rs` dosyamızın başına `#![feature(asm)]` ekleyerek satır içi assembly için deneysel [`asm!` makrosunu][`asm!` macro] etkinleştirebilirdik. Bu tür deneysel özelliklerin tamamen kararsız olduğunu unutmayın; bu, gelecekteki Rust sürümlerinin bunları önceden uyarı olmaksızın değiştirebileceği veya kaldırabileceği anlamına gelir. Bu nedenle, onları yalnızca kesinlikle gerekli olduğunda kullanacağız.

[`asm!` macro]: https://doc.rust-lang.org/stable/reference/inline-assembly.html

### Hedef (Target) Belirtimi
Cargo, `--target` parametresi aracılığıyla farklı hedef sistemleri destekler. Hedef, CPU mimarisini, satıcıyı, işletim sistemini ve [ABI]'yi tanımlayan _[target triple]_ adı verilen bir şeyle açıklanır. Örneğin, `x86_64-unknown-linux-gnu` target triple'ı; bir `x86_64` CPU'ya, belirli bir satıcısı olmayan ve GNU ABI'li bir Linux işletim sistemine sahip bir sistemi tanımlar. Rust, Android için `arm-linux-androideabi` veya [WebAssembly için `wasm32-unknown-unknown`](https://www.hellorust.com/setup/wasm-target/) dahil olmak üzere [pek çok farklı target triple'ı][platform-support] destekler.

[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[ABI]: https://stackoverflow.com/a/2456882
[platform-support]: https://forge.rust-lang.org/release/platform-support.html
[custom-targets]: https://doc.rust-lang.org/nightly/rustc/targets/custom.html

Ancak hedef sistemimiz için bazı özel yapılandırma parametrelerine ihtiyacımız var (örneğin alttaki bir OS olmaması), bu yüzden [mevcut target triple'lardan][platform-support] hiçbiri uymuyor. Neyse ki Rust, bir JSON dosyası aracılığıyla [kendi hedefimizi][custom-targets] tanımlamamıza olanak tanır. Örneğin, `x86_64-unknown-linux-gnu` hedefini tanımlayan bir JSON dosyası şöyle görünür:

```json
{
    "llvm-target": "x86_64-unknown-linux-gnu",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": 64,
    "target-c-int-width": 32,
    "os": "linux",
    "executables": true,
    "linker-flavor": "gcc",
    "pre-link-args": ["-m64"],
    "morestack": false
}
```

Çoğu alan, LLVM tarafından o platform için kod üretmek üzere gereklidir. Örneğin, [`data-layout`] alanı çeşitli tamsayı, kayan nokta ve işaretçi (pointer) tiplerinin boyutunu tanımlar. Ardından, `target-pointer-width` gibi Rust'ın koşullu derleme için kullandığı alanlar vardır. Üçüncü tür alan ise crate'in nasıl derlenmesi gerektiğini tanımlar. Örneğin, `pre-link-args` alanı [linker]'a geçirilen argümanları belirtir.

[`data-layout`]: https://llvm.org/docs/LangRef.html#data-layout
[linker]: https://en.wikipedia.org/wiki/Linker_(computing)

Kernel'imizle biz de `x86_64` sistemlerini hedefliyoruz, bu yüzden hedef belirtimimiz yukarıdakine çok benzeyecek. Ortak içeriğe sahip bir `x86_64-blog_os.json` dosyası (istediğiniz herhangi bir ismi seçin) oluşturarak başlayalım:

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": 64,
    "target-c-int-width": 32,
    "os": "none",
    "executables": true
}
```

`llvm-target` ve `os` alanındaki OS'u `none` olarak değiştirdiğimize dikkat edin, çünkü bare metal üzerinde çalışacağız.

Aşağıdaki derlemeyle ilgili girdileri ekliyoruz:


```json
"linker-flavor": "ld.lld",
"linker": "rust-lld",
```

Platformun varsayılan linker'ını (Linux hedeflerini desteklemeyebilir) kullanmak yerine, kernel'imizi bağlamak için Rust ile birlikte gelen çapraz platform [LLD] linker'ını kullanıyoruz.

[LLD]: https://lld.llvm.org/

```json
"panic-strategy": "abort",
```

Bu ayar, hedefin panic anında [stack unwinding]'i desteklemediğini, bu yüzden programın bunun yerine doğrudan abort etmesi gerektiğini belirtir. Bunun, Cargo.toml dosyamızdaki `panic = "abort"` seçeneğiyle aynı etkisi vardır, bu yüzden onu oradan kaldırabiliriz. (Cargo.toml seçeneğinin aksine, bu hedef seçeneğinin, bu yazının ilerleyen kısmında `core` kütüphanesini yeniden derlediğimizde de geçerli olduğunu unutmayın. Yani Cargo.toml seçeneğini tutmayı tercih etseniz bile, bu seçeneği dahil ettiğinizden emin olun.)

[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php

```json
"disable-redzone": true,
```

Bir kernel yazıyoruz, bu yüzden bir noktada interrupt'ları işlememiz gerekecek. Bunu güvenli bir şekilde yapmak için, _“red zone”_ adı verilen belirli bir stack pointer optimizasyonunu devre dışı bırakmamız gerekir, çünkü aksi takdirde stack bozulmasına neden olur. Daha fazla bilgi için [red zone'u devre dışı bırakma][disabling the red zone] hakkındaki ayrı yazımıza bakın.

[disabling the red zone]: @/edition-2/posts/02-minimal-rust-kernel/disable-red-zone/index.tr.md

```json
"features": "-mmx,-sse,+soft-float",
```

`features` alanı, hedef özelliklerini etkinleştirir/devre dışı bırakır. `mmx` ve `sse` özelliklerini önlerine eksi koyarak devre dışı bırakıyor, `soft-float` özelliğini ise önüne artı koyarak etkinleştiriyoruz. Farklı bayraklar arasında boşluk olmaması gerektiğini unutmayın; aksi takdirde LLVM, özellik (features) dizesini yorumlayamaz.

`mmx` ve `sse` özellikleri, programları genellikle önemli ölçüde hızlandırabilen [Single Instruction Multiple Data (SIMD)] komutlarına yönelik desteği belirler. Ancak OS kernel'lerinde büyük SIMD register'larını kullanmak performans sorunlarına yol açar. Bunun nedeni, kernel'in kesintiye uğramış bir programa devam etmeden önce tüm register'ları orijinal durumlarına geri yüklemesi gerektiğidir. Bu, kernel'in her sistem çağrısında veya donanım interrupt'ında tüm SIMD durumunu ana belleğe kaydetmesi gerektiği anlamına gelir. SIMD durumu çok büyük olduğu için (512–1600 bayt) ve interrupt'lar çok sık meydana gelebileceği için, bu ek kaydetme/geri yükleme işlemleri performansa önemli ölçüde zarar verir. Bunu önlemek için kernel'imizde SIMD'yi devre dışı bırakıyoruz (üzerinde çalışan uygulamalar için değil!).

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

SIMD'yi devre dışı bırakmanın bir sorunu, `x86_64`'te kayan nokta işlemlerinin varsayılan olarak SIMD register'larını gerektirmesidir. Bu sorunu çözmek için, tüm kayan nokta işlemlerini normal tamsayılara dayalı yazılım fonksiyonları aracılığıyla öykünen `soft-float` özelliğini ekliyoruz.

Daha fazla bilgi için [SIMD'yi devre dışı bırakma](@/edition-2/posts/02-minimal-rust-kernel/disable-simd/index.tr.md) hakkındaki yazımıza bakın.

```json
"rustc-abi": "softfloat"
```

`soft-float` özelliğini kullanmak istediğimiz için, Rust derleyicisi `rustc`'ye de ilgili ABI'yi kullanmak istediğimizi söylememiz gerekir. Bunu, `rustc-abi` alanını `softfloat` olarak ayarlayarak yapabiliriz.

#### Hepsini Bir Araya Getirmek
Hedef belirtim dosyamız artık şöyle görünüyor:

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": 64,
    "target-c-int-width": 32,
    "os": "none",
    "executables": true,
    "linker-flavor": "ld.lld",
    "linker": "rust-lld",
    "panic-strategy": "abort",
    "disable-redzone": true,
    "features": "-mmx,-sse,+soft-float",
    "rustc-abi": "softfloat"
}
```

### Kernel'imizi Derlemek
Yeni hedefimiz için derleme yapmak Linux kurallarını kullanır, çünkü ld.lld linker-flavor'ı llvm'e `-flavor gnu` bayrağıyla derleme yapmasını söyler (daha fazla linker seçeneği için [rustc belgelerine](https://doc.rust-lang.org/rustc/codegen-options/index.html#linker-flavor) bakın). Bu, [önceki yazıda][previous post] açıklandığı gibi `_start` adında bir giriş noktasına ihtiyacımız olduğu anlamına gelir:

[previous post]: @/edition-2/posts/01-freestanding-rust-binary/index.tr.md

```rust
// src/main.rs

#![no_std] // Rust standart kütüphanesini bağlama
#![no_main] // tüm Rust seviyesindeki giriş noktalarını devre dışı bırak

use core::panic::PanicInfo;

/// Bu fonksiyon panic anında çağrılır.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)] // bu fonksiyonun adını parçalama (mangle etme)
pub extern "C" fn _start() -> ! {
    // bu fonksiyon giriş noktasıdır, çünkü linker varsayılan olarak
    // `_start` adında bir fonksiyon arar
    loop {}
}
```

Host işletim sisteminizden bağımsız olarak giriş noktasının `_start` olarak adlandırılması gerektiğine dikkat edin.

Artık JSON dosyasının adını `--target` olarak geçirerek kernel'i yeni hedefimiz için derleyebiliriz:

```
> cargo build --target x86_64-blog_os.json

error: `.json` target specs require -Zjson-target-spec
```

Başarısız oldu! Hata bize, özel JSON hedef belirtimlerinin açık bir tercih (opt-in) gerektiren kararsız bir özellik olduğunu söylüyor. Bunun nedeni, JSON hedef dosyalarının biçiminin henüz kararlı kabul edilmemesidir, bu yüzden gelecekteki Rust sürümlerinde değişiklikler olabilir. Daha fazla bilgi için [özel JSON hedef belirtimleri için takip issue'suna][json-target-spec-issue] bakın.

[json-target-spec-issue]: https://github.com/rust-lang/rust/issues/151528

#### `json-target-spec` Seçeneği

Özel JSON hedef belirtimlerine yönelik desteği etkinleştirmek için, `.cargo/config.toml` yolunda yerel bir [cargo yapılandırma][cargo configuration] dosyası oluşturmamız gerekir (`.cargo` klasörü `src` klasörünüzün yanında olmalıdır); içeriği şu şekilde olmalı:

[cargo configuration]: https://doc.rust-lang.org/cargo/reference/config.html

```toml
# .cargo/config.toml içinde

[unstable]
json-target-spec = true
```

Bu, kararsız `json-target-spec` özelliğini etkinleştirir ve özel JSON hedef dosyaları kullanmamıza olanak tanır.

Bu yapılandırma yerindeyken, tekrar derlemeyi deneyelim:

```
> cargo build --target x86_64-blog_os.json

error[E0463]: can't find crate for `core`
```

Hâlâ başarısız oluyor, ancak yeni bir hatayla. Hata bize, Rust derleyicisinin [`core` kütüphanesini][`core` library] bulamadığını söylüyor. Bu kütüphane, `Result`, `Option` ve iterator'lar gibi temel Rust tiplerini içerir ve tüm `no_std` crate'lerine örtük olarak bağlanır.

[`core` library]: https://doc.rust-lang.org/nightly/core/index.html

Sorun şu ki, core kütüphanesi Rust derleyicisiyle birlikte _önceden derlenmiş_ bir kütüphane olarak dağıtılır. Yani yalnızca desteklenen host triple'lar için (örneğin `x86_64-unknown-linux-gnu`) geçerlidir, ancak özel hedefimiz için geçerli değildir. Diğer hedefler için kod derlemek istiyorsak, önce `core`'u bu hedefler için yeniden derlememiz gerekir.

#### `build-std` Seçeneği

İşte cargo'nun [`build-std` özelliği][`build-std` feature] burada devreye giriyor. Bu özellik, Rust kurulumuyla gelen önceden derlenmiş sürümleri kullanmak yerine `core`'u ve diğer standart kütüphane crate'lerini talep üzerine yeniden derlemeye olanak tanır. Bu özellik çok yeni ve hâlâ tamamlanmamış durumda, bu yüzden "kararsız" olarak işaretlenmiştir ve yalnızca [nightly Rust derleyicilerinde][nightly Rust compilers] kullanılabilir.

[`build-std` feature]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std
[nightly Rust compilers]: #installing-rust-nightly

Bu özelliği kullanmak için, `.cargo/config.toml` yolundaki [cargo yapılandırma][cargo configuration] dosyamıza aşağıdakileri eklememiz gerekir:

```toml
# .cargo/config.toml içinde

[unstable]
json-target-spec = true
build-std = ["core", "compiler_builtins"]
```

Bu, cargo'ya `core` ve `compiler_builtins` kütüphanelerini yeniden derlemesi gerektiğini söyler. İkincisi gereklidir, çünkü `core`'un bir bağımlılığıdır. Bu kütüphaneleri yeniden derlemek için cargo'nun Rust kaynak koduna erişmesi gerekir; bunu `rustup component add rust-src` ile kurabiliriz.

<div class="note">

**Not:** `unstable.build-std` yapılandırma anahtarı, en az 2020-07-15 tarihli Rust nightly'sini gerektirir.

</div>

`unstable.build-std` yapılandırma anahtarını ayarladıktan ve `rust-src` bileşenini kurduktan sonra derleme komutumuzu yeniden çalıştırabiliriz:

```
> cargo build --target x86_64-blog_os.json
   Compiling core v0.0.0 (/…/rust/src/libcore)
   Compiling rustc-std-workspace-core v1.99.0 (/…/rust/src/tools/rustc-std-workspace-core)
   Compiling compiler_builtins v0.1.32
   Compiling blog_os v0.1.0 (/…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

`cargo build`'in artık özel hedefimiz için `core`, `rustc-std-workspace-core` (`compiler_builtins`'in bir bağımlılığı) ve `compiler_builtins` kütüphanelerini yeniden derlediğini görüyoruz.

#### Bellekle İlgili Intrinsic'ler

Rust derleyicisi, belirli bir yerleşik fonksiyon kümesinin tüm sistemler için kullanılabilir olduğunu varsayar. Bu fonksiyonların çoğu, az önce yeniden derlediğimiz `compiler_builtins` crate'i tarafından sağlanır. Ancak bu crate'te, normalde sistemdeki C kütüphanesi tarafından sağlandıkları için varsayılan olarak etkin olmayan, bellekle ilgili bazı fonksiyonlar vardır. Bu fonksiyonlar arasında, bir bellek bloğundaki tüm baytları belirli bir değere ayarlayan `memset`; bir bellek bloğunu başka birine kopyalayan `memcpy`; ve iki bellek bloğunu karşılaştıran `memcmp` bulunur. Şu anda kernel'imizi derlemek için bu fonksiyonların hiçbirine ihtiyacımız olmasa da, ona biraz daha kod ekler eklemez (örneğin struct'ları etrafta kopyalarken) gerekli olacaklar.

İşletim sisteminin C kütüphanesine bağlanamayacağımız için, bu fonksiyonları derleyiciye sağlamanın alternatif bir yoluna ihtiyacımız var. Bunun için olası bir yaklaşım, kendi `memset` vb. fonksiyonlarımızı uygulamak ve onlara `#[unsafe(no_mangle)]` özniteliğini uygulamak olabilir (derleme sırasındaki otomatik yeniden adlandırmayı önlemek için). Ancak bu tehlikelidir, çünkü bu fonksiyonların uygulamasındaki en ufak bir hata tanımsız davranışa yol açabilir. Örneğin, `memcpy`'yi bir `for` döngüsüyle uygulamak sonsuz bir özyinelemeyle (recursion) sonuçlanabilir, çünkü `for` döngüleri örtük olarak [`IntoIterator::into_iter`] trait metodunu çağırır ve bu da yeniden `memcpy`'yi çağırabilir. Bu yüzden, bunun yerine mevcut, iyi test edilmiş uygulamaları yeniden kullanmak iyi bir fikirdir.

[`IntoIterator::into_iter`]: https://doc.rust-lang.org/stable/core/iter/trait.IntoIterator.html#tymethod.into_iter

Neyse ki `compiler_builtins` crate'i, gereken tüm fonksiyonlar için uygulamaları zaten içeriyor; bunlar yalnızca C kütüphanesinden gelen uygulamalarla çakışmamak için varsayılan olarak devre dışı bırakılmış. Bunları, cargo'nun [`build-std-features`] bayrağını `["compiler-builtins-mem"]` olarak ayarlayarak etkinleştirebiliriz. `build-std` bayrağı gibi, bu bayrak da ya komut satırında bir `-Z` bayrağı olarak geçirilebilir ya da `.cargo/config.toml` dosyasındaki `unstable` tablosunda yapılandırılabilir. Her zaman bu bayrakla derleme yapmak istediğimiz için, yapılandırma dosyası seçeneği bizim için daha mantıklı:

[`build-std-features`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features

```toml
# .cargo/config.toml içinde

[unstable]
json-target-spec = true
build-std-features = ["compiler-builtins-mem"]
build-std = ["core", "compiler_builtins"]
```

(`compiler-builtins-mem` özelliğine yönelik destek yalnızca [çok yakın bir zamanda eklendi](https://github.com/rust-lang/rust/pull/77284), bu yüzden bunun için en az `2020-09-30` tarihli Rust nightly'sine ihtiyacınız var.)

Perde arkasında bu bayrak, `compiler_builtins` crate'inin [`mem` özelliğini][`mem` feature] etkinleştirir. Bunun etkisi, `#[unsafe(no_mangle)]` özniteliğinin crate'in [`memcpy` vb. uygulamalarına][`memcpy` etc. implementations] uygulanması ve böylece bunların linker tarafından kullanılabilir hale gelmesidir.

[`mem` feature]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/Cargo.toml#L54-L55
[`memcpy` etc. implementations]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/src/mem.rs#L12-L69

Bu değişiklikle kernel'imiz, derleyicinin gerektirdiği tüm fonksiyonlar için geçerli uygulamalara sahip oluyor; böylece kodumuz daha karmaşık hale gelse bile derlenmeye devam edecek.

#### Varsayılan Bir Hedef Belirlemek {#set-a-default-target}

Her `cargo build` çağrısında `--target` parametresini geçirmekten kaçınmak için, varsayılan hedefi geçersiz kılabiliriz. Bunu yapmak için, `.cargo/config.toml` yolundaki [cargo yapılandırma][cargo configuration] dosyamıza aşağıdakileri ekliyoruz:

```toml
# .cargo/config.toml içinde

[build]
target = "x86_64-blog_os.json"
```

Bu, `cargo`'ya açık bir `--target` argümanı geçirilmediğinde `x86_64-blog_os.json` hedefimizi kullanmasını söyler. Bu, artık kernel'imizi basit bir `cargo build` ile derleyebileceğimiz anlamına gelir. Cargo yapılandırma seçenekleri hakkında daha fazla bilgi için [resmi belgelere][cargo configuration] göz atın.

Artık kernel'imizi basit bir `cargo build` ile bir bare metal hedefi için derleyebiliyoruz. Ancak bootloader tarafından çağrılacak olan `_start` giriş noktamız hâlâ boş. Artık ondan ekrana bir şeyler yazdırmanın zamanı geldi.

### Ekrana Yazdırmak
Bu aşamada ekrana metin yazdırmanın en kolay yolu [VGA metin arabelleğidir (text buffer)][VGA text buffer]. Bu, VGA donanımına eşlenmiş ve ekranda görüntülenen içeriği barındıran özel bir bellek alanıdır. Normalde, her biri 80 karakter hücresi içeren 25 satırdan oluşur. Her karakter hücresi, bazı ön plan ve arka plan renkleriyle bir ASCII karakteri görüntüler. Ekran çıktısı şöyle görünür:

[VGA text buffer]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode

![yaygın ASCII karakterleri için ekran çıktısı](https://upload.wikimedia.org/wikipedia/commons/f/f8/Codepage-437.png)

VGA arabelleğinin tam yerleşimini, onun için ilk küçük sürücüyü yazacağımız bir sonraki yazıda tartışacağız. “Hello World!” yazdırmak için sadece arabelleğin `0xb8000` adresinde bulunduğunu ve her karakter hücresinin bir ASCII baytı ile bir renk baytından oluştuğunu bilmemiz yeterli.

Uygulama şöyle görünür:

```rust
static HELLO: &[u8] = b"Hello World!";

#[unsafe(no_mangle)]
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

İlk olarak, `0xb8000` tamsayısını bir [ham işaretçiye (raw pointer)][raw pointer] dönüştürüyoruz. Ardından [statik][static] `HELLO` [bayt dizesinin (byte string)][byte string] baytları üzerinde [iterasyon yapıyoruz][iterate]. Ek olarak çalışan bir `i` değişkeni elde etmek için [`enumerate`] metodunu kullanıyoruz. for döngüsünün gövdesinde, dize baytını ve karşılık gelen renk baytını (`0xb`, açık camgöbeği bir renktir) yazmak için [`offset`] metodunu kullanıyoruz.

[iterate]: https://doc.rust-lang.org/stable/book/ch13-02-iterators.html
[static]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate
[byte string]: https://doc.rust-lang.org/reference/tokens.html#byte-string-literals
[raw pointer]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

Tüm bellek yazma işlemlerinin etrafında bir [`unsafe`] bloğu olduğuna dikkat edin. Bunun nedeni, Rust derleyicisinin oluşturduğumuz ham işaretçilerin geçerli olduğunu kanıtlayamamasıdır. Bunlar herhangi bir yere işaret edebilir ve veri bozulmasına yol açabilir. Onları bir `unsafe` bloğunun içine koyarak, temelde derleyiciye işlemlerin geçerli olduğundan kesinlikle emin olduğumuzu söylüyoruz. Bir `unsafe` bloğunun Rust'ın güvenlik denetimlerini kapatmadığını unutmayın. Yalnızca [beş ek şey][five additional things] yapmanıza olanak tanır.

[`unsafe`]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html
[five additional things]: https://doc.rust-lang.org/stable/book/ch20-01-unsafe-rust.html#unsafe-superpowers

**Bunun, Rust'ta işleri yapmak istediğimiz yol olmadığını** vurgulamak istiyorum! unsafe blokların içinde ham işaretçilerle çalışırken işleri batırmak çok kolaydır. Örneğin, dikkatli olmazsak kolayca arabelleğin sonunun ötesine yazabiliriz.

Bu yüzden `unsafe` kullanımını mümkün olduğunca en aza indirmek istiyoruz. Rust bize, güvenli soyutlamalar oluşturarak bunu yapma yeteneği veriyor. Örneğin, tüm güvensizliği (unsafety) kapsülleyen ve dışarıdan yanlış bir şey yapmanın _imkânsız_ olmasını sağlayan bir VGA arabellek tipi oluşturabiliriz. Bu sayede, yalnızca minimal miktarda `unsafe` koda ihtiyaç duyar ve [bellek güvenliğini][memory safety] ihlal etmediğimizden emin olabiliriz. Böyle güvenli bir VGA arabellek soyutlamasını bir sonraki yazıda oluşturacağız.

[memory safety]: https://en.wikipedia.org/wiki/Memory_safety

## Kernel'imizi Çalıştırmak

Artık algılanabilir bir şey yapan bir çalıştırılabilir dosyamız olduğuna göre, onu çalıştırmanın zamanı geldi. İlk olarak, derlenmiş kernel'imizi bir bootloader ile bağlayarak önyüklenebilir bir disk imajına dönüştürmemiz gerekiyor. Ardından disk imajını [QEMU] sanal makinesinde çalıştırabilir veya bir USB bellek kullanarak gerçek donanımda önyükleyebiliriz.

### Bir Bootimage Oluşturmak {#creating-a-bootimage}

Derlenmiş kernel'imizi önyüklenebilir bir disk imajına dönüştürmek için, onu bir bootloader ile bağlamamız gerekir. [Önyükleme hakkındaki bölümde][section about booting] öğrendiğimiz gibi, bootloader CPU'yu başlatmaktan ve kernel'imizi yüklemekten sorumludur.

[section about booting]: #the-boot-process

Başlı başına bir proje olan kendi bootloader'ımızı yazmak yerine, [`bootloader`] crate'ini kullanıyoruz. Bu crate, herhangi bir C bağımlılığı olmadan, yalnızca Rust ve satır içi assembly ile temel bir BIOS bootloader'ı uygular. Onu kernel'imizi önyüklemek için kullanabilmek için, ona bir bağımlılık eklememiz gerekir:

[`bootloader`]: https://crates.io/crates/bootloader

```toml
# Cargo.toml içinde

[dependencies]
bootloader = "0.9"
```

**Not:** Bu yazı yalnızca `bootloader v0.9` ile uyumludur. Daha yeni sürümler farklı bir derleme sistemi kullanır ve bu yazıyı takip ederken derleme hatalarına neden olur.

Bootloader'ı bir bağımlılık olarak eklemek, gerçekte önyüklenebilir bir disk imajı oluşturmak için yeterli değildir. Sorun, kernel'imizi derlemeden sonra bootloader ile bağlamamız gerekmesi, ancak cargo'nun [derleme sonrası betiklerini (post-build scripts)][post-build scripts] desteklememesidir.

[post-build scripts]: https://github.com/rust-lang/cargo/issues/545

Bu sorunu çözmek için, önce kernel'i ve bootloader'ı derleyen, ardından önyüklenebilir bir disk imajı oluşturmak için onları birbirine bağlayan `bootimage` adlı bir araç oluşturduk. Aracı kurmak için ana dizininize (veya cargo projenizin dışındaki herhangi bir dizine) gidin ve terminalinizde aşağıdaki komutu çalıştırın:

```
cargo install bootimage
```

`bootimage`'ı çalıştırmak ve bootloader'ı derlemek için, `llvm-tools-preview` rustup bileşeninin kurulu olması gerekir. Bunu `rustup component add llvm-tools-preview` komutunu çalıştırarak yapabilirsiniz.

`bootimage`'ı kurduktan ve `llvm-tools-preview` bileşenini ekledikten sonra, cargo projenizin dizinine geri dönüp aşağıdakini çalıştırarak önyüklenebilir bir disk imajı oluşturabilirsiniz:

```
> cargo bootimage
```

Aracın, kernel'imizi `cargo build` kullanarak yeniden derlediğini görüyoruz; böylece yaptığınız tüm değişiklikleri otomatik olarak alacaktır. Ardından, biraz zaman alabilecek olan bootloader'ı derler. Tüm crate bağımlılıkları gibi, o da yalnızca bir kez derlenir ve sonra önbelleğe alınır, bu yüzden sonraki derlemeler çok daha hızlı olacaktır. Son olarak `bootimage`, bootloader ile kernel'inizi önyüklenebilir bir disk imajında birleştirir.

Komutu çalıştırdıktan sonra, `target/x86_64-blog_os/debug` dizininizde `bootimage-blog_os.bin` adlı önyüklenebilir bir disk imajı görmelisiniz. Onu bir sanal makinede önyükleyebilir veya gerçek donanımda önyüklemek için bir USB sürücüsüne kopyalayabilirsiniz. (Bunun, farklı bir biçime sahip bir CD imajı olmadığını, bu yüzden onu bir CD'ye yazdırmanın işe yaramadığını unutmayın.)

#### Nasıl çalışır?
`bootimage` aracı perde arkasında aşağıdaki adımları gerçekleştirir:

- Kernel'imizi bir [ELF] dosyasına derler.
- Bootloader bağımlılığını bağımsız bir çalıştırılabilir dosya olarak derler.
- Kernel ELF dosyasının baytlarını bootloader'a bağlar.

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
[rust-osdev/bootloader]: https://github.com/rust-osdev/bootloader

Önyüklendiğinde, bootloader eklenmiş ELF dosyasını okur ve ayrıştırır. Ardından program segmentlerini sayfa tablolarındaki (page tables) sanal adreslere eşler, `.bss` bölümünü sıfırlar ve bir stack kurar. Son olarak, giriş noktası adresini (`_start` fonksiyonumuz) okur ve ona atlar.

### QEMU'da Önyükleme

Artık disk imajını bir sanal makinede önyükleyebiliriz. Onu [QEMU]'da önyüklemek için aşağıdaki komutu çalıştırın:

[QEMU]: https://www.qemu.org/

```
> qemu-system-x86_64 -drive format=raw,file=target/x86_64-blog_os/debug/bootimage-blog_os.bin
```

Bu, şuna benzer görünmesi gereken ayrı bir pencere açar:

![QEMU'da görünen "Hello World!"](qemu.png)

"Hello World!" yazımızın ekranda görünür olduğunu görüyoruz.

### Gerçek Makine

Onu bir USB belleğe yazmak ve gerçek bir makinede önyüklemek de mümkündür, **ancak doğru cihaz adını seçmeye dikkat edin**, çünkü **o cihazdaki her şeyin üzerine yazılır**:

```
> dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

Burada `sdX`, USB belleğinizin cihaz adıdır.

İmajı USB belleğe yazdıktan sonra, ondan önyükleyerek gerçek donanımda çalıştırabilirsiniz. USB bellekten önyüklemek için muhtemelen özel bir önyükleme menüsü kullanmanız veya BIOS yapılandırmanızda önyükleme sırasını değiştirmeniz gerekecek. `bootloader` crate'i henüz UEFI desteğine sahip olmadığı için, bunun şu anda UEFI makineleri için çalışmadığını unutmayın.

### `cargo run` Kullanmak {#using-cargo-run}

Kernel'imizi QEMU'da çalıştırmayı kolaylaştırmak için, cargo'nun `runner` yapılandırma anahtarını ayarlayabiliriz:

```toml
# .cargo/config.toml içinde

[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```

`target.'cfg(target_os = "none")'` tablosu, hedef yapılandırma dosyasının `"os"` alanı `"none"` olarak ayarlanmış tüm hedeflere uygulanır. Buna `x86_64-blog_os.json` hedefimiz de dahildir. `runner` anahtarı, `cargo run` için çağrılması gereken komutu belirtir. Komut, başarılı bir derlemenin ardından, ilk argüman olarak çalıştırılabilir dosyanın yolu geçirilerek çalıştırılır. Daha fazla ayrıntı için [cargo belgelerine][cargo configuration] bakın.

`bootimage runner` komutu, özellikle bir `runner` çalıştırılabilir dosyası olarak kullanılabilecek şekilde tasarlanmıştır. Verilen çalıştırılabilir dosyayı projenin bootloader bağımlılığıyla bağlar ve ardından QEMU'yu başlatır. Daha fazla ayrıntı ve olası yapılandırma seçenekleri için [`bootimage`'ın Readme'sine][Readme of `bootimage`] bakın.

[Readme of `bootimage`]: https://github.com/rust-osdev/bootimage

Artık kernel'imizi derlemek ve QEMU'da önyüklemek için `cargo run` kullanabiliriz.

## Sırada ne var?

Bir sonraki yazıda, VGA metin arabelleğini daha ayrıntılı olarak inceleyeceğiz ve onun için güvenli bir arayüz yazacağız. Ayrıca `println` makrosu için destek ekleyeceğiz.
