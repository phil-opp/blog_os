+++
title = "Heap Ayırma"
weight = 10
path = "tr/heap-allocation"
date = 2019-06-26

[extra]
chapter = "Memory Management"

# Please update this when updating the translation
translation_based_on_commit = "211f460251cd332905225c93eb66b1aff9f4aefd"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

Bu yazı, kernel'imize heap ayırma desteği ekler. İlk olarak, dinamik belleğe bir giriş yapar ve borrow checker'ın yaygın ayırma hatalarını nasıl önlediğini gösterir. Ardından Rust'ın temel ayırma arayüzünü uygular, bir heap bellek bölgesi oluşturur ve bir allocator crate'i kurar. Bu yazının sonunda, yerleşik `alloc` crate'inin tüm ayırma ve koleksiyon tipleri kernel'imiz için kullanılabilir olacak.

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-10`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-10

<!-- toc -->

## Yerel ve Statik Değişkenler

Şu anda kernel'imizde iki tür değişken kullanıyoruz: yerel değişkenler ve `static` değişkenler. Yerel değişkenler [çağrı stack'inde (call stack)][call stack] saklanır ve yalnızca çevreleyen fonksiyon geri dönene kadar geçerlidir. Statik değişkenler sabit bir bellek konumunda saklanır ve her zaman programın tüm yaşam süresi boyunca var olur.

### Yerel Değişkenler

Yerel değişkenler, `push` ve `pop` işlemlerini destekleyen bir [stack veri yapısı][stack data structure] olan [çağrı stack'inde][call stack] saklanır. Her fonksiyon girişinde, çağrılan fonksiyonun parametreleri, dönüş adresi ve yerel değişkenleri derleyici tarafından push'lanır:

[call stack]: https://en.wikipedia.org/wiki/Call_stack
[stack data structure]: https://en.wikipedia.org/wiki/Stack_(abstract_data_type)

![Bir `outer()` ve bir `inner(i: usize)` fonksiyonu; `outer`, `inner(1)`'i çağırır. Her ikisinin de bazı yerel değişkenleri var. Çağrı stack'i şu yuvaları içerir: önce outer'ın yerel değişkenleri, ardından `i = 1` argümanı, ardından dönüş adresi, ardından inner'ın yerel değişkenleri.](call-stack.svg)

Yukarıdaki örnek, `outer` fonksiyonu `inner` fonksiyonunu çağırdıktan sonraki çağrı stack'ini gösterir. Çağrı stack'inin önce `outer`'ın yerel değişkenlerini içerdiğini görüyoruz. `inner` çağrısında, `1` parametresi ve fonksiyon için dönüş adresi push'landı. Ardından kontrol `inner`'a aktarıldı; o da kendi yerel değişkenlerini push'ladı.

`inner` fonksiyonu geri döndükten sonra, çağrı stack'inin onun kısmı tekrar pop'lanır ve yalnızca `outer`'ın yerel değişkenleri kalır:

![Yalnızca `outer`'ın yerel değişkenlerini içeren çağrı stack'i](call-stack-return.svg)

`inner`'ın yerel değişkenlerinin yalnızca fonksiyon geri dönene kadar var olduğunu görüyoruz. Rust derleyicisi bu yaşam sürelerini zorunlu kılar ve bir değeri çok uzun süre kullandığımızda bir hata fırlatır; örneğin bir yerel değişkene bir referans döndürmeye çalıştığımızda:

```rust
fn inner(i: usize) -> &'static u32 {
    let z = [1, 2, 3];
    &z[i]
}
```

([örneği playground'da çalıştır](https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=6186a0f3a54f468e1de8894996d12819))

Bu örnekte bir referans döndürmek anlamsız olsa da, bir değişkenin fonksiyondan daha uzun süre var olmasını istediğimiz durumlar vardır. Kernel'imizde böyle bir durumu, [bir interrupt descriptor table yüklemeye][load an interrupt descriptor table] çalıştığımızda ve yaşam süresini uzatmak için bir `static` değişken kullanmak zorunda kaldığımızda zaten gördük.

[load an interrupt descriptor table]: @/edition-2/posts/05-cpu-exceptions/index.tr.md#loading-the-idt

### Statik Değişkenler

Statik değişkenler, stack'ten ayrı, sabit bir bellek konumunda saklanır. Bu bellek konumu, derleme zamanında linker tarafından atanır ve çalıştırılabilir dosyada kodlanır. Statics, programın tüm çalışma süresi boyunca var olur, bu yüzden `'static` yaşam süresine sahiptirler ve her zaman yerel değişkenlerden referans alınabilirler:

![Aynı outer/inner örneği; tek fark, inner'ın bir `static Z: [u32; 3] = [1,2,3];` içermesi ve bir `&Z[i]` referansı döndürmesi](call-stack-static.svg)

Yukarıdaki örnekte `inner` fonksiyonu geri döndüğünde, çağrı stack'inin onun kısmı yok edilir. Statik değişkenler asla yok edilmeyen ayrı bir bellek aralığında var olur, bu yüzden `&Z[1]` referansı dönüşten sonra hâlâ geçerlidir.

`'static` yaşam süresinin yanı sıra, statik değişkenlerin ayrıca konumlarının derleme zamanında bilinmesi gibi yararlı bir özelliği vardır; böylece onlara erişmek için bir referans gerekmez. Bu özellikten `println` makromuz için yararlandık: Dahili olarak [statik bir `Writer`][static `Writer`] kullanarak, makroyu çağırmak için bir `&mut Writer` referansı gerekmez; bu da ek değişkenlere erişimimiz olmayan [exception handler'larda][exception handlers] çok yararlıdır.

[static `Writer`]: @/edition-2/posts/03-vga-text-buffer/index.tr.md#a-global-interface
[exception handlers]: @/edition-2/posts/05-cpu-exceptions/index.tr.md#implementation

Ancak statik değişkenlerin bu özelliği önemli bir dezavantaj getirir: varsayılan olarak salt okunurdurlar. Rust bunu zorunlu kılar, çünkü örneğin iki thread bir statik değişkeni aynı anda değiştirseydi bir [veri yarışı (data race)][data race] meydana gelirdi. Bir statik değişkeni değiştirmenin tek yolu, onu bir [`Mutex`] tipinde kapsüllemektir; bu da herhangi bir zamanda yalnızca tek bir `&mut` referansının var olmasını sağlar. [Statik VGA arabelleği `Writer`'ımız][vga mutex] için zaten bir `Mutex` kullandık.

[data race]: https://doc.rust-lang.org/nomicon/races.html
[`Mutex`]: https://docs.rs/spin/0.5.2/spin/struct.Mutex.html
[vga mutex]: @/edition-2/posts/03-vga-text-buffer/index.tr.md#spinlocks

## Dinamik Bellek {#dynamic-memory}

Yerel ve statik değişkenler birlikte zaten çok güçlüdür ve çoğu kullanım senaryosunu mümkün kılar. Ancak, her ikisinin de sınırlamaları olduğunu gördük:

- Yerel değişkenler yalnızca çevreleyen fonksiyonun veya bloğun sonuna kadar var olur. Bunun nedeni, çağrı stack'inde var olmaları ve çevreleyen fonksiyon geri döndükten sonra yok edilmeleridir.
- Statik değişkenler her zaman programın tüm çalışma süresi boyunca var olur, bu yüzden artık ihtiyaç duyulmadıklarında belleklerini geri alıp yeniden kullanmanın bir yolu yoktur. Ayrıca, belirsiz sahiplik semantiğine sahiptirler ve tüm fonksiyonlardan erişilebilirler, bu yüzden onları değiştirmek istediğimizde bir [`Mutex`] ile korunmaları gerekir.

Yerel ve statik değişkenlerin bir başka sınırlaması da sabit bir boyuta sahip olmalarıdır. Bu yüzden, daha fazla eleman eklendiğinde dinamik olarak büyüyen bir koleksiyon saklayamazlar. (Rust'ta dinamik boyutlu yerel değişkenlere izin verecek [unsized rvalue'ler][unsized rvalues] için öneriler vardır, ancak bunlar yalnızca bazı belirli durumlarda çalışır.)

[unsized rvalues]: https://github.com/rust-lang/rust/issues/48055

Bu dezavantajları aşmak için, programlama dilleri değişkenleri saklamak amacıyla genellikle **heap** adı verilen üçüncü bir bellek bölgesini destekler. Heap, çalışma zamanında `allocate` ve `deallocate` adı verilen iki fonksiyon aracılığıyla _dinamik bellek ayırmayı (dynamic memory allocation)_ destekler. Şu şekilde çalışır: `allocate` fonksiyonu, belirtilen boyutta, bir değişkeni saklamak için kullanılabilecek boş bir bellek parçası döndürür. Bu değişken daha sonra, değişkene bir referansla `deallocate` fonksiyonu çağrılarak serbest bırakılana kadar var olur.

Bir örnek üzerinden gidelim:

![inner fonksiyonu `allocate(size_of([u32; 3]))` çağırır, `z.write([1,2,3]);` yazar ve `(z as *mut u32).offset(i)` döndürür. Döndürülen değer `y` üzerinde, outer fonksiyonu `deallocate(y, size_of(u32))` gerçekleştirir.](call-stack-heap.svg)

Burada `inner` fonksiyonu, `z`'yi saklamak için statik değişkenler yerine heap belleği kullanır. Önce gereken boyutta bir bellek bloğu ayırır; bu da bir `*mut u32` [ham işaretçisi (raw pointer)][raw pointer] döndürür. Ardından `[1,2,3]` dizisini ona yazmak için [`ptr::write`] metodunu kullanır. Son adımda, `i`. elemana bir işaretçi hesaplamak için [`offset`] fonksiyonunu kullanır ve sonra onu döndürür. (Bu örnek fonksiyonda kısalık adına bazı gereken dönüşümleri ve unsafe blokları atladığımızı unutmayın.)

[raw pointer]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`ptr::write`]: https://doc.rust-lang.org/core/ptr/fn.write.html
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

Ayrılan bellek, `deallocate` çağrısıyla açıkça serbest bırakılana kadar var olur. Böylece, döndürülen işaretçi `inner` geri döndükten ve çağrı stack'inin onun kısmı yok edildikten sonra bile hâlâ geçerlidir. Heap belleği kullanmanın statik belleğe kıyasla avantajı, belleğin serbest bırakıldıktan sonra yeniden kullanılabilmesidir; bunu `outer`'daki `deallocate` çağrısıyla yaparız. O çağrıdan sonra durum şöyle görünür:

![Çağrı stack'i `outer`'ın yerel değişkenlerini içerir, heap `z[0]` ve `z[2]`'yi içerir, ancak artık `z[1]`'i içermez.](call-stack-heap-freed.svg)

`z[1]` yuvasının tekrar boş olduğunu ve bir sonraki `allocate` çağrısı için yeniden kullanılabileceğini görüyoruz. Ancak, `z[0]` ve `z[2]`'nin asla serbest bırakılmadığını da görüyoruz, çünkü onları hiç deallocate etmiyoruz. Böyle bir hataya _bellek sızıntısı (memory leak)_ denir ve genellikle programların aşırı bellek tüketiminin nedenidir (sadece `inner`'ı bir döngüde tekrar tekrar çağırdığımızda ne olacağını hayal edin). Bu kötü görünebilir, ancak dinamik ayırmayla meydana gelebilecek çok daha tehlikeli hata türleri vardır.

### Yaygın Hatalar

Talihsiz olan ancak programı saldırganlara karşı savunmasız kılmayan bellek sızıntılarının yanı sıra, daha ciddi sonuçları olan iki yaygın hata türü vardır:

- Bir değişken üzerinde `deallocate` çağırdıktan sonra yanlışlıkla onu kullanmaya devam ettiğimizde, **use-after-free** (serbest bırakma sonrası kullanım) adı verilen bir güvenlik açığımız olur. Böyle bir hata tanımsız davranışa neden olur ve genellikle saldırganlar tarafından keyfi kod çalıştırmak için sömürülebilir.
- Bir değişkeni yanlışlıkla iki kez serbest bıraktığımızda, bir **double-free** (çifte serbest bırakma) güvenlik açığımız olur. Bu sorunludur, çünkü ilk `deallocate` çağrısından sonra aynı yere ayrılmış farklı bir ayırmayı serbest bırakabilir. Böylece, yine bir use-after-free güvenlik açığına yol açabilir.

Bu tür güvenlik açıkları yaygın olarak bilinir, bu yüzden insanların artık onlardan nasıl kaçınılacağını öğrenmiş olmaları beklenebilir. Ama hayır, bu tür güvenlik açıkları hâlâ düzenli olarak bulunuyor; örneğin keyfi kod çalıştırmaya izin veren bu [Linux'taki use-after-free güvenlik açığı][linux vulnerability] (2019). `use-after-free linux {bu yıl}` gibi bir web araması muhtemelen her zaman sonuç verecektir. Bu, en iyi programcıların bile karmaşık projelerde dinamik belleği her zaman doğru şekilde ele alamadığını gösterir.

[linux vulnerability]: https://securityboulevard.com/2019/02/linux-use-after-free-vulnerability-found-in-linux-2-6-through-4-20-11/

Bu sorunlardan kaçınmak için, Java veya Python gibi pek çok dil, [_çöp toplama (garbage collection)_] adı verilen bir teknik kullanarak dinamik belleği otomatik olarak yönetir. Fikir, programcının `deallocate`'i asla elle çağırmamasıdır. Bunun yerine, program düzenli olarak duraklatılır ve kullanılmayan heap değişkenleri için taranır; bunlar daha sonra otomatik olarak deallocate edilir. Böylece, yukarıdaki güvenlik açıkları asla meydana gelemez. Dezavantajları, düzenli taramanın performans yükü ve muhtemelen uzun duraklama süreleridir.

[_çöp toplama (garbage collection)_]: https://en.wikipedia.org/wiki/Garbage_collection_(computer_science)

Rust soruna farklı bir yaklaşım benimser: Dinamik bellek işlemlerinin doğruluğunu derleme zamanında kontrol edebilen [_sahiplik (ownership)_] adı verilen bir kavram kullanır. Böylece, bahsedilen güvenlik açıklarından kaçınmak için çöp toplamaya gerek yoktur; bu da hiçbir performans yükü olmadığı anlamına gelir. Bu yaklaşımın bir başka avantajı, programcının tıpkı C veya C++'ta olduğu gibi dinamik bellek kullanımı üzerinde hâlâ ince taneli kontrole sahip olmasıdır.

[_sahiplik (ownership)_]: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html

### Rust'ta Ayırmalar (Allocations)

Programcının `allocate` ve `deallocate`'i elle çağırmasına izin vermek yerine, Rust standart kütüphanesi bu fonksiyonları örtük olarak çağıran soyutlama tipleri sağlar. En önemli tip, heap'te ayrılmış bir değer için bir soyutlama olan [**`Box`**]'tır. Bir değer alan, değerin boyutuyla `allocate`'i çağıran ve ardından değeri heap'teki yeni ayrılan yuvaya taşıyan bir [`Box::new`] yapıcı (constructor) fonksiyonu sağlar. Heap belleğini tekrar serbest bırakmak için, `Box` tipi kapsam dışına çıktığında `deallocate`'i çağırmak üzere [`Drop` trait'ini][`Drop` trait] uygular:

[**`Box`**]: https://doc.rust-lang.org/std/boxed/index.html
[`Box::new`]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html#method.new
[`Drop` trait]: https://doc.rust-lang.org/book/ch15-03-drop.html

```rust
{
    let z = Box::new([1,2,3]);
    […]
} // z kapsam dışına çıkar ve `deallocate` çağrılır
```

Bu örüntünün [_kaynak edinme başlatmadır (resource acquisition is initialization)_] (ya da kısaca _RAII_) gibi tuhaf bir adı vardır. C++'ta ortaya çıkmıştır; orada [`std::unique_ptr`] adı verilen benzer bir soyutlama tipini uygulamak için kullanılır.

[_kaynak edinme başlatmadır (resource acquisition is initialization)_]: https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization
[`std::unique_ptr`]: https://en.cppreference.com/w/cpp/memory/unique_ptr

Böyle bir tip tek başına tüm use-after-free hatalarını önlemeye yetmez, çünkü programcılar `Box` kapsam dışına çıktıktan ve karşılık gelen heap bellek yuvası deallocate edildikten sonra hâlâ referanslara tutunabilir:

```rust
let x = {
    let z = Box::new([1,2,3]);
    &z[1]
}; // z kapsam dışına çıkar ve `deallocate` çağrılır
println!("{}", x);
```

İşte Rust'ın sahipliği burada devreye girer. Her referansa, referansın geçerli olduğu kapsam olan soyut bir [yaşam süresi (lifetime)][lifetime] atar. Yukarıdaki örnekte, `x` referansı `z` dizisinden alınır, bu yüzden `z` kapsam dışına çıktıktan sonra geçersiz hale gelir. [Yukarıdaki örneği playground'da çalıştırdığınızda][playground-2], Rust derleyicisinin gerçekten bir hata fırlattığını görürsünüz:

[lifetime]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
[playground-2]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=28180d8de7b62c6b4a681a7b1f745a48

```
error[E0597]: `z[_]` does not live long enough
 --> src/main.rs:4:9
  |
2 |     let x = {
  |         - borrow later stored here
3 |         let z = Box::new([1,2,3]);
  |             - binding `z` declared here
4 |         &z[1]
  |         ^^^^^ borrowed value does not live long enough
5 |     }; // z goes out of scope and `deallocate` is called
  |     - `z[_]` dropped here while still borrowed
```

Terminoloji ilk başta biraz kafa karıştırıcı olabilir. Bir değere referans almaya, o değeri _ödünç almak (borrowing)_ denir; çünkü gerçek hayattaki bir ödünç almaya benzer: Bir nesneye geçici erişiminiz vardır ancak onu bir ara geri vermeniz gerekir ve onu yok etmemelisiniz. Bir nesne yok edilmeden önce tüm ödünç almaların sona erdiğini kontrol ederek, Rust derleyicisi hiçbir use-after-free durumunun meydana gelemeyeceğini garanti edebilir.

Rust'ın sahiplik sistemi daha da ileri gider; yalnızca use-after-free hatalarını önlemekle kalmaz, aynı zamanda Java veya Python gibi çöp toplamalı dillerin yaptığı gibi tam [_bellek güvenliği (memory safety)_] de sağlar. Buna ek olarak, [_thread güvenliği (thread safety)_] garanti eder ve böylece çok thread'li kodda o dillerden bile daha güvenlidir. Ve en önemlisi, tüm bu kontroller derleme zamanında gerçekleşir, bu yüzden C'deki elle yazılmış bellek yönetimine kıyasla hiçbir çalışma zamanı yükü yoktur.

[_bellek güvenliği (memory safety)_]: https://en.wikipedia.org/wiki/Memory_safety
[_thread güvenliği (thread safety)_]: https://en.wikipedia.org/wiki/Thread_safety

### Kullanım Senaryoları

Artık Rust'ta dinamik bellek ayırmanın temellerini biliyoruz, ama onu ne zaman kullanmalıyız? Kernel'imizle dinamik bellek ayırma olmadan gerçekten çok yol kat ettik, peki şimdi neden ona ihtiyacımız var?

İlk olarak, dinamik bellek ayırma her zaman biraz performans yüküyle gelir, çünkü her ayırma için heap'te boş bir yuva bulmamız gerekir. Bu nedenle, özellikle performansa duyarlı kernel kodunda yerel değişkenler genellikle tercih edilir. Ancak, dinamik bellek ayırmanın en iyi seçim olduğu durumlar vardır.

Temel bir kural olarak, dinamik bir yaşam süresine veya değişken bir boyuta sahip değişkenler için dinamik bellek gereklidir. Dinamik yaşam süresine sahip en önemli tip, sarmaladığı değere yapılan referansları sayan ve tüm referanslar kapsam dışına çıktıktan sonra onu deallocate eden [**`Rc`**]'dir. Değişken boyuta sahip tiplere örnekler [**`Vec`**], [**`String`**] ve daha fazla eleman eklendiğinde dinamik olarak büyüyen diğer [koleksiyon tipleridir][collection types]. Bu tipler, dolduklarında daha büyük miktarda bellek ayırarak, tüm elemanları kopyalayarak ve ardından eski ayırmayı deallocate ederek çalışır.

[**`Rc`**]: https://doc.rust-lang.org/alloc/rc/index.html
[**`Vec`**]: https://doc.rust-lang.org/alloc/vec/index.html
[**`String`**]: https://doc.rust-lang.org/alloc/string/index.html
[collection types]: https://doc.rust-lang.org/alloc/collections/index.html

Kernel'imiz için çoğunlukla koleksiyon tiplerine ihtiyaç duyacağız; örneğin gelecekteki yazılarda çoklu görev (multitasking) uygularken aktif görevlerin bir listesini saklamak için.

## Allocator Arayüzü {#the-allocator-interface}

Bir heap allocator'ı uygulamanın ilk adımı, yerleşik [`alloc`] crate'ine bir bağımlılık eklemektir. [`core`] crate'i gibi, o da standart kütüphanenin bir alt kümesidir ve buna ek olarak ayırma ve koleksiyon tiplerini içerir. `alloc`'a bağımlılık eklemek için, `lib.rs`'imize aşağıdakini ekliyoruz:

[`alloc`]: https://doc.rust-lang.org/alloc/
[`core`]: https://doc.rust-lang.org/core/

```rust
// src/lib.rs içinde

extern crate alloc;
```

Normal bağımlılıkların aksine, `Cargo.toml`'u değiştirmemize gerek yok. Bunun nedeni, `alloc` crate'inin standart kütüphanenin bir parçası olarak Rust derleyicisiyle birlikte gelmesidir, bu yüzden derleyici crate'i zaten bilir. Bu `extern crate` ifadesini ekleyerek, derleyicinin onu dahil etmeye çalışması gerektiğini belirtiriz. (Tarihsel olarak, tüm bağımlılıkların bir `extern crate` ifadesine ihtiyacı vardı; bu artık isteğe bağlıdır.)

Özel bir hedef için derleme yaptığımız için, Rust kurulumuyla gelen `alloc`'un önceden derlenmiş sürümünü kullanamayız. Bunun yerine, cargo'ya crate'i kaynaktan yeniden derlemesini söylememiz gerekir. Bunu, `.cargo/config.toml` dosyamızdaki `unstable.build-std` dizisine onu ekleyerek yapabiliriz:

```toml
# .cargo/config.toml içinde

[unstable]
build-std = ["core", "compiler_builtins", "alloc"]
```

Artık derleyici `alloc` crate'ini yeniden derleyip kernel'imize dahil edecek.

`alloc` crate'inin `#[no_std]` crate'lerinde varsayılan olarak devre dışı olmasının nedeni, ek gereksinimlere sahip olmasıdır. Projemizi şimdi derlemeye çalıştığımızda, bu gereksinimleri hatalar olarak göreceğiz:

```
error: no global memory allocator found but one is required; link to std or add
       #[global_allocator] to a static item that implements the GlobalAlloc trait.
```

Hata, `alloc` crate'inin `allocate` ve `deallocate` fonksiyonlarını sağlayan bir nesne olan bir heap allocator gerektirmesi nedeniyle oluşur. Rust'ta, heap allocator'lar hata mesajında bahsedilen [`GlobalAlloc`] trait'i ile tanımlanır. Crate için heap allocator'ı belirlemek üzere, `#[global_allocator]` özniteliği `GlobalAlloc` trait'ini uygulayan bir `static` değişkene uygulanmalıdır.

[`GlobalAlloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html

### `GlobalAlloc` Trait'i

[`GlobalAlloc`] trait'i, bir heap allocator'ın sağlaması gereken fonksiyonları tanımlar. Trait özeldir, çünkü neredeyse hiçbir zaman programcı tarafından doğrudan kullanılmaz. Bunun yerine, `alloc`'un ayırma ve koleksiyon tiplerini kullanırken derleyici trait metotlarına uygun çağrıları otomatik olarak ekler.

Trait'i tüm allocator tiplerimiz için uygulamamız gerekeceğinden, bildirimine daha yakından bakmaya değer:

```rust
pub unsafe trait GlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 { ... }
    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize
    ) -> *mut u8 { ... }
}
```

Örneklerimizde kullandığımız `allocate` ve `deallocate` fonksiyonlarına karşılık gelen, gerekli iki metot olan [`alloc`] ve [`dealloc`]'u tanımlar:
- [`alloc`] metodu, argüman olarak ayrılan belleğin sahip olması gereken istenen boyutu ve hizalamayı (alignment) açıklayan bir [`Layout`] örneği alır. Ayrılan bellek bloğunun ilk baytına bir [ham işaretçi][raw pointer] döndürür. Açık bir hata değeri yerine, `alloc` metodu bir ayırma hatasını bildirmek için null bir işaretçi döndürür. Bu biraz deyimsel olmayan (non-idiomatic) bir durumdur, ancak mevcut sistem allocator'larını sarmalamayı kolaylaştırma avantajı vardır; çünkü onlar da aynı kuralı kullanır.
- [`dealloc`] metodu karşılığıdır ve bir bellek bloğunu tekrar serbest bırakmaktan sorumludur. İki argüman alır: `alloc` tarafından döndürülen işaretçi ve ayırma için kullanılan `Layout`.

[`alloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.alloc
[`dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc
[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html

Trait buna ek olarak, varsayılan uygulamalara sahip [`alloc_zeroed`] ve [`realloc`] olmak üzere iki metot daha tanımlar:

- [`alloc_zeroed`] metodu, `alloc`'u çağırmaya ve ardından ayrılan bellek bloğunu sıfıra ayarlamaya eşdeğerdir; ki sağlanan varsayılan uygulamanın tam olarak yaptığı şey budur. Bir allocator uygulaması, mümkünse varsayılan uygulamaları daha verimli özel bir uygulamayla geçersiz kılabilir.
- [`realloc`] metodu, bir ayırmayı büyütmeye veya küçültmeye olanak tanır. Varsayılan uygulama, istenen boyutta yeni bir bellek bloğu ayırır ve önceki ayırmadan tüm içeriği kopyalar. Yine, bir allocator uygulaması muhtemelen bu metodun daha verimli bir uygulamasını sağlayabilir; örneğin mümkünse ayırmayı yerinde büyütüp/küçülterek.

[`alloc_zeroed`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#method.alloc_zeroed
[`realloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#method.realloc

#### Güvensizlik (Unsafety)

Dikkat edilmesi gereken bir nokta, hem trait'in kendisinin hem de tüm trait metotlarının `unsafe` olarak bildirilmesidir:

- Trait'i `unsafe` olarak bildirmenin nedeni, programcının bir allocator tipi için trait uygulamasının doğru olduğunu garanti etmesi gerekmesidir. Örneğin, `alloc` metodu asla başka bir yerde zaten kullanılan bir bellek bloğu döndürmemelidir; çünkü bu tanımsız davranışa neden olurdu.
- Benzer şekilde, metotların `unsafe` olmasının nedeni, çağıranın metotları çağırırken çeşitli değişmezleri (invariant) sağlaması gerekmesidir; örneğin `alloc`'a geçirilen `Layout`'un sıfır olmayan bir boyut belirtmesi gibi. Metotlar normalde doğrudan derleyici tarafından çağrıldığından ve derleyici gereksinimlerin karşılandığından emin olduğundan, bu pratikte pek alakalı değildir.

### Bir `DummyAllocator`

Artık bir allocator tipinin neyi sağlaması gerektiğini bildiğimize göre, basit bir sahte (dummy) allocator oluşturabiliriz. Bunun için yeni bir `allocator` modülü oluşturuyoruz:

```rust
// src/lib.rs içinde

pub mod allocator;
```

Sahte allocator'ımız, trait'i uygulamak için kesinlikle minimum olanı yapar ve `alloc` çağrıldığında her zaman bir hata döndürür. Şöyle görünür:

```rust
// src/allocator.rs içinde

use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should be never called")
    }
}
```

Struct'ın herhangi bir alana ihtiyacı yok, bu yüzden onu bir [sıfır boyutlu tip (zero-sized type)][zero-sized type] olarak oluşturuyoruz. Yukarıda belirtildiği gibi, `alloc`'tan her zaman bir ayırma hatasına karşılık gelen null işaretçiyi döndürüyoruz. Allocator asla bellek döndürmediğinden, `dealloc`'a bir çağrı asla meydana gelmemelidir. Bu nedenle, `dealloc` metodunda yalnızca panic yapıyoruz. `alloc_zeroed` ve `realloc` metotlarının varsayılan uygulamaları var, bu yüzden onlar için uygulama sağlamamıza gerek yok.

[zero-sized type]: https://doc.rust-lang.org/nomicon/exotic-sizes.html#zero-sized-types-zsts

Artık basit bir allocator'ımız var, ancak hâlâ Rust derleyicisine bu allocator'ı kullanması gerektiğini söylememiz gerekiyor. İşte `#[global_allocator]` özniteliği burada devreye girer.

### `#[global_allocator]` Özniteliği {#the-global-allocator-attribute}

`#[global_allocator]` özniteliği, Rust derleyicisine global heap allocator olarak hangi allocator örneğini kullanması gerektiğini söyler. Öznitelik yalnızca `GlobalAlloc` trait'ini uygulayan bir `static`'e uygulanabilir. `Dummy` allocator'ımızın bir örneğini global allocator olarak kaydedelim:

```rust
// src/allocator.rs içinde

#[global_allocator]
static ALLOCATOR: Dummy = Dummy;
```

`Dummy` allocator bir [sıfır boyutlu tip][zero-sized type] olduğundan, başlatma ifadesinde herhangi bir alan belirtmemize gerek yok.

Bu static ile, derleme hataları düzeltilmiş olmalı. Artık `alloc`'un ayırma ve koleksiyon tiplerini kullanabiliriz. Örneğin, heap'te bir değer ayırmak için bir [`Box`] kullanabiliriz:

[`Box`]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html

```rust
// src/main.rs içinde

extern crate alloc;

use alloc::boxed::Box;

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] "Hello World!" yazdır, `init` çağır, `mapper` ve `frame_allocator` oluştur

    let x = Box::new(41);

    // […] test modunda `test_main` çağır

    println!("It did not crash!");
    blog_os::hlt_loop();
}

```

`extern crate alloc` ifadesini `main.rs`'imizde de belirtmemiz gerektiğine dikkat edin. Bu gereklidir, çünkü `lib.rs` ve `main.rs` kısımları ayrı crate'ler olarak ele alınır. Ancak, başka bir `#[global_allocator]` static'i oluşturmamıza gerek yok, çünkü global allocator projedeki tüm crate'lere uygulanır. Aslında, başka bir crate'te ek bir allocator belirtmek bir hata olurdu.

Yukarıdaki kodu çalıştırdığımızda, bir panic'in meydana geldiğini görüyoruz:

![QEMU "panicked at `allocation error: Layout { size_: 4, align_: 4 }, src/lib.rs:89:5`" yazdırıyor](qemu-dummy-output.png)

Panic, `Box::new` fonksiyonunun global allocator'ın `alloc` fonksiyonunu örtük olarak çağırması nedeniyle meydana gelir. Sahte allocator'ımız her zaman null bir işaretçi döndürür, bu yüzden her ayırma başarısız olur. Bunu düzeltmek için, gerçekten kullanılabilir bellek döndüren bir allocator oluşturmamız gerekir.

## Bir Kernel Heap'i Oluşturmak {#creating-a-kernel-heap}

Düzgün bir allocator oluşturmadan önce, ilk olarak allocator'ın bellek ayırabileceği bir heap bellek bölgesi oluşturmamız gerekir. Bunu yapmak için, heap bölgesi için bir sanal bellek aralığı tanımlamamız ve ardından bu bölgeyi fiziksel frame'lere eşlememiz gerekir. Sanal bellek ve sayfa tablolarına genel bir bakış için [_"Paging'e Giriş"_][_"Introduction To Paging"_] yazısına bakın.

[_"Introduction To Paging"_]: @/edition-2/posts/08-paging-introduction/index.tr.md

İlk adım, heap için bir sanal bellek bölgesi tanımlamaktır. Henüz farklı bir bellek bölgesi için kullanılmadığı sürece, sevdiğimiz herhangi bir sanal adres aralığını seçebiliriz. Bir heap işaretçisini daha sonra kolayca tanıyabilmemiz için, onu `0x_4444_4444_0000` adresinden başlayan bellek olarak tanımlayalım:

```rust
// src/allocator.rs içinde

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB
```

Heap boyutunu şimdilik 100&nbsp;KiB olarak ayarlıyoruz. Gelecekte daha fazla alana ihtiyacımız olursa, onu basitçe artırabiliriz.

Bu heap bölgesini şimdi kullanmaya çalışsaydık, sanal bellek bölgesi henüz fiziksel belleğe eşlenmediği için bir page fault meydana gelirdi. Bunu çözmek için, [_"Paging Uygulaması"_] yazısında tanıttığımız [`Mapper` API'sini][`Mapper` API] kullanarak heap sayfalarını eşleyen bir `init_heap` fonksiyonu oluşturuyoruz:

[`Mapper` API]: @/edition-2/posts/09-paging-implementation/index.tr.md#using-offsetpagetable
[_"Paging Uygulaması"_]: @/edition-2/posts/09-paging-implementation/index.tr.md

```rust
// src/allocator.rs içinde

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        };
    }

    Ok(())
}
```

Fonksiyon, generic parametre olarak [`Size4KiB`] kullanılarak her ikisi de 4&nbsp;KiB sayfalarla sınırlanmış bir [`Mapper`] ve bir [`FrameAllocator`] örneğine değiştirilebilir referanslar alır. Fonksiyonun dönüş değeri, başarı varyantı olarak birim tipi `()` ve hata varyantı olarak bir [`MapToError`] içeren bir [`Result`]'tur; bu da [`Mapper::map_to`] metodunun döndürdüğü hata tipidir. Hata tipini yeniden kullanmak burada mantıklıdır, çünkü `map_to` metodu bu fonksiyondaki ana hata kaynağıdır.

[`Mapper`]:https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html
[`FrameAllocator`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.FrameAllocator.html
[`Size4KiB`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/enum.Size4KiB.html
[`Result`]: https://doc.rust-lang.org/core/result/enum.Result.html
[`MapToError`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/enum.MapToError.html
[`Mapper::map_to`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html#method.map_to

Uygulama iki kısma ayrılabilir:

- **Sayfa aralığını oluşturmak:** Eşlemek istediğimiz sayfaların bir aralığını oluşturmak için, `HEAP_START` işaretçisini bir [`VirtAddr`] tipine dönüştürüyoruz. Ardından `HEAP_SIZE`'ı ekleyerek ondan heap bitiş adresini hesaplıyoruz. Dahil edici (inclusive) bir sınır istiyoruz (heap'in son baytının adresi), bu yüzden 1 çıkarıyoruz. Sonra, [`containing_address`] fonksiyonunu kullanarak adresleri [`Page`] tiplerine dönüştürüyoruz. Son olarak, [`Page::range_inclusive`] fonksiyonunu kullanarak başlangıç ve bitiş sayfalarından bir sayfa aralığı oluşturuyoruz.

- **Sayfaları eşlemek:** İkinci adım, az önce oluşturduğumuz sayfa aralığının tüm sayfalarını eşlemektir. Bunun için, bir `for` döngüsü kullanarak bu sayfalar üzerinde iterasyon yapıyoruz. Her sayfa için şunları yapıyoruz:

    - Sayfanın eşlenmesi gereken bir fiziksel frame'i [`FrameAllocator::allocate_frame`] metodunu kullanarak ayırıyoruz. Bu metot, geriye frame kalmadığında [`None`] döndürür. Bu durumu, [`Option::ok_or`] metodu aracılığıyla onu bir [`MapToError::FrameAllocationFailed`] hatasına eşleyerek ve ardından bir hata durumunda erken dönmek için [soru işareti operatörünü][question mark operator] uygulayarak ele alıyoruz.

    - Sayfa için gereken `PRESENT` bayrağını ve `WRITABLE` bayrağını ayarlıyoruz. Bu bayraklarla, hem okuma hem yazma erişimlerine izin verilir; ki bu heap belleği için mantıklıdır.

    - Aktif sayfa tablosunda eşlemeyi oluşturmak için [`Mapper::map_to`] metodunu kullanıyoruz. Metot başarısız olabilir, bu yüzden hatayı çağırana iletmek için yine [soru işareti operatörünü][question mark operator] kullanıyoruz. Başarı durumunda, metot bir [`MapperFlush`] örneği döndürür; bunu [`flush`] metodunu kullanarak [_translation lookaside buffer_]'ı güncellemek için kullanabiliriz.

[`VirtAddr`]: https://docs.rs/x86_64/0.14.2/x86_64/addr/struct.VirtAddr.html
[`Page`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/struct.Page.html
[`containing_address`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/struct.Page.html#method.containing_address
[`Page::range_inclusive`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/struct.Page.html#method.range_inclusive
[`FrameAllocator::allocate_frame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.FrameAllocator.html#tymethod.allocate_frame
[`None`]: https://doc.rust-lang.org/core/option/enum.Option.html#variant.None
[`MapToError::FrameAllocationFailed`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/enum.MapToError.html#variant.FrameAllocationFailed
[`Option::ok_or`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.ok_or
[question mark operator]: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html
[`MapperFlush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html
[_translation lookaside buffer_]: @/edition-2/posts/08-paging-introduction/index.tr.md#the-translation-lookaside-buffer
[`flush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html#method.flush

Son adım, bu fonksiyonu `kernel_main`'imizden çağırmaktır:

```rust
// src/main.rs içinde

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::allocator; // yeni içe aktarma
    use blog_os::memory::{self, BootInfoFrameAllocator};

    println!("Hello World{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    // yeni
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    let x = Box::new(41);

    // […] test modunda `test_main` çağır

    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

Bağlam için burada fonksiyonun tamamını gösteriyoruz. Tek yeni satırlar `blog_os::allocator` içe aktarması ve `allocator::init_heap` fonksiyonuna yapılan çağrıdır. `init_heap` fonksiyonu bir hata döndürmesi durumunda, bu hatayı ele almak için şu anda mantıklı bir yolumuz olmadığından [`Result::expect`] metodunu kullanarak panic yapıyoruz.

[`Result::expect`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.expect

Artık kullanılmaya hazır, eşlenmiş bir heap bellek bölgemiz var. `Box::new` çağrısı hâlâ eski `Dummy` allocator'ımızı kullanır, bu yüzden onu çalıştırdığınızda hâlâ "out of memory" hatasını göreceksiniz. Düzgün bir allocator kullanarak bunu düzeltelim.

## Bir Allocator Crate'i Kullanmak {#using-an-allocator-crate}

Bir allocator uygulamak biraz karmaşık olduğundan, harici bir allocator crate'i kullanarak başlıyoruz. Kendi allocator'ımızı nasıl uygulayacağımızı bir sonraki yazıda öğreneceğiz.

`no_std` uygulamaları için basit bir allocator crate'i [`linked_list_allocator`] crate'idir. Adı, deallocate edilmiş bellek bölgelerini takip etmek için bağlı liste (linked list) veri yapısı kullanmasından gelir. Bu yaklaşımın daha ayrıntılı bir açıklaması için bir sonraki yazıya bakın.

Crate'i kullanmak için, önce `Cargo.toml`'umuzda ona bir bağımlılık eklememiz gerekir:

[`linked_list_allocator`]: https://github.com/phil-opp/linked-list-allocator/

```toml
# Cargo.toml içinde

[dependencies]
linked_list_allocator = "0.9.0"
```

Ardından sahte allocator'ımızı crate'in sağladığı allocator ile değiştirebiliriz:

```rust
// src/allocator.rs içinde

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();
```

Struct'ın adı `LockedHeap`'tir, çünkü senkronizasyon için [`spinning_top::Spinlock`] tipini kullanır. Bu gereklidir, çünkü birden çok thread `ALLOCATOR` static'ine aynı anda erişebilir. Her zaman olduğu gibi, bir spinlock veya mutex kullanırken yanlışlıkla bir deadlock'a neden olmamaya dikkat etmemiz gerekir. Bu, interrupt handler'larda herhangi bir ayırma yapmamamız gerektiği anlamına gelir; çünkü onlar keyfi bir zamanda çalışabilir ve devam etmekte olan bir ayırmayı kesebilir.

[`spinning_top::Spinlock`]: https://docs.rs/spinning_top/0.1.0/spinning_top/type.Spinlock.html

`LockedHeap`'i global allocator olarak ayarlamak yeterli değildir. Bunun nedeni, herhangi bir destek belleği olmadan bir allocator oluşturan [`empty`] yapıcı fonksiyonunu kullanmamızdır. Sahte allocator'ımız gibi, o da `alloc`'ta her zaman bir hata döndürür. Bunu düzeltmek için, allocator'ı heap'i oluşturduktan sonra başlatmamız gerekir:

[`empty`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.LockedHeap.html#method.empty

```rust
// src/allocator.rs içinde

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // […] tüm heap sayfalarını fiziksel frame'lere eşle

    // yeni
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
```

Sarmalanmış [`Heap`] örneğine özel bir referans almak için `LockedHeap` tipinin iç spinlock'undaki [`lock`] metodunu kullanıyoruz; ardından onun üzerinde heap sınırlarını argüman olarak alan [`init`] metodunu çağırıyoruz. [`init`] fonksiyonu zaten heap belleğine yazmaya çalıştığından, heap'i yalnızca heap sayfalarını eşledikten _sonra_ başlatmalıyız.

[`lock`]: https://docs.rs/lock_api/0.3.3/lock_api/struct.Mutex.html#method.lock
[`Heap`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html
[`init`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.init

Heap'i başlattıktan sonra, artık yerleşik [`alloc`] crate'inin tüm ayırma ve koleksiyon tiplerini hatasız kullanabiliriz:

```rust
// src/main.rs içinde

use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] interrupt'ları, mapper'ı, frame_allocator'ı, heap'i başlat

    // heap'te bir sayı ayır
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    // dinamik boyutlu bir vektör oluştur
    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    // referans sayımlı bir vektör oluştur -> sayım 0'a ulaştığında serbest bırakılır
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!("current reference count is {}", Rc::strong_count(&cloned_reference));
    core::mem::drop(reference_counted);
    println!("reference count is {} now", Rc::strong_count(&cloned_reference));

    // […] test bağlamında `test_main` çağır
    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

Bu kod örneği [`Box`], [`Vec`] ve [`Rc`] tiplerinin bazı kullanımlarını gösterir. `Box` ve `Vec` tipleri için, alttaki heap işaretçilerini [`{:p}` biçimlendirme belirteci][`{:p}` formatting specifier] kullanarak yazdırıyoruz. `Rc`'yi sergilemek için, referans sayımlı bir heap değeri oluşturuyor ve bir örneği drop etmeden önce ve sonra ([`core::mem::drop`] kullanarak) mevcut referans sayısını yazdırmak için [`Rc::strong_count`] fonksiyonunu kullanıyoruz.

[`Vec`]: https://doc.rust-lang.org/alloc/vec/
[`Rc`]: https://doc.rust-lang.org/alloc/rc/
[`{:p}` formatting specifier]: https://doc.rust-lang.org/core/fmt/trait.Pointer.html
[`Rc::strong_count`]: https://doc.rust-lang.org/alloc/rc/struct.Rc.html#method.strong_count
[`core::mem::drop`]: https://doc.rust-lang.org/core/mem/fn.drop.html

Onu çalıştırdığımızda, aşağıdakini görüyoruz:

![QEMU `heap_value at 0x444444440000`, `vec at 0x4444444408000`, `current reference count is 2`, `reference count is 1 now` yazdırıyor](qemu-alloc-showcase.png)

Beklendiği gibi, `0x_4444_4444_*` önekiyle başlayan işaretçinin belirttiği gibi, `Box` ve `Vec` değerlerinin heap'te var olduğunu görüyoruz. Referans sayımlı değer de beklendiği gibi davranıyor; `clone` çağrısından sonra referans sayısı 2 ve örneklerden biri drop edildikten sonra tekrar 1.

Vektörün `0x800` ofsetinde başlamasının nedeni, box'lanmış değerin `0x800` bayt büyüklüğünde olması değil, vektörün kapasitesini artırması gerektiğinde meydana gelen [yeniden ayırmalardır (reallocations)][reallocations]. Örneğin, vektörün kapasitesi 32 olduğunda ve bir sonraki elemanı eklemeye çalıştığımızda, vektör perde arkasında 64 kapasiteli yeni bir destek dizisi ayırır ve tüm elemanları kopyalar. Sonra eski ayırmayı serbest bırakır.

[reallocations]: https://doc.rust-lang.org/alloc/vec/struct.Vec.html#capacity-and-reallocation

Elbette, `alloc` crate'inde artık kernel'imizde kullanabileceğimiz çok daha fazla ayırma ve koleksiyon tipi var; bunlar arasında:

- thread güvenli referans sayımlı işaretçi [`Arc`]
- sahipli dize tipi [`String`] ve [`format!`] makrosu
- [`LinkedList`]
- büyüyebilir halka arabelleği (ring buffer) [`VecDeque`]
- [`BinaryHeap`] öncelik kuyruğu
- [`BTreeMap`] ve [`BTreeSet`]

[`Arc`]: https://doc.rust-lang.org/alloc/sync/struct.Arc.html
[`String`]: https://doc.rust-lang.org/alloc/string/struct.String.html
[`format!`]: https://doc.rust-lang.org/alloc/macro.format.html
[`LinkedList`]: https://doc.rust-lang.org/alloc/collections/linked_list/struct.LinkedList.html
[`VecDeque`]: https://doc.rust-lang.org/alloc/collections/vec_deque/struct.VecDeque.html
[`BinaryHeap`]: https://doc.rust-lang.org/alloc/collections/binary_heap/struct.BinaryHeap.html
[`BTreeMap`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html
[`BTreeSet`]: https://doc.rust-lang.org/alloc/collections/btree_set/struct.BTreeSet.html

Bu tipler, thread listeleri, zamanlama kuyrukları veya async/await desteği uygulamak istediğimizde çok yararlı olacak.

## Bir Test Eklemek {#adding-a-test}

Yeni ayırma kodumuzu yanlışlıkla bozmadığımızdan emin olmak için, onun için bir entegrasyon testi eklemeliyiz. Aşağıdaki içerikle yeni bir `tests/heap_allocation.rs` dosyası oluşturarak başlıyoruz:

```rust
// tests/heap_allocation.rs içinde

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

`lib.rs`'imizden `test_runner` ve `test_panic_handler` fonksiyonlarını yeniden kullanıyoruz. Ayırmaları test etmek istediğimiz için, `extern crate alloc` ifadesi aracılığıyla `alloc` crate'ini etkinleştiriyoruz. Test ön kalıbı (boilerplate) hakkında daha fazla bilgi için [_Test Etme_][_Testing_] yazısına göz atın.

[_Testing_]: @/edition-2/posts/04-testing/index.tr.md

`main` fonksiyonunun uygulaması şöyle görünür:

```rust
// tests/heap_allocation.rs içinde

fn main(boot_info: &'static BootInfo) -> ! {
    use blog_os::allocator;
    use blog_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    blog_os::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    test_main();
    loop {}
}
```

`main.rs`'imizdeki `kernel_main` fonksiyonuna çok benzer; farkları, `println` çağırmamamız, herhangi bir örnek ayırma içermememiz ve `test_main`'i koşulsuz çağırmamızdır.

Artık birkaç test senaryosu eklemeye hazırız. İlk olarak, [`Box`] kullanarak bazı basit ayırmalar yapan ve temel ayırmaların çalıştığından emin olmak için ayrılan değerleri kontrol eden bir test ekliyoruz:

```rust
// tests/heap_allocation.rs içinde
use alloc::boxed::Box;

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}
```

En önemlisi, bu test hiçbir ayırma hatasının meydana gelmediğini doğrular.

Ardından, hem büyük ayırmaları hem de (yeniden ayırmalar nedeniyle) birden çok ayırmayı test etmek için yinelemeli olarak büyük bir vektör oluşturuyoruz:

```rust
// tests/heap_allocation.rs içinde

use alloc::vec::Vec;

#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}
```

Toplamı, [n. kısmi toplam][n-th partial sum] formülüyle karşılaştırarak doğruluyoruz. Bu bize, ayrılan değerlerin hepsinin doğru olduğuna dair biraz güven verir.

[n-th partial sum]: https://en.wikipedia.org/wiki/1_%2B_2_%2B_3_%2B_4_%2B_%E2%8B%AF#Partial_sums

Üçüncü test olarak, art arda on bin ayırma oluşturuyoruz:

```rust
// tests/heap_allocation.rs içinde

use blog_os::allocator::HEAP_SIZE;

#[test_case]
fn many_boxes() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}
```

Bu test, allocator'ın serbest bırakılan belleği sonraki ayırmalar için yeniden kullandığından emin olur; çünkü aksi takdirde bellek tükenir. Bu, bir allocator için bariz bir gereksinim gibi görünebilir, ancak bunu yapmayan allocator tasarımları da vardır. Bir örnek, bir sonraki yazıda açıklanacak olan bump allocator tasarımıdır.

Yeni entegrasyon testimizi çalıştıralım:

```
> cargo test --test heap_allocation
[…]
Running 3 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
```

Üç testin de hepsi başarılı oldu! Tüm birim ve entegrasyon testlerini çalıştırmak için `cargo test`'i de (`--test` argümanı olmadan) çağırabilirsiniz.

## Özet

Bu yazı dinamik belleğe bir giriş yaptı ve onun neden ve nerede gerekli olduğunu açıkladı. Rust'ın borrow checker'ının yaygın güvenlik açıklarını nasıl önlediğini gördük ve Rust'ın ayırma API'sinin nasıl çalıştığını öğrendik.

Sahte bir allocator kullanarak Rust'ın allocator arayüzünün minimal bir uygulamasını oluşturduktan sonra, kernel'imiz için düzgün bir heap bellek bölgesi oluşturduk. Bunun için, heap için bir sanal adres aralığı tanımladık ve ardından önceki yazıdaki `Mapper` ve `FrameAllocator`'ı kullanarak o aralığın tüm sayfalarını fiziksel frame'lere eşledik.

Son olarak, kernel'imize düzgün bir allocator eklemek için `linked_list_allocator` crate'ine bir bağımlılık ekledik. Bu allocator ile, `alloc` crate'inden `Box`, `Vec` ve diğer ayırma ve koleksiyon tiplerini kullanabildik.

## Sırada ne var?

Bu yazıda heap ayırma desteğini zaten ekledik, ancak işin çoğunu `linked_list_allocator` crate'ine bıraktık. Bir sonraki yazı, bir allocator'ın sıfırdan nasıl uygulanabileceğini ayrıntılı olarak gösterecek. Olası birden çok allocator tasarımını sunacak, onların basit sürümlerinin nasıl uygulanacağını gösterecek ve avantaj ve dezavantajlarını açıklayacak.
