+++
title = "Allocator Tasarımları"
weight = 11
path = "tr/allocator-designs"
date = 2020-01-20

[extra]
chapter = "Memory Management"

# Please update this when updating the translation
translation_based_on_commit = "1132d7a3835dc6c0b3fd8f6b45c9295a9bc1f837"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

Bu yazı, heap allocator'ların sıfırdan nasıl uygulanacağını açıklar. Bump ayırma, bağlı liste ayırma ve sabit boyutlu blok ayırma dahil olmak üzere farklı allocator tasarımlarını sunar ve tartışır. Üç tasarımın her biri için, kernel'imizde kullanılabilecek temel bir uygulama oluşturacağız.

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-11`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-11

<!-- toc -->

## Giriş

[Önceki yazıda][previous post], kernel'imize temel heap ayırma desteği ekledik. Bunun için, sayfa tablolarında [yeni bir bellek bölgesi oluşturduk][map-heap] ve o belleği yönetmek için [`linked_list_allocator` crate'ini kullandık][use-alloc-crate]. Artık çalışan bir heap'imiz olsa da, işin nasıl çalıştığını anlamaya çalışmadan çoğunu allocator crate'ine bıraktık.

[previous post]: @/edition-2/posts/10-heap-allocation/index.tr.md
[map-heap]: @/edition-2/posts/10-heap-allocation/index.tr.md#creating-a-kernel-heap
[use-alloc-crate]: @/edition-2/posts/10-heap-allocation/index.tr.md#using-an-allocator-crate

Bu yazıda, mevcut bir allocator crate'ine güvenmek yerine kendi heap allocator'ımızı sıfırdan nasıl oluşturacağımızı göstereceğiz. Basit bir _bump allocator_ ve temel bir _sabit boyutlu blok allocator'ı_ dahil olmak üzere farklı allocator tasarımlarını tartışacak ve bu bilgiyi (`linked_list_allocator` crate'ine kıyasla) geliştirilmiş performansa sahip bir allocator uygulamak için kullanacağız.

### Tasarım Hedefleri

Bir allocator'ın sorumluluğu, mevcut heap belleğini yönetmektir. `alloc` çağrılarında kullanılmayan belleği döndürmesi ve `dealloc` tarafından serbest bırakılan belleği takip etmesi gerekir, böylece tekrar yeniden kullanılabilir. En önemlisi, başka bir yerde zaten kullanımda olan belleği asla teslim etmemelidir; çünkü bu tanımsız davranışa neden olurdu.

Doğruluğun yanı sıra, pek çok ikincil tasarım hedefi vardır. Örneğin, allocator mevcut belleği etkili bir şekilde kullanmalı ve [_parçalanmayı (fragmentation)_] düşük tutmalıdır. Ayrıca, eşzamanlı (concurrent) uygulamalar için iyi çalışmalı ve herhangi bir sayıda işlemciye ölçeklenmelidir. Maksimum performans için, [önbellek konumsallığını (cache locality)][cache locality] iyileştirmek ve [yanlış paylaşımdan (false sharing)][false sharing] kaçınmak için bellek düzenini CPU önbelleklerine göre bile optimize edebilir.

[cache locality]: https://www.geeksforgeeks.org/locality-of-reference-and-cache-operation-in-cache-memory/
[_parçalanmayı (fragmentation)_]: https://en.wikipedia.org/wiki/Fragmentation_(computing)
[false sharing]: https://mechanical-sympathy.blogspot.de/2011/07/false-sharing.html

Bu gereksinimler iyi allocator'ları çok karmaşık hale getirebilir. Örneğin, [jemalloc]'un 30.000'den fazla satır kodu var. Bu karmaşıklık, tek bir hatanın ciddi güvenlik açıklarına yol açabileceği kernel kodunda genellikle istenmez. Neyse ki, kernel kodunun ayırma örüntüleri genellikle kullanıcı alanı koduna kıyasla çok daha basittir, bu yüzden nispeten basit allocator tasarımları çoğu zaman yeterli olur.

[jemalloc]: http://jemalloc.net/

Aşağıda, üç olası kernel allocator tasarımını sunuyor ve avantaj ve dezavantajlarını açıklıyoruz.

## Bump Allocator {#bump-allocator}

En basit allocator tasarımı bir _bump allocator_'dır (_stack allocator_ olarak da bilinir). Belleği doğrusal olarak ayırır ve yalnızca ayrılan bayt sayısını ve ayırma sayısını takip eder. Yalnızca çok belirli kullanım senaryolarında yararlıdır, çünkü ciddi bir sınırlaması vardır: yalnızca tüm belleği bir kerede serbest bırakabilir.

### Fikir

Bir bump allocator'ın arkasındaki fikir, kullanılmayan belleğin başlangıcına işaret eden bir `next` değişkenini artırarak (_"bump"_layarak) belleği doğrusal olarak ayırmaktır. Başlangıçta, `next`, heap'in başlangıç adresine eşittir. Her ayırmada, `next`, ayırma boyutu kadar artırılır; böylece her zaman kullanılan ve kullanılmayan bellek arasındaki sınıra işaret eder:

![Üç zaman noktasındaki heap bellek alanı:
 1: Heap'in başında tek bir ayırma var; `next` işaretçisi onun sonuna işaret eder.
 2: İlkinin hemen ardına ikinci bir ayırma eklendi; `next` işaretçisi ikinci ayırmanın sonuna işaret eder.
 3: İkincinin hemen ardına üçüncü bir ayırma eklendi; `next` işaretçisi üçüncü ayırmanın sonuna işaret eder.](bump-allocation.svg)

`next` işaretçisi yalnızca tek bir yönde hareket eder ve böylece aynı bellek bölgesini asla iki kez teslim etmez. Heap'in sonuna ulaştığında, daha fazla bellek ayrılamaz ve bir sonraki ayırmada bellek-yetersiz (out-of-memory) hatasıyla sonuçlanır.

Bir bump allocator genellikle, her `alloc` çağrısında 1 artırılan ve her `dealloc` çağrısında 1 azaltılan bir ayırma sayacıyla uygulanır. Ayırma sayacı sıfıra ulaştığında, bu, heap'teki tüm ayırmaların deallocate edildiği anlamına gelir. Bu durumda, `next` işaretçisi heap'in başlangıç adresine sıfırlanabilir; böylece tüm heap belleği yeniden ayırmalar için kullanılabilir olur.

### Uygulama

Uygulamamıza yeni bir `allocator::bump` alt modülü bildirerek başlıyoruz:

```rust
// src/allocator.rs içinde

pub mod bump;
```

Alt modülün içeriği, aşağıdaki içerikle oluşturduğumuz yeni bir `src/allocator/bump.rs` dosyasında bulunur:

```rust
// src/allocator/bump.rs içinde

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    /// Yeni, boş bir bump allocator oluşturur.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    /// Bump allocator'ı verilen heap sınırlarıyla başlatır.
    ///
    /// Bu metot unsafe'tir, çünkü çağıranın verilen bellek aralığının
    /// kullanılmadığından emin olması gerekir. Ayrıca, bu metot yalnızca bir
    /// kez çağrılmalıdır.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}
```

`heap_start` ve `heap_end` alanları, heap bellek bölgesinin alt ve üst sınırlarını takip eder. Çağıranın bu adreslerin geçerli olduğundan emin olması gerekir; aksi takdirde allocator geçersiz bellek döndürür. Bu nedenle, `init` fonksiyonunun çağrılması `unsafe` olmalıdır.

`next` alanının amacı her zaman heap'in ilk kullanılmayan baytına, yani bir sonraki ayırmanın başlangıç adresine işaret etmektir. `init` fonksiyonunda `heap_start`'a ayarlanır, çünkü başlangıçta tüm heap kullanılmamıştır. Her ayırmada, bu alan aynı bellek bölgesini iki kez döndürmediğimizden emin olmak için ayırma boyutu kadar artırılacak (_"bump"_lanacak).

`allocations` alanı, son ayırma serbest bırakıldıktan sonra allocator'ı sıfırlamak amacıyla aktif ayırmalar için basit bir sayaçtır. 0 ile başlatılır.

Başlatmayı doğrudan `new`'de yapmak yerine ayrı bir `init` fonksiyonu oluşturmayı, arayüzü `linked_list_allocator` crate'inin sağladığı allocator ile aynı tutmak için seçtik. Bu sayede, allocator'lar ek kod değişikliği olmadan değiştirilebilir.

### `GlobalAlloc`'u Uygulamak

[Önceki yazıda açıklandığı gibi][global-alloc], tüm heap allocator'ların şu şekilde tanımlanan [`GlobalAlloc`] trait'ini uygulaması gerekir:

[global-alloc]: @/edition-2/posts/10-heap-allocation/index.tr.md#the-allocator-interface
[`GlobalAlloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html

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

Yalnızca `alloc` ve `dealloc` metotları gereklidir; diğer iki metodun varsayılan uygulamaları vardır ve atlanabilir.

#### İlk Uygulama Denemesi

`BumpAllocator`'ımız için `alloc` metodunu uygulamayı deneyelim:

```rust
// src/allocator/bump.rs içinde

use alloc::alloc::{GlobalAlloc, Layout};

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // TODO hizalama ve sınır kontrolü
        let alloc_start = self.next;
        self.next = alloc_start + layout.size();
        self.allocations += 1;
        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        todo!();
    }
}
```

İlk olarak, ayırmamız için başlangıç adresi olarak `next` alanını kullanıyoruz. Ardından `next` alanını, heap'teki bir sonraki kullanılmayan adres olan ayırmanın bitiş adresine işaret edecek şekilde güncelliyoruz. Ayırmanın başlangıç adresini bir `*mut u8` işaretçisi olarak döndürmeden önce, `allocations` sayacını 1 artırıyoruz.

Herhangi bir sınır kontrolü veya hizalama ayarlaması yapmadığımıza dikkat edin, bu yüzden bu uygulama henüz güvenli değil. Bu pek önemli değil, çünkü zaten aşağıdaki hatayla derlenmiyor:

```
error[E0594]: cannot assign to `self.next` which is behind a `&` reference
  --> src/allocator/bump.rs:29:9
   |
29 |         self.next = alloc_start + layout.size();
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `self` is a `&` reference, so the data it refers to cannot be written
```

(Aynı hata `self.allocations += 1` satırı için de oluşur. Kısalık adına onu burada atladık.)

Hata, `GlobalAlloc` trait'inin [`alloc`] ve [`dealloc`] metotlarının yalnızca değiştirilemez bir `&self` referansı üzerinde çalışması nedeniyle oluşur, bu yüzden `next` ve `allocations` alanlarını güncellemek mümkün değildir. Bu sorunludur, çünkü `next`'i her ayırmada güncellemek bir bump allocator'ın temel ilkesidir.

[`alloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.alloc
[`dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc

#### `GlobalAlloc` ve Değiştirilebilirlik {#globalalloc-and-mutability}

Bu değiştirilebilirlik (mutability) sorununa olası bir çözüme bakmadan önce, `GlobalAlloc` trait metotlarının neden `&self` argümanlarıyla tanımlandığını anlamaya çalışalım: [Önceki yazıda gördüğümüz gibi][global-allocator], global heap allocator, `#[global_allocator]` özniteliğini `GlobalAlloc` trait'ini uygulayan bir `static`'e ekleyerek tanımlanır. Statik değişkenler Rust'ta değiştirilemezdir, bu yüzden statik allocator üzerinde `&mut self` alan bir metodu çağırmanın bir yolu yoktur. Bu nedenle, `GlobalAlloc`'un tüm metotları yalnızca değiştirilemez bir `&self` referansı alır.

[global-allocator]:  @/edition-2/posts/10-heap-allocation/index.tr.md#the-global-allocator-attribute

Neyse ki, bir `&self` referansından bir `&mut self` referansı elde etmenin bir yolu var: Allocator'ı bir [`spin::Mutex`] spinlock'ında sararak senkronize [iç değiştirilebilirlik (interior mutability)][interior mutability] kullanabiliriz. Bu tip, [karşılıklı dışlama (mutual exclusion)][mutual exclusion] gerçekleştiren ve böylece bir `&self` referansını güvenli bir şekilde bir `&mut self` referansına dönüştüren bir `lock` metodu sağlar. Sarmalayıcı tipi kernel'imizde birçok kez kullandık zaten, örneğin [VGA metin arabelleği][vga-mutex] için.

[interior mutability]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
[vga-mutex]: @/edition-2/posts/03-vga-text-buffer/index.tr.md#spinlocks
[`spin::Mutex`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html
[mutual exclusion]: https://en.wikipedia.org/wiki/Mutual_exclusion

#### Bir `Locked` Sarmalayıcı Tipi {#a-locked-wrapper-type}

`spin::Mutex` sarmalayıcı tipinin yardımıyla, bump allocator'ımız için `GlobalAlloc` trait'ini uygulayabiliriz. Püf nokta, trait'i doğrudan `BumpAllocator` için değil, sarmalanmış `spin::Mutex<BumpAllocator>` tipi için uygulamaktır:

```rust
unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {…}
```

Ne yazık ki bu yine de çalışmaz, çünkü Rust derleyicisi başka crate'lerde tanımlanan tipler için trait uygulamalarına izin vermez:

```
error[E0117]: only traits defined in the current crate can be implemented for arbitrary types
  --> src/allocator/bump.rs:28:1
   |
28 | unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^--------------------------
   | |                           |
   | |                           `spin::mutex::Mutex` is not defined in the current crate
   | impl doesn't use only types from inside the current crate
   |
   = note: define and implement a trait or new type instead
```

Bunu düzeltmek için, `spin::Mutex` etrafında kendi sarmalayıcı tipimizi oluşturmamız gerekir:

```rust
// src/allocator.rs içinde

/// Trait uygulamalarına izin vermek için spin::Mutex etrafında bir sarmalayıcı.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}
```

Tip, bir `spin::Mutex<A>` etrafında generic bir sarmalayıcıdır. Sarmalanan `A` tipine hiçbir kısıtlama getirmez, bu yüzden yalnızca allocator'ları değil, her türlü tipi sarmalamak için kullanılabilir. Verilen bir değeri sarmalayan basit bir `new` yapıcı fonksiyonu sağlar. Kolaylık için, sarmalanan `Mutex` üzerinde `lock` çağıran bir `lock` fonksiyonu da sağlar. `Locked` tipi diğer allocator uygulamaları için de yararlı olacak kadar genel olduğundan, onu üst `allocator` modülüne koyuyoruz.

#### `Locked<BumpAllocator>` için Uygulama

`Locked` tipi (`spin::Mutex`'in aksine) kendi crate'imizde tanımlanmıştır, bu yüzden onu bump allocator'ımız için `GlobalAlloc`'u uygulamak amacıyla kullanabiliriz. Tam uygulama şöyle görünür:

```rust
// src/allocator/bump.rs içinde

use super::{align_up, Locked};
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock(); // değiştirilebilir bir referans al

        let alloc_start = align_up(bump.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            ptr::null_mut() // bellek yetersiz
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock(); // değiştirilebilir bir referans al

        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}
```

Hem `alloc` hem de `dealloc` için ilk adım, sarmalanan allocator tipine değiştirilebilir bir referans almak amacıyla `inner` alanı aracılığıyla [`Mutex::lock`] metodunu çağırmaktır. Örnek, metodun sonuna kadar kilitli kalır, böylece çok thread'li bağlamlarda hiçbir veri yarışı meydana gelemez (yakında thread desteği ekleyeceğiz).

[`Mutex::lock`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html#method.lock

Önceki prototiple karşılaştırıldığında, `alloc` uygulaması artık hizalama gereksinimlerine uyuyor ve ayırmaların heap bellek bölgesinin içinde kalmasını sağlamak için bir sınır kontrolü gerçekleştiriyor. İlk adım, `next` adresini `Layout` argümanı tarafından belirtilen hizalamaya yuvarlamaktır. `align_up` fonksiyonunun kodu birazdan gösterilecek. Ardından, ayırmanın bitiş adresini elde etmek için istenen ayırma boyutunu `alloc_start`'a ekliyoruz. Büyük ayırmalarda tamsayı taşmasını (integer overflow) önlemek için [`checked_add`] metodunu kullanıyoruz. Bir taşma meydana gelirse veya ayırmanın elde edilen bitiş adresi heap'in bitiş adresinden büyükse, bir bellek-yetersiz durumunu bildirmek için null bir işaretçi döndürürüz. Aksi takdirde, `next` adresini güncelliyor ve önceki gibi `allocations` sayacını 1 artırıyoruz. Son olarak, bir `*mut u8` işaretçisine dönüştürülmüş `alloc_start` adresini döndürüyoruz.

[`checked_add`]: https://doc.rust-lang.org/std/primitive.usize.html#method.checked_add
[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html

`dealloc` fonksiyonu, verilen işaretçi ve `Layout` argümanlarını yok sayar. Bunun yerine, yalnızca `allocations` sayacını azaltır. Sayaç tekrar `0`'a ulaşırsa, bu, tüm ayırmaların tekrar serbest bırakıldığı anlamına gelir. Bu durumda, tüm heap belleğini yeniden kullanılabilir kılmak için `next` adresini `heap_start` adresine sıfırlar.

#### Adres Hizalama

`align_up` fonksiyonu, onu üst `allocator` modülüne koyabileceğimiz kadar geneldir. Temel bir uygulama şöyle görünür:

```rust
// src/allocator.rs içinde

/// Verilen `addr` adresini `align` hizalamasına yukarı doğru hizala.
fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr // addr zaten hizalı
    } else {
        addr - remainder + align
    }
}
```

Fonksiyon önce `addr`'ın `align`'a bölümünün [kalanını (remainder)][remainder] hesaplar. Kalan `0` ise, adres verilen hizalamayla zaten hizalanmıştır. Aksi takdirde, kalanı çıkararak (böylece yeni kalan 0 olur) ve ardından hizalamayı ekleyerek (böylece adres orijinal adresten küçük olmaz) adresi hizalarız.

[remainder]: https://en.wikipedia.org/wiki/Euclidean_division

Bunun bu fonksiyonu uygulamanın en verimli yolu olmadığına dikkat edin. Çok daha hızlı bir uygulama şöyle görünür:

```rust
/// Verilen `addr` adresini `align` hizalamasına yukarı doğru hizala.
///
/// `align`'ın ikinin bir kuvveti olmasını gerektirir.
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
```

Bu yöntem `align`'ın ikinin bir kuvveti olmasını gerektirir; bu da `GlobalAlloc` trait'inden (ve onun [`Layout`] parametresinden) yararlanarak garanti edilebilir. Bu, adresi çok verimli bir şekilde hizalamak için bir [bit maskesi (bitmask)][bitmask] oluşturmayı mümkün kılar. Nasıl çalıştığını anlamak için, sağ taraftan başlayarak adım adım gidelim:

[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html
[bitmask]: https://en.wikipedia.org/wiki/Mask_(computing)

- `align` ikinin bir kuvveti olduğundan, [ikili gösteriminin][binary representation] yalnızca tek bir biti ayarlıdır (örneğin `0b000100000`). Bu, `align - 1`'in tüm alt bitlerinin ayarlı olduğu anlamına gelir (örneğin `0b00011111`).
- `!` operatörü aracılığıyla [bit düzeyinde `NOT`][bitwise `NOT`] oluşturarak, `align`'dan düşük bitler hariç tüm bitleri ayarlı olan bir sayı elde ederiz (örneğin `0b…111111111100000`).
- Bir adres ile `!(align - 1)` üzerinde [bit düzeyinde `AND`][bitwise `AND`] gerçekleştirerek, adresi _aşağı doğru_ hizalarız. Bu, `align`'dan düşük olan tüm bitleri temizleyerek çalışır.
- Aşağı doğru değil yukarı doğru hizalamak istediğimiz için, bit düzeyinde `AND` gerçekleştirmeden önce `addr`'ı `align - 1` kadar artırırız. Bu sayede, zaten hizalı adresler aynı kalırken, hizalı olmayan adresler bir sonraki hizalama sınırına yuvarlanır.

[binary representation]: https://en.wikipedia.org/wiki/Binary_number#Representation
[bitwise `NOT`]: https://en.wikipedia.org/wiki/Bitwise_operation#NOT
[bitwise `AND`]: https://en.wikipedia.org/wiki/Bitwise_operation#AND

Hangi varyantı seçeceğiniz size kalmış. Her ikisi de yalnızca farklı yöntemler kullanarak aynı sonucu hesaplar.

### Kullanmak

`linked_list_allocator` crate'i yerine bump allocator'ı kullanmak için, `allocator.rs`'teki `ALLOCATOR` static'ini güncellememiz gerekir:

```rust
// src/allocator.rs içinde

use bump::BumpAllocator;

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());
```

Burada `BumpAllocator::new` ve `Locked::new`'i [`const` fonksiyonları][`const` functions] olarak bildirmiş olmamız önemli hale gelir. Eğer normal fonksiyonlar olsalardı, bir `static`'in başlatma ifadesinin derleme zamanında değerlendirilebilir olması gerektiği için bir derleme hatası oluşurdu.

[`const` functions]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

`init_heap` fonksiyonumuzdaki `ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE)` çağrısını değiştirmemize gerek yok, çünkü bump allocator, `linked_list_allocator`'ın sağladığı allocator ile aynı arayüzü sağlar.

Artık kernel'imiz bump allocator'ımızı kullanıyor! Önceki yazıda oluşturduğumuz [`heap_allocation` testleri][`heap_allocation` tests] dahil her şey hâlâ çalışmalı:

[`heap_allocation` tests]: @/edition-2/posts/10-heap-allocation/index.tr.md#adding-a-test

```
> cargo test --test heap_allocation
[…]
Running 3 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
```

### Tartışma

Bump ayırmanın büyük avantajı çok hızlı olmasıdır. Uygun bir bellek bloğunu aktif olarak araması ve `alloc` ile `dealloc`'ta çeşitli kayıt tutma görevleri gerçekleştirmesi gereken diğer allocator tasarımlarına (aşağıya bakın) kıyasla, bir bump allocator yalnızca birkaç assembly komutuna [optimize edilebilir][bump downwards]. Bu, bump allocator'ları ayırma performansını optimize etmek için yararlı kılar; örneğin bir [sanal DOM kütüphanesi][virtual DOM library] oluştururken.

[bump downwards]: https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
[virtual DOM library]: https://hacks.mozilla.org/2019/03/fast-bump-allocated-virtual-doms-with-rust-and-wasm/

Bir bump allocator nadiren global allocator olarak kullanılsa da, bump ayırma ilkesi genellikle [arena ayırma (arena allocation)][arena allocation] biçiminde uygulanır; bu da temelde performansı iyileştirmek için tek tek ayırmaları bir araya toplar. Rust için bir arena allocator örneği [`toolshed`] crate'inde bulunur.

[arena allocation]: https://mgravell.github.io/Pipelines.Sockets.Unofficial/docs/arenas.html
[`toolshed`]: https://docs.rs/toolshed/0.8.1/toolshed/index.html

#### Bump Allocator'ın Dezavantajı

Bir bump allocator'ın ana sınırlaması, deallocate edilmiş belleği yalnızca tüm ayırmalar serbest bırakıldıktan sonra yeniden kullanabilmesidir. Bu, tek bir uzun ömürlü ayırmanın bile bellek yeniden kullanımını önlemeye yettiği anlamına gelir. Bunu, `many_boxes` testinin bir varyasyonunu eklediğimizde görebiliriz:

```rust
// tests/heap_allocation.rs içinde

#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1); // yeni
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1); // yeni
}
```

`many_boxes` testi gibi, bu test de allocator serbest bırakılan belleği yeniden kullanmazsa bir bellek-yetersiz hatasına yol açmak için çok sayıda ayırma oluşturur. Buna ek olarak, test, döngünün tüm yürütülmesi boyunca var olan bir `long_lived` ayırması oluşturur.

Yeni testimizi çalıştırmaya çalıştığımızda, gerçekten başarısız olduğunu görüyoruz:

```
> cargo test --test heap_allocation
Running 4 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [failed]

Error: panicked at 'allocation error: Layout { size_: 8, align_: 8 }', src/lib.rs:86:5
```

Bu başarısızlığın neden meydana geldiğini ayrıntılı olarak anlamaya çalışalım: İlk olarak, `long_lived` ayırması heap'in başında oluşturulur ve böylece `allocations` sayacını 1 artırır. Döngünün her yinelemesi için, kısa ömürlü bir ayırma oluşturulur ve bir sonraki yineleme başlamadan önce doğrudan tekrar serbest bırakılır. Bu, `allocations` sayacının bir yinelemenin başında geçici olarak 2'ye yükseltildiği ve sonunda 1'e düşürüldüğü anlamına gelir. Şimdi sorun şu ki, bump allocator belleği yalnızca _tüm_ ayırmalar serbest bırakıldıktan sonra, yani `allocations` sayacı 0'a düştüğünde yeniden kullanabilir. Bu, döngünün sonundan önce gerçekleşmediği için, her döngü yinelemesi yeni bir bellek bölgesi ayırır ve birkaç yinelemeden sonra bir bellek-yetersiz hatasına yol açar.

#### Testi Düzeltmek?

Bump allocator'ımız için testi düzeltmek amacıyla yararlanabileceğimiz iki potansiyel hile var:

- `dealloc`'u, serbest bırakılan ayırmanın `alloc` tarafından döndürülen son ayırma olup olmadığını, bitiş adresini `next` işaretçisiyle karşılaştırarak kontrol edecek şekilde güncelleyebilirdik. Eşit olmaları durumunda, `next`'i serbest bırakılan ayırmanın başlangıç adresine güvenle sıfırlayabiliriz. Bu sayede, her döngü yinelemesi aynı bellek bloğunu yeniden kullanır.
- Ek bir `next_back` alanı kullanarak heap'in _sonundan_ bellek ayıran bir `alloc_back` metodu ekleyebilirdik. Sonra bu ayırma metodunu tüm uzun ömürlü ayırmalar için elle kullanabilir ve böylece heap'te kısa ömürlü ve uzun ömürlü ayırmaları ayırabilirdik. Bu ayrımın yalnızca her ayırmanın ne kadar süre var olacağı önceden açıksa işe yaradığını unutmayın. Bu yaklaşımın bir başka dezavantajı, ayırmaları elle gerçekleştirmenin zahmetli ve potansiyel olarak güvensiz olmasıdır.

Bu yaklaşımların ikisi de testi düzeltmek için işe yarasa da, yalnızca çok belirli durumlarda belleği yeniden kullanabildikleri için genel bir çözüm değildirler. Soru şu: _Serbest_ bırakılan tüm belleği yeniden kullanan genel bir çözüm var mı?

#### Serbest Bırakılan Tüm Belleği Yeniden Kullanmak?

[Önceki yazıda öğrendiğimiz gibi][heap-intro], ayırmalar keyfi olarak uzun süre var olabilir ve keyfi bir sırada serbest bırakılabilir. Bu, aşağıdaki örnekte gösterildiği gibi, potansiyel olarak sınırsız sayıda sürekli olmayan, kullanılmayan bellek bölgesini takip etmemiz gerektiği anlamına gelir:

[heap-intro]: @/edition-2/posts/10-heap-allocation/index.tr.md#dynamic-memory

![](allocation-fragmentation.svg)

Grafik, heap'i zaman içinde gösterir. Başlangıçta, tüm heap kullanılmamıştır ve `next` adresi `heap_start`'a eşittir (satır 1). Ardından ilk ayırma meydana gelir (satır 2). Satır 3'te, ikinci bir bellek bloğu ayrılır ve ilk ayırma serbest bırakılır. Satır 4'te çok daha fazla ayırma eklenir. Yarısı çok kısa ömürlüdür ve satır 5'te zaten serbest bırakılır; burada başka bir yeni ayırma da eklenir.

Satır 5 temel sorunu gösterir: Farklı boyutlarda beş kullanılmayan bellek bölgemiz var, ancak `next` işaretçisi yalnızca son bölgenin başlangıcına işaret edebilir. Bu örnek için diğer kullanılmayan bellek bölgelerinin başlangıç adreslerini ve boyutlarını boyutu 4 olan bir dizide saklayabilsek de, bu genel bir çözüm değildir; çünkü 8, 16 veya 1000 kullanılmayan bellek bölgesi olan bir örnek kolayca oluşturabilirdik.

Normalde, potansiyel olarak sınırsız sayıda öğemiz olduğunda, yalnızca heap'te ayrılmış bir koleksiyon kullanabiliriz. Bu bizim durumumuzda gerçekten mümkün değildir, çünkü heap allocator kendisine bağımlı olamaz (bu, bitmeyen özyinelemeye veya deadlock'lara neden olurdu). Bu yüzden farklı bir çözüm bulmamız gerekir.

## Bağlı Liste Allocator'ı {#linked-list-allocator}

Allocator'ları uygularken keyfi sayıda boş bellek alanını takip etmenin yaygın bir hilesi, bu alanların kendilerini destek deposu (backing storage) olarak kullanmaktır. Bu, bölgelerin hâlâ bir sanal adrese eşlenmiş ve fiziksel bir frame tarafından desteklenmiş olduğu, ancak saklanan bilgiye artık ihtiyaç duyulmadığı gerçeğinden yararlanır. Serbest bırakılan bölge hakkındaki bilgiyi bölgenin kendisinde saklayarak, ek belleğe ihtiyaç duymadan sınırsız sayıda serbest bırakılan bölgeyi takip edebiliriz.

En yaygın uygulama yaklaşımı, her düğümü serbest bırakılan bir bellek bölgesi olan tek bağlı bir liste (single linked list) oluşturmaktır:

![](linked-list-allocation.svg)

Her liste düğümü iki alan içerir: bellek bölgesinin boyutu ve bir sonraki kullanılmayan bellek bölgesine bir işaretçi. Bu yaklaşımla, sayılarından bağımsız olarak tüm kullanılmayan bölgeleri takip etmek için yalnızca ilk kullanılmayan bölgeye (`head` adı verilen) bir işaretçiye ihtiyacımız var. Elde edilen veri yapısına genellikle [_free list_] denir.

[_free list_]: https://en.wikipedia.org/wiki/Free_list

Adından tahmin edebileceğiniz gibi, bu, `linked_list_allocator` crate'inin kullandığı tekniktir. Bu tekniği kullanan allocator'lara genellikle _havuz allocator'ları (pool allocators)_ da denir.

### Uygulama

Aşağıda, serbest bırakılan bellek bölgelerini takip etmek için yukarıdaki yaklaşımı kullanan kendi basit `LinkedListAllocator` tipimizi oluşturacağız. Yazının bu kısmı gelecekteki yazılar için gerekli değildir, bu yüzden isterseniz uygulama ayrıntılarını atlayabilirsiniz.

#### Allocator Tipi {#the-allocator-type}

Yeni bir `allocator::linked_list` alt modülünde özel bir `ListNode` struct'ı oluşturarak başlıyoruz:

```rust
// src/allocator.rs içinde

pub mod linked_list;
```

```rust
// src/allocator/linked_list.rs içinde

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}
```

Grafikte olduğu gibi, bir liste düğümünün bir `size` alanı ve `Option<&'static mut ListNode>` tipiyle temsil edilen, bir sonraki düğüme isteğe bağlı bir işaretçisi vardır. `&'static mut` tipi, anlamsal olarak bir işaretçinin arkasındaki [sahipli (owned)][owned] bir nesneyi tanımlar. Temelde, kapsamın sonunda nesneyi serbest bırakan bir yıkıcısı (destructor) olmayan bir [`Box`]'tır.

[owned]: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html
[`Box`]: https://doc.rust-lang.org/alloc/boxed/index.html

`ListNode` için aşağıdaki metot kümesini uyguluyoruz:

```rust
// src/allocator/linked_list.rs içinde

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}
```

Tipin `new` adında basit bir yapıcı fonksiyonu ve temsil edilen bölgenin başlangıç ve bitiş adreslerini hesaplayan metotları vardır. `new` fonksiyonunu, statik bir bağlı liste allocator'ı oluştururken daha sonra gerekli olacak bir [const fonksiyonu][const function] yapıyoruz.

[const function]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

`ListNode` struct'ını bir yapı taşı olarak kullanarak, artık `LinkedListAllocator` struct'ını oluşturabiliriz:

```rust
// src/allocator/linked_list.rs içinde

pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    /// Boş bir LinkedListAllocator oluşturur.
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// Allocator'ı verilen heap sınırlarıyla başlatır.
    ///
    /// Bu fonksiyon unsafe'tir, çünkü çağıranın verilen heap sınırlarının
    /// geçerli olduğunu ve heap'in kullanılmadığını garanti etmesi gerekir.
    /// Bu metot yalnızca bir kez çağrılmalıdır.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    /// Verilen bellek bölgesini listenin başına ekler.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        todo!();
    }
}
```

Struct, ilk heap bölgesine işaret eden bir `head` düğümü içerir. Yalnızca `next` işaretçisinin değeriyle ilgileniyoruz, bu yüzden `ListNode::new` fonksiyonunda `size`'ı 0'a ayarlıyoruz. `head`'i yalnızca bir `&'static mut ListNode` yerine bir `ListNode` yapmanın, `alloc` metodunun uygulamasının daha basit olması avantajı vardır.

Bump allocator'da olduğu gibi, `new` fonksiyonu allocator'ı heap sınırlarıyla başlatmaz. API uyumluluğunu korumanın yanı sıra, bunun nedeni başlatma rutininin heap belleğine bir düğüm yazmayı gerektirmesidir; ki bu yalnızca çalışma zamanında gerçekleşebilir. Ancak `new` fonksiyonu, `ALLOCATOR` static'ini başlatmak için kullanılacağından derleme zamanında değerlendirilebilen bir [`const` fonksiyonu][`const` function] olmalıdır. Bu nedenle, yine ayrı, sabit olmayan bir `init` metodu sağlıyoruz.

[`const` function]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

`init` metodu, uygulaması birazdan gösterilecek bir `add_free_region` metodu kullanır. Şimdilik, her zaman panic yapan bir yer tutucu uygulama sağlamak için [`todo!`] makrosunu kullanıyoruz.

[`todo!`]: https://doc.rust-lang.org/core/macro.todo.html

#### `add_free_region` Metodu

`add_free_region` metodu, bağlı liste üzerindeki temel _push_ işlemini sağlar. Şu anda bu metodu yalnızca `init`'ten çağırıyoruz, ancak `dealloc` uygulamamızda da merkezi metot olacak. Hatırlayın, `dealloc` metodu ayrılmış bir bellek bölgesi tekrar serbest bırakıldığında çağrılır. Bu serbest bırakılan bellek bölgesini takip etmek için, onu bağlı listeye push'lamak istiyoruz.

`add_free_region` metodunun uygulaması şöyle görünür:

```rust
// src/allocator/linked_list.rs içinde

use super::align_up;
use core::mem;

impl LinkedListAllocator {
    /// Verilen bellek bölgesini listenin başına ekler.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // serbest bırakılan bölgenin ListNode tutabilecek kapasitede olduğundan emin ol
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // yeni bir liste düğümü oluştur ve onu listenin başına ekle
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        unsafe {
            node_ptr.write(node);
            self.head.next = Some(&mut *node_ptr)
        }
    }
}
```

Metot, argüman olarak bir bellek bölgesinin adresini ve boyutunu alır ve onu listenin başına ekler. İlk olarak, verilen bölgenin bir `ListNode` saklamak için gereken boyuta ve hizalamaya sahip olduğundan emin olur. Ardından düğümü oluşturur ve aşağıdaki adımlarla listeye ekler:

![](linked-list-allocator-push.svg)

Adım 0, `add_free_region` çağrılmadan önceki heap durumunu gösterir. Adım 1'de, metot grafikte `freed` olarak işaretlenmiş bellek bölgesiyle çağrılır. İlk kontrollerden sonra, metot kendi stack'inde serbest bırakılan bölgenin boyutuyla yeni bir `node` oluşturur. Ardından düğümün `next` işaretçisini mevcut `head` işaretçisine ayarlamak için [`Option::take`] metodunu kullanır ve böylece `head` işaretçisini `None`'a sıfırlar.

[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

Adım 2'de, metot yeni oluşturulan `node`'u [`write`] metodu aracılığıyla serbest bırakılan bellek bölgesinin başına yazar. Ardından `head` işaretçisini yeni düğüme yönlendirir. Elde edilen işaretçi yapısı biraz kaotik görünür, çünkü serbest bırakılan bölge her zaman listenin başına eklenir; ancak işaretçileri takip edersek, her boş bölgenin hâlâ `head` işaretçisinden ulaşılabilir olduğunu görürüz.

[`write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write

#### `find_region` Metodu

Bir bağlı liste üzerindeki ikinci temel işlem, bir girdi bulmak ve onu listeden kaldırmaktır. Bu, `alloc` metodunu uygulamak için gereken merkezi işlemdir. İşlemi bir `find_region` metodu olarak şu şekilde uyguluyoruz:

```rust
// src/allocator/linked_list.rs içinde

impl LinkedListAllocator {
    /// Verilen boyut ve hizalamaya sahip boş bir bölge arar ve onu listeden
    /// kaldırır.
    ///
    /// Liste düğümünün ve ayırmanın başlangıç adresinin bir tuple'ını döndürür.
    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut ListNode, usize)>
    {
        // mevcut liste düğümüne referans, her yineleme için güncellenir
        let mut current = &mut self.head;
        // bağlı listede yeterince büyük bir bellek bölgesi ara
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // bölge ayırma için uygun -> düğümü listeden kaldır
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // bölge uygun değil -> bir sonraki bölgeyle devam et
                current = current.next.as_mut().unwrap();
            }
        }

        // uygun bölge bulunamadı
        None
    }
}
```

Metot, liste elemanları üzerinde iterasyon yapmak için bir `current` değişkeni ve bir [`while let` döngüsü][`while let` loop] kullanır. Başlangıçta, `current`, (sahte) `head` düğümüne ayarlanır. Her yinelemede, ardından (`else` bloğunda) mevcut düğümün `next` alanına güncellenir. Bölge, verilen boyut ve hizalamaya sahip bir ayırma için uygunsa, bölge listeden kaldırılır ve `alloc_start` adresiyle birlikte döndürülür.

[`while let` loop]: https://doc.rust-lang.org/reference/expressions/loop-expr.html#while-let-patterns

`current.next` işaretçisi `None` olduğunda, döngü çıkar. Bu, tüm liste üzerinde iterasyon yaptığımız ancak bir ayırma için uygun bölge bulamadığımız anlamına gelir. Bu durumda, `None` döndürürüz. Bir bölgenin uygun olup olmadığı, uygulaması birazdan gösterilecek `alloc_from_region` fonksiyonu tarafından kontrol edilir.

Uygun bir bölgenin listeden nasıl kaldırıldığına daha ayrıntılı bir göz atalım:

![](linked-list-allocator-remove-region.svg)

Adım 0, herhangi bir işaretçi ayarlamasından önceki durumu gösterir. `region` ve `current` bölgeleri ile `region.next` ve `current.next` işaretçileri grafikte işaretlenmiştir. Adım 1'de, hem `region.next` hem de `current.next` işaretçileri [`Option::take`] metodu kullanılarak `None`'a sıfırlanır. Orijinal işaretçiler `next` ve `ret` adlı yerel değişkenlerde saklanır.

Adım 2'de, `current.next` işaretçisi, orijinal `region.next` işaretçisi olan yerel `next` işaretçisine ayarlanır. Etkisi, `current`'in artık doğrudan `region`'dan sonraki bölgeye işaret etmesidir; böylece `region` artık bağlı listenin bir elemanı değildir. Fonksiyon daha sonra yerel `ret` değişkeninde saklanan `region`'a işaretçiyi döndürür.

##### `alloc_from_region` Fonksiyonu

`alloc_from_region` fonksiyonu, bir bölgenin verilen bir boyut ve hizalamaya sahip bir ayırma için uygun olup olmadığını döndürür. Şu şekilde tanımlanır:

```rust
// src/allocator/linked_list.rs içinde

impl LinkedListAllocator {
    /// Verilen bölgeyi verilen boyut ve hizalamaya sahip bir ayırma için
    /// kullanmayı dener.
    ///
    /// Başarı durumunda ayırma başlangıç adresini döndürür.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
        -> Result<usize, ()>
    {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            // bölge çok küçük
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            // bölgenin geri kalanı bir ListNode tutamayacak kadar küçük (gerekli,
            // çünkü ayırma bölgeyi kullanılan ve boş bir parçaya böler)
            return Err(());
        }

        // bölge ayırma için uygun
        Ok(alloc_start)
    }
}
```

İlk olarak, fonksiyon daha önce tanımladığımız `align_up` fonksiyonunu ve [`checked_add`] metodunu kullanarak potansiyel bir ayırmanın başlangıç ve bitiş adresini hesaplar. Bir taşma meydana gelirse veya bitiş adresi bölgenin bitiş adresinin gerisindeyse, ayırma bölgeye sığmaz ve bir hata döndürürüz.

Fonksiyon bundan sonra daha az bariz bir kontrol gerçekleştirir. Bu kontrol gereklidir, çünkü çoğu zaman bir ayırma uygun bir bölgeye mükemmel şekilde sığmaz, bu yüzden bölgenin bir kısmı ayırmadan sonra kullanılabilir kalır. Bölgenin bu kısmı ayırmadan sonra kendi `ListNode`'unu saklamalıdır, bu yüzden bunu yapacak kadar büyük olmalıdır. Kontrol tam olarak bunu doğrular: ya ayırma mükemmel şekilde sığar (`excess_size == 0`) ya da fazla boyut bir `ListNode` saklayacak kadar büyüktür.

#### `GlobalAlloc`'u Uygulamak

`add_free_region` ve `find_region` metotlarının sağladığı temel işlemlerle, artık nihayet `GlobalAlloc` trait'ini uygulayabiliriz. Bump allocator'da olduğu gibi, trait'i doğrudan `LinkedListAllocator` için değil, yalnızca sarmalanmış bir `Locked<LinkedListAllocator>` için uyguluyoruz. [`Locked` sarmalayıcısı][`Locked` wrapper], bir spinlock aracılığıyla iç değiştirilebilirlik ekler; bu da `alloc` ve `dealloc` metotları yalnızca `&self` referansları alsa bile allocator örneğini değiştirmemize olanak tanır.

[`Locked` wrapper]: @/edition-2/posts/11-allocator-designs/index.tr.md#a-locked-wrapper-type

Uygulama şöyle görünür:

```rust
// src/allocator/linked_list.rs içinde

use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // layout ayarlamalarını gerçekleştir
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                unsafe {
                    allocator.add_free_region(alloc_end, excess_size);
                }
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // layout ayarlamalarını gerçekleştir
        let (size, _) = LinkedListAllocator::size_align(layout);

        unsafe { self.lock().add_free_region(ptr as usize, size) }
    }
}
```

Daha basit olduğu için `dealloc` metoduyla başlayalım: İlk olarak, birazdan açıklayacağımız bazı layout ayarlamaları gerçekleştirir. Ardından, [`Locked` sarmalayıcısı][`Locked` wrapper] üzerinde [`Mutex::lock`] fonksiyonunu çağırarak bir `&mut LinkedListAllocator` referansı alır. Son olarak, deallocate edilen bölgeyi free list'e eklemek için `add_free_region` fonksiyonunu çağırır.

`alloc` metodu biraz daha karmaşıktır. Aynı layout ayarlamalarıyla başlar ve değiştirilebilir bir allocator referansı almak için [`Mutex::lock`] fonksiyonunu da çağırır. Ardından ayırma için uygun bir bellek bölgesi bulmak ve onu listeden kaldırmak için `find_region` metodunu kullanır. Bu başarılı olmazsa ve `None` döndürülürse, uygun bir bellek bölgesi olmadığı için bir hatayı bildirmek üzere `null_mut` döndürür.

Başarı durumunda, `find_region` metodu uygun bölgenin (artık listede değil) ve ayırmanın başlangıç adresinin bir tuple'ını döndürür. `alloc_start`'ı, ayırma boyutunu ve bölgenin bitiş adresini kullanarak, ayırmanın bitiş adresini ve fazla boyutu yeniden hesaplar. Fazla boyut sıfır değilse, bellek bölgesinin fazla boyutunu free list'e geri eklemek için `add_free_region` çağırır. Son olarak, bir `*mut u8` işaretçisi olarak dönüştürülmüş `alloc_start` adresini döndürür.

#### Layout Ayarlamaları

Peki hem `alloc` hem de `dealloc`'un başında yaptığımız bu layout ayarlamaları nedir? Bunlar, ayrılan her bloğun bir `ListNode` saklayabilir olmasını sağlar. Bu önemlidir, çünkü bellek bloğu bir noktada deallocate edilecek; o noktada ona bir `ListNode` yazmak isteriz. Blok bir `ListNode`'dan küçükse veya doğru hizalamaya sahip değilse, tanımsız davranış meydana gelebilir.

Layout ayarlamaları, şu şekilde tanımlanan `size_align` fonksiyonu tarafından gerçekleştirilir:

```rust
// src/allocator/linked_list.rs içinde

impl LinkedListAllocator {
    /// Verilen layout'u, elde edilen ayrılmış bellek bölgesi bir `ListNode`
    /// saklayabilecek şekilde de ayarlar.
    ///
    /// Ayarlanan boyut ve hizalamayı bir (size, align) tuple'ı olarak döndürür.
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}
```

İlk olarak, fonksiyon geçirilen [`Layout`] üzerinde [`align_to`] metodunu kullanarak, gerekirse hizalamayı bir `ListNode`'un hizalamasına yükseltir. Ardından, bir sonraki bellek bloğunun başlangıç adresinin de bir `ListNode` saklamak için doğru hizalamaya sahip olmasını sağlamak amacıyla boyutu hizalamanın bir katına yuvarlamak için [`pad_to_align`] metodunu kullanır.
İkinci adımda, `mem::size_of::<ListNode>` minimum ayırma boyutunu zorunlu kılmak için [`max`] metodunu kullanır. Bu sayede, `dealloc` fonksiyonu serbest bırakılan bellek bloğuna güvenle bir `ListNode` yazabilir.

[`align_to`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align_to
[`pad_to_align`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.pad_to_align
[`max`]: https://doc.rust-lang.org/std/cmp/trait.Ord.html#method.max

### Kullanmak

Artık `allocator` modülündeki `ALLOCATOR` static'ini yeni `LinkedListAllocator`'ımızı kullanacak şekilde güncelleyebiliriz:

```rust
// src/allocator.rs içinde

use linked_list::LinkedListAllocator;

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> =
    Locked::new(LinkedListAllocator::new());
```

`init` fonksiyonu bump ve bağlı liste allocator'ları için aynı şekilde davrandığından, `init_heap`'teki `init` çağrısını değiştirmemize gerek yok.

`heap_allocation` testlerimizi şimdi tekrar çalıştırdığımızda, bump allocator ile başarısız olan `many_boxes_long_lived` testi dahil tüm testlerin artık geçtiğini görüyoruz:

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

Bu, bağlı liste allocator'ımızın serbest bırakılan belleği sonraki ayırmalar için yeniden kullanabildiğini gösterir.

### Tartışma

Bump allocator'ın aksine, bağlı liste allocator'ı genel amaçlı bir allocator olarak çok daha uygundur; esas olarak serbest bırakılan belleği doğrudan yeniden kullanabildiği için. Ancak bazı dezavantajları da vardır. Bazıları yalnızca temel uygulamamızdan kaynaklanır, ancak allocator tasarımının kendisinin de temel dezavantajları vardır.

#### Serbest Bırakılan Blokları Birleştirmek {#merging-freed-blocks}

Uygulamamızın ana sorunu, heap'i yalnızca daha küçük bloklara bölmesi ama onları asla tekrar birleştirmemesidir. Şu örneği düşünün:

![](linked-list-allocator-fragmentation-on-dealloc.svg)

İlk satırda, heap'te üç ayırma oluşturulur. Bunlardan ikisi satır 2'de tekrar serbest bırakılır ve üçüncüsü satır 3'te serbest bırakılır. Artık tüm heap tekrar kullanılmamıştır, ancak hâlâ dört ayrı bloğa bölünmüştür. Bu noktada, dört bloğun hiçbiri yeterince büyük olmadığı için büyük bir ayırma artık mümkün olmayabilir. Zamanla süreç devam eder ve heap giderek daha küçük bloklara bölünür. Bir noktada, heap o kadar parçalanır ki normal boyutlu ayırmalar bile başarısız olur.

Bu sorunu düzeltmek için, bitişik serbest bırakılan blokları tekrar birleştirmemiz gerekir. Yukarıdaki örnek için bu, şu anlama gelirdi:

![](linked-list-allocator-merge-on-dealloc.svg)

Önceki gibi, üç ayırmanın ikisi satır `2`'de serbest bırakılır. Parçalanmış heap'i tutmak yerine, artık en sağdaki iki bloğu tekrar birleştirmek için satır `2a`'da ek bir adım gerçekleştiriyoruz. Satır `3`'te, üçüncü ayırma (önceki gibi) serbest bırakılır ve üç ayrı blokla temsil edilen tamamen kullanılmayan bir heap ile sonuçlanır. Satır `3a`'daki ek bir birleştirme adımında, ardından üç bitişik bloğu tekrar birleştiririz.

`linked_list_allocator` crate'i bu birleştirme stratejisini şu şekilde uygular: `deallocate`'te serbest bırakılan bellek bloklarını bağlı listenin başına eklemek yerine, listeyi her zaman başlangıç adresine göre sıralı tutar. Bu sayede, birleştirme doğrudan `deallocate` çağrısında, listedeki iki komşu bloğun adreslerini ve boyutlarını inceleyerek gerçekleştirilebilir. Tabii ki, deallocation işlemi bu şekilde daha yavaştır, ancak yukarıda gördüğümüz heap parçalanmasını önler.

#### Performans

Yukarıda öğrendiğimiz gibi, bump allocator son derece hızlıdır ve yalnızca birkaç assembly işlemine optimize edilebilir. Bağlı liste allocator'ı bu kategoride çok daha kötü performans gösterir. Sorun, bir ayırma isteğinin uygun bir blok bulana kadar tüm bağlı liste üzerinde dolaşması gerekebilmesidir.

Liste uzunluğu kullanılmayan bellek bloklarının sayısına bağlı olduğundan, performans farklı programlar için aşırı değişebilir. Yalnızca birkaç ayırma oluşturan bir program nispeten hızlı ayırma performansı yaşar. Ancak heap'i çok sayıda ayırmayla parçalayan bir program için, ayırma performansı çok kötü olacaktır; çünkü bağlı liste çok uzun olacak ve çoğunlukla çok küçük bloklar içerecektir.

Bu performans sorununun temel uygulamamızdan kaynaklanan bir sorun değil, bağlı liste yaklaşımının temel bir sorunu olduğunu belirtmekte fayda var. Ayırma performansı kernel seviyesindeki kod için çok önemli olabileceğinden, aşağıda azalan bellek kullanımı karşılığında geliştirilmiş performans sunan üçüncü bir allocator tasarımını inceliyoruz.

## Sabit Boyutlu Blok Allocator'ı {#fixed-size-block-allocator}

Aşağıda, ayırma isteklerini karşılamak için sabit boyutlu bellek blokları kullanan bir allocator tasarımı sunuyoruz. Bu sayede, allocator genellikle ayırmalar için gerekenden daha büyük bloklar döndürür; bu da [iç parçalanma (internal fragmentation)][internal fragmentation] nedeniyle israf edilen bellekle sonuçlanır. Öte yandan, (bağlı liste allocator'ına kıyasla) uygun bir blok bulmak için gereken süreyi büyük ölçüde azaltır ve çok daha iyi ayırma performansıyla sonuçlanır.

### Giriş

Bir _sabit boyutlu blok allocator'ının_ arkasındaki fikir şudur: İstenen kadar tam olarak bellek ayırmak yerine, az sayıda blok boyutu tanımlarız ve her ayırmayı bir sonraki blok boyutuna yuvarlarız. Örneğin, 16, 64 ve 512 baytlık blok boyutlarıyla, 4 baytlık bir ayırma 16 baytlık bir blok, 48 baytlık bir ayırma 64 baytlık bir blok ve 128 baytlık bir ayırma 512 baytlık bir blok döndürürdü.

Bağlı liste allocator'ı gibi, kullanılmayan bellekte bir bağlı liste oluşturarak kullanılmayan belleği takip ederiz. Ancak, farklı blok boyutlarına sahip tek bir liste kullanmak yerine, her boyut sınıfı için ayrı bir liste oluştururuz. Her liste daha sonra yalnızca tek bir boyuttaki blokları saklar. Örneğin, 16, 64 ve 512 blok boyutlarıyla, bellekte üç ayrı bağlı liste olurdu:

![](fixed-size-block-example.svg).

Tek bir `head` işaretçisi yerine, her biri karşılık gelen boyuttaki ilk kullanılmayan bloğa işaret eden `head_16`, `head_64` ve `head_512` olmak üzere üç head işaretçisine sahibiz. Tek bir listedeki tüm düğümler aynı boyuttadır. Örneğin, `head_16` işaretçisiyle başlatılan liste yalnızca 16 baytlık bloklar içerir. Bu, boyutu zaten head işaretçisinin adıyla belirtildiği için artık her liste düğümünde boyutu saklamamıza gerek olmadığı anlamına gelir.

Bir listedeki her eleman aynı boyutta olduğundan, her liste elemanı bir ayırma isteği için eşit derecede uygundur. Bu, aşağıdaki adımları kullanarak bir ayırmayı çok verimli bir şekilde gerçekleştirebileceğimiz anlamına gelir:

- İstenen ayırma boyutunu bir sonraki blok boyutuna yuvarla. Örneğin, 12 baytlık bir ayırma istendiğinde, yukarıdaki örnekte 16 blok boyutunu seçerdik.
- Liste için head işaretçisini al; örneğin blok boyutu 16 için `head_16`'yı kullanmamız gerekir.
- İlk bloğu listeden kaldır ve onu döndür.

En önemlisi, her zaman listenin ilk elemanını döndürebiliriz ve artık tüm liste üzerinde dolaşmamıza gerek yoktur. Böylece, ayırmalar bağlı liste allocator'ına göre çok daha hızlıdır.

#### Blok Boyutları ve İsraf Edilen Bellek

Blok boyutlarına bağlı olarak, yuvarlama yaparak çok fazla bellek kaybederiz. Örneğin, 128 baytlık bir ayırma için 512 baytlık bir blok döndürüldüğünde, ayrılan belleğin dörtte üçü kullanılmaz. Makul blok boyutları tanımlayarak, israf edilen bellek miktarını bir dereceye kadar sınırlamak mümkündür. Örneğin, blok boyutu olarak 2'nin kuvvetlerini (4, 8, 16, 32, 64, 128, …) kullandığımızda, bellek israfını en kötü durumda ayırma boyutunun yarısıyla ve ortalama durumda ayırma boyutunun dörtte biriyle sınırlayabiliriz.

Blok boyutlarını bir programdaki yaygın ayırma boyutlarına göre optimize etmek de yaygındır. Örneğin, sık sık 24 baytlık ayırmalar yapan programlar için bellek kullanımını iyileştirmek amacıyla buna ek olarak 24 blok boyutunu ekleyebiliriz. Bu sayede, israf edilen bellek miktarı genellikle performans avantajları kaybedilmeden azaltılabilir.

#### Deallocation (Serbest Bırakma)

Tıpkı ayırma gibi, deallocation da çok performanslıdır. Aşağıdaki adımları içerir:

- Serbest bırakılan ayırma boyutunu bir sonraki blok boyutuna yuvarla. Bu gereklidir, çünkü derleyici `dealloc`'a yalnızca istenen ayırma boyutunu geçirir, `alloc` tarafından döndürülen bloğun boyutunu değil. Hem `alloc` hem de `dealloc`'ta aynı boyut ayarlama fonksiyonunu kullanarak, her zaman doğru miktarda belleği serbest bıraktığımızdan emin olabiliriz.
- Liste için head işaretçisini al.
- Head işaretçisini güncelleyerek serbest bırakılan bloğu listenin başına ekle.

En önemlisi, deallocation için de listede dolaşmaya gerek yoktur. Bu, bir `dealloc` çağrısı için gereken sürenin liste uzunluğundan bağımsız olarak aynı kaldığı anlamına gelir.

#### Yedek (Fallback) Allocator

Büyük ayırmaların (>2&nbsp;KB) genellikle, özellikle işletim sistemi kernel'lerinde nadir olduğu göz önüne alındığında, bu ayırmalar için farklı bir allocator'a geri dönmek (fall back) mantıklı olabilir. Örneğin, bellek israfını azaltmak için 2048 bayttan büyük ayırmalar için bir bağlı liste allocator'ına geri dönebiliriz. O boyutta yalnızca çok az ayırma beklendiğinden, bağlı liste küçük kalır ve (de)allocation'lar yine de makul ölçüde hızlı olur.

#### Yeni Bloklar Oluşturmak {#creating-new-blocks}

Yukarıda, her zaman tüm ayırma isteklerini karşılamak için listede belirli bir boyutta yeterli blok olduğunu varsaydık. Ancak bir noktada, belirli bir blok boyutu için bağlı liste boşalır. Bu noktada, bir ayırma isteğini karşılamak için belirli bir boyutta yeni kullanılmayan bloklar oluşturmanın iki yolu vardır:

- Yedek allocator'dan yeni bir blok ayır (eğer varsa).
- Farklı bir listeden daha büyük bir bloğu böl. Bu, en iyi blok boyutları 2'nin kuvvetleriyse işe yarar. Örneğin, 32 baytlık bir blok iki adet 16 baytlık bloğa bölünebilir.

Uygulamamız için, uygulaması çok daha basit olduğundan yeni blokları yedek allocator'dan ayıracağız.

### Uygulama

Artık bir sabit boyutlu blok allocator'ının nasıl çalıştığını bildiğimize göre, uygulamamıza başlayabiliriz. Önceki bölümde oluşturulan bağlı liste allocator'ının uygulamasına bağımlı olmayacağız, bu yüzden bağlı liste allocator'ı uygulamasını atladıysanız bile bu kısmı takip edebilirsiniz.

#### Liste Düğümü (List Node)

Uygulamamıza yeni bir `allocator::fixed_size_block` modülünde bir `ListNode` tipi oluşturarak başlıyoruz:

```rust
// src/allocator.rs içinde

pub mod fixed_size_block;
```

```rust
// src/allocator/fixed_size_block.rs içinde

struct ListNode {
    next: Option<&'static mut ListNode>,
}
```

Bu tip, [bağlı liste allocator'ı uygulamamızdaki][linked list allocator implementation] `ListNode` tipine benzer; farkı, bir `size` alanımız olmamasıdır. Sabit boyutlu blok allocator tasarımıyla bir listedeki her blok aynı boyutta olduğu için ona ihtiyaç yoktur.

[linked list allocator implementation]: #the-allocator-type

#### Blok Boyutları

Sonra, uygulamamız için kullanılan blok boyutlarıyla sabit bir `BLOCK_SIZES` dilimi tanımlıyoruz:

```rust
// src/allocator/fixed_size_block.rs içinde

/// Kullanılacak blok boyutları.
///
/// Boyutların her biri 2'nin bir kuvveti olmalıdır, çünkü aynı zamanda blok
/// hizalaması olarak da kullanılırlar (hizalamalar her zaman 2'nin kuvveti olmalıdır).
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];
```

Blok boyutları olarak, 8'den 2048'e kadar 2'nin kuvvetlerini kullanıyoruz. 8'den küçük hiçbir blok boyutu tanımlamıyoruz, çünkü her blok serbest bırakıldığında bir sonraki bloğa 64-bit bir işaretçi saklayabilir olmalıdır. 2048 bayttan büyük ayırmalar için bir bağlı liste allocator'ına geri döneceğiz.

Uygulamayı basitleştirmek için, bir bloğun boyutunu bellekteki gerekli hizalaması olarak tanımlıyoruz. Yani 16 baytlık bir blok her zaman 16 baytlık bir sınırda hizalanır ve 512 baytlık bir blok 512 baytlık bir sınırda hizalanır. Hizalamaların her zaman 2'nin kuvveti olması gerektiğinden, bu diğer tüm blok boyutlarını dışlar. Gelecekte 2'nin kuvveti olmayan blok boyutlarına ihtiyaç duyarsak, uygulamamızı bunun için yine de ayarlayabiliriz (örneğin ikinci bir `BLOCK_ALIGNMENTS` dizisi tanımlayarak).

#### Allocator Tipi {#the-allocator-type-2}

`ListNode` tipini ve `BLOCK_SIZES` dilimini kullanarak, artık allocator tipimizi tanımlayabiliriz:

```rust
// src/allocator/fixed_size_block.rs içinde

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}
```

`list_heads` alanı, her blok boyutu için bir tane olmak üzere `head` işaretçilerinden oluşan bir dizidir. Bu, dizi uzunluğu olarak `BLOCK_SIZES` diliminin `len()`'i kullanılarak uygulanır. En büyük blok boyutundan daha büyük ayırmalar için bir yedek allocator olarak, `linked_list_allocator`'ın sağladığı allocator'ı kullanırız. Bunun yerine kendi uyguladığımız `LinkedListAllocator`'ı da kullanabilirdik, ancak onun [serbest bırakılan blokları birleştirmeme][merge freed blocks] dezavantajı vardır.

[merge freed blocks]: #merging-freed-blocks

Bir `FixedSizeBlockAllocator` oluşturmak için, diğer allocator tipleri için de uyguladığımız aynı `new` ve `init` fonksiyonlarını sağlıyoruz:

```rust
// src/allocator/fixed_size_block.rs içinde

impl FixedSizeBlockAllocator {
    /// Boş bir FixedSizeBlockAllocator oluşturur.
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Allocator'ı verilen heap sınırlarıyla başlatır.
    ///
    /// Bu fonksiyon unsafe'tir, çünkü çağıranın verilen heap sınırlarının
    /// geçerli olduğunu ve heap'in kullanılmadığını garanti etmesi gerekir.
    /// Bu metot yalnızca bir kez çağrılmalıdır.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe { self.fallback_allocator.init(heap_start, heap_size); }
    }
}
```

`new` fonksiyonu yalnızca `list_heads` dizisini boş düğümlerle başlatır ve `fallback_allocator` olarak [boş][`empty`] bir bağlı liste allocator'ı oluşturur. `EMPTY` sabiti, Rust derleyicisine diziyi sabit bir değerle başlatmak istediğimizi söylemek için gereklidir. Diziyi doğrudan `[None; BLOCK_SIZES.len()]` olarak başlatmak çalışmaz, çünkü o zaman derleyici `Option<&'static mut ListNode>`'un `Copy` trait'ini uygulamasını gerektirir; ki uygulamaz. Bu, Rust derleyicisinin gelecekte ortadan kalkabilecek mevcut bir sınırlamasıdır.

[`empty`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.empty

Unsafe `init` fonksiyonu, `list_heads` dizisinin herhangi bir ek başlatmasını yapmadan yalnızca `fallback_allocator`'ın [`init`] fonksiyonunu çağırır. Bunun yerine, listeleri `alloc` ve `dealloc` çağrılarında tembelce (lazily) başlatacağız.

[`init`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.init

Kolaylık için, `fallback_allocator`'ı kullanarak ayırma yapan özel bir `fallback_alloc` metodu da oluşturuyoruz:

```rust
// src/allocator/fixed_size_block.rs içinde

use alloc::alloc::Layout;
use core::ptr;

impl FixedSizeBlockAllocator {
    /// Yedek allocator'ı kullanarak ayırır.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}
```

`linked_list_allocator` crate'inin [`Heap`] tipi [`GlobalAlloc`]'u uygulamaz ([kilitleme olmadan mümkün olmadığı][not possible without locking] için). Bunun yerine, biraz farklı bir arayüze sahip bir [`allocate_first_fit`] metodu sağlar. Bir `*mut u8` döndürmek ve bir hatayı bildirmek için null bir işaretçi kullanmak yerine, bir `Result<NonNull<u8>, ()>` döndürür. [`NonNull`] tipi, null bir işaretçi olmayacağı garanti edilen bir ham işaretçi için bir soyutlamadır. `Ok` durumunu [`NonNull::as_ptr`] metoduna ve `Err` durumunu null bir işaretçiye eşleyerek, bunu kolayca bir `*mut u8` tipine geri çevirebiliriz.

[`Heap`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html
[not possible without locking]: #globalalloc-and-mutability
[`allocate_first_fit`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.allocate_first_fit
[`NonNull`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html
[`NonNull::as_ptr`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html#method.as_ptr

#### Liste İndeksini Hesaplamak

`GlobalAlloc` trait'ini uygulamadan önce, verilen bir [`Layout`] için mümkün olan en düşük blok boyutunu döndüren bir `list_index` yardımcı fonksiyonu tanımlıyoruz:

```rust
// src/allocator/fixed_size_block.rs içinde

/// Verilen layout için uygun bir blok boyutu seç.
///
/// `BLOCK_SIZES` dizisine bir indeks döndürür.
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}
```

Blok, en az verilen `Layout` tarafından gereken boyut ve hizalamaya sahip olmalıdır. Blok boyutunun aynı zamanda onun hizalaması olduğunu tanımladığımız için, bu, `required_block_size`'ın layout'un [`size()`] ve [`align()`] özniteliklerinin [maksimumu][maximum] olduğu anlamına gelir. `BLOCK_SIZES` dilimindeki bir sonraki daha büyük bloğu bulmak için, önce bir iterator almak üzere [`iter()`] metodunu, ardından en az `required_block_size` kadar büyük olan ilk bloğun indeksini bulmak için [`position()`] metodunu kullanıyoruz.

[maximum]: https://doc.rust-lang.org/core/cmp/trait.Ord.html#method.max
[`size()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.size
[`align()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align
[`iter()`]: https://doc.rust-lang.org/std/primitive.slice.html#method.iter
[`position()`]:  https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.position

Blok boyutunun kendisini değil, `BLOCK_SIZES` dilimine indeksi döndürdüğümüze dikkat edin. Bunun nedeni, döndürülen indeksi `list_heads` dizisine bir indeks olarak kullanmak istememizdir.

#### `GlobalAlloc`'u Uygulamak

Son adım, `GlobalAlloc` trait'ini uygulamaktır:

```rust
// src/allocator/fixed_size_block.rs içinde

use super::Locked;
use alloc::alloc::GlobalAlloc;

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        todo!();
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        todo!();
    }
}
```

Diğer allocator'larda olduğu gibi, `GlobalAlloc` trait'ini doğrudan allocator tipimiz için uygulamıyoruz, senkronize iç değiştirilebilirlik eklemek için [`Locked` sarmalayıcısını][`Locked` wrapper] kullanıyoruz. `alloc` ve `dealloc` uygulamaları nispeten büyük olduğundan, onları aşağıda tek tek tanıtıyoruz.

##### `alloc`

`alloc` metodunun uygulaması şöyle görünür:

```rust
// src/allocator/fixed_size_block.rs'teki `impl` bloğunda

unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            match allocator.list_heads[index].take() {
                Some(node) => {
                    allocator.list_heads[index] = node.next.take();
                    node as *mut ListNode as *mut u8
                }
                None => {
                    // listede blok yok => yeni blok ayır
                    let block_size = BLOCK_SIZES[index];
                    // yalnızca tüm blok boyutları 2'nin kuvvetiyse çalışır
                    let block_align = block_size;
                    let layout = Layout::from_size_align(block_size, block_align)
                        .unwrap();
                    allocator.fallback_alloc(layout)
                }
            }
        }
        None => allocator.fallback_alloc(layout),
    }
}
```

Adım adım gidelim:

İlk olarak, sarmalanan allocator örneğine değiştirilebilir bir referans almak için `Locked::lock` metodunu kullanıyoruz. Sonra, verilen layout için uygun blok boyutunu hesaplamak ve `list_heads` dizisine karşılık gelen indeksi almak için az önce tanımladığımız `list_index` fonksiyonunu çağırıyoruz. Bu indeks `None` ise, ayırma için uygun bir blok boyutu yoktur, bu yüzden `fallback_alloc` fonksiyonunu kullanarak `fallback_allocator`'ı kullanıyoruz.

Liste indeksi `Some` ise, [`Option::take`] metodunu kullanarak `list_heads[index]` ile başlatılan karşılık gelen listedeki ilk düğümü kaldırmaya çalışıyoruz. Liste boş değilse, `match` ifadesinin `Some(node)` dalına gireriz; burada listenin head işaretçisini pop'lanan `node`'un ardılına yönlendiririz (yine [`take`][`Option::take`] kullanarak). Son olarak, pop'lanan `node` işaretçisini bir `*mut u8` olarak döndürürüz.

[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

Liste head'i `None` ise, bu, blok listesinin boş olduğunu gösterir. Bu, [yukarıda açıklandığı gibi](#creating-new-blocks) yeni bir blok oluşturmamız gerektiği anlamına gelir. Bunun için, önce `BLOCK_SIZES` diliminden mevcut blok boyutunu alıyor ve onu yeni blok için hem boyut hem de hizalama olarak kullanıyoruz. Sonra ondan yeni bir `Layout` oluşturuyor ve ayırmayı gerçekleştirmek için `fallback_alloc` metodunu çağırıyoruz. Layout ve hizalamayı ayarlamanın nedeni, bloğun deallocation'da blok listesine ekleneceği olmasıdır.

#### `dealloc`

`dealloc` metodunun uygulaması şöyle görünür:

```rust
// src/allocator/fixed_size_block.rs içinde

use core::{mem, ptr::NonNull};

// `unsafe impl GlobalAlloc` bloğunun içinde

unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            let new_node = ListNode {
                next: allocator.list_heads[index].take(),
            };
            // bloğun düğümü saklamak için gereken boyut ve hizalamaya sahip olduğunu doğrula
            assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
            assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
            let new_node_ptr = ptr as *mut ListNode;
            unsafe {
                new_node_ptr.write(new_node);
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
        }
        None => {
            let ptr = NonNull::new(ptr).unwrap();
            unsafe {
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}
```

`alloc`'ta olduğu gibi, önce değiştirilebilir bir allocator referansı almak için `lock` metodunu, ardından verilen `Layout`'a karşılık gelen blok listesini almak için `list_index` fonksiyonunu kullanıyoruz. İndeks `None` ise, `BLOCK_SIZES`'ta uygun bir blok boyutu yoktur; bu da ayırmanın yedek allocator tarafından oluşturulduğunu gösterir. Bu nedenle, belleği tekrar serbest bırakmak için onun [`deallocate`][`Heap::deallocate`]'ini kullanıyoruz. Metot bir `*mut u8` yerine bir [`NonNull`] bekler, bu yüzden önce işaretçiyi dönüştürmemiz gerekir. (`unwrap` çağrısı yalnızca işaretçi null olduğunda başarısız olur; ki bu, derleyici `dealloc`'u çağırdığında asla olmamalıdır.)

[`Heap::deallocate`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.deallocate

`list_index` bir blok indeksi döndürürse, serbest bırakılan bellek bloğunu listeye eklememiz gerekir. Bunun için, önce mevcut liste head'ine işaret eden yeni bir `ListNode` oluşturuyoruz (yine [`Option::take`] kullanarak). Yeni düğümü serbest bırakılan bellek bloğuna yazmadan önce, `index` tarafından belirtilen mevcut blok boyutunun bir `ListNode` saklamak için gereken boyut ve hizalamaya sahip olduğunu doğrularız. Ardından verilen `*mut u8` işaretçisini bir `*mut ListNode` işaretçisine dönüştürerek ve sonra onun üzerinde unsafe [`write`][`pointer::write`] metodunu çağırarak yazmayı gerçekleştiririz. Son adım, üzerinde `take` çağırdığımız için şu anda `None` olan listenin head işaretçisini, yeni yazdığımız `ListNode`'a ayarlamaktır. Bunun için, ham `new_node_ptr`'yi değiştirilebilir bir referansa dönüştürürüz.

[`pointer::write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write

Belirtmeye değer birkaç şey var:

- Bir blok listesinden ayrılan bloklar ile yedek allocator'dan ayrılan blokları ayırt etmiyoruz. Bu, `alloc`'ta oluşturulan yeni blokların `dealloc`'ta blok listesine eklendiği ve böylece o boyuttaki blokların sayısının arttığı anlamına gelir.
- `alloc` metodu, uygulamamızda yeni blokların oluşturulduğu tek yerdir. Bu, başlangıçta boş blok listeleriyle başladığımız ve bu listeleri yalnızca kendi blok boyutlarında ayırmalar yapıldığında tembelce doldurduğumuz anlamına gelir.
- Bazı `unsafe` işlemler gerçekleştirsek de, `alloc` ve `dealloc`'ta `unsafe` bloklara ihtiyacımız yok. Bunun nedeni, Rust'ın şu anda unsafe fonksiyonların tüm gövdesini bir büyük `unsafe` blok olarak ele almasıdır. Açık `unsafe` bloklar kullanmanın, hangi işlemlerin unsafe olduğunun ve hangilerinin olmadığının belli olması avantajı olduğundan, bu davranışı değiştirmek için [önerilen bir RFC](https://github.com/rust-lang/rfcs/pull/2585) var.

### Kullanmak

Yeni `FixedSizeBlockAllocator`'ımızı kullanmak için, `allocator` modülündeki `ALLOCATOR` static'ini güncellememiz gerekir:

```rust
// src/allocator.rs içinde

use fixed_size_block::FixedSizeBlockAllocator;

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(
    FixedSizeBlockAllocator::new());
```

`init` fonksiyonu uyguladığımız tüm allocator'lar için aynı şekilde davrandığından, `init_heap`'teki `init` çağrısını değiştirmemize gerek yok.

`heap_allocation` testlerimizi şimdi tekrar çalıştırdığımızda, tüm testler hâlâ geçmelidir:

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

Yeni allocator'ımız çalışıyor gibi görünüyor!

### Tartışma

Sabit boyutlu blok yaklaşımı bağlı liste yaklaşımından çok daha iyi performansa sahip olsa da, blok boyutu olarak 2'nin kuvvetlerini kullanırken belleğin yarısına kadarını israf eder. Bu ödünleşimin (tradeoff) buna değip değmediği büyük ölçüde uygulama türüne bağlıdır. Performansın kritik olduğu bir işletim sistemi kernel'i için, sabit boyutlu blok yaklaşımı daha iyi bir seçim gibi görünür.

Uygulama tarafında, mevcut uygulamamızda iyileştirebileceğimiz çeşitli şeyler var:

- Yalnızca yedek allocator'ı kullanarak blokları tembelce ayırmak yerine, ilk ayırmaların performansını iyileştirmek için listeleri önceden doldurmak daha iyi olabilir.
- Uygulamayı basitleştirmek için, yalnızca 2'nin kuvveti olan blok boyutlarına izin verdik; böylece onları blok hizalaması olarak da kullanabildik. Hizalamayı farklı bir şekilde saklayarak (veya hesaplayarak), keyfi diğer blok boyutlarına da izin verebiliriz. Bu sayede, israf edilen belleği en aza indirmek için, örneğin yaygın ayırma boyutları için daha fazla blok boyutu ekleyebiliriz.
- Şu anda yalnızca yeni bloklar oluşturuyoruz, ancak onları asla tekrar serbest bırakmıyoruz. Bu, parçalanmaya yol açar ve sonunda büyük ayırmalar için ayırma başarısızlığıyla sonuçlanabilir. Her blok boyutu için maksimum bir liste uzunluğu zorunlu kılmak mantıklı olabilir. Maksimum uzunluğa ulaşıldığında, sonraki deallocation'lar listeye eklenmek yerine yedek allocator kullanılarak serbest bırakılır.
- Bir bağlı liste allocator'ına geri dönmek yerine, 4&nbsp;KiB'tan büyük ayırmalar için özel bir allocator'a sahip olabiliriz. Fikir, sürekli bir sanal bellek bloğunu sürekli olmayan fiziksel frame'lere eşlemek için 4&nbsp;KiB sayfalarla çalışan [paging]'den yararlanmaktır. Bu sayede, kullanılmayan belleğin parçalanması büyük ayırmalar için artık bir sorun olmaz.
- Böyle bir sayfa allocator'ıyla, 4&nbsp;KiB'a kadar blok boyutları eklemek ve bağlı liste allocator'ını tamamen bırakmak mantıklı olabilir. Bunun ana avantajları, azalan parçalanma ve geliştirilmiş performans öngörülebilirliği, yani daha iyi en kötü durum performansı olurdu.

[paging]: @/edition-2/posts/08-paging-introduction/index.tr.md

Yukarıda özetlenen uygulama iyileştirmelerinin yalnızca öneri olduğunu belirtmek önemlidir. İşletim sistemi kernel'lerinde kullanılan allocator'lar tipik olarak kernel'in belirli iş yükü için yüksek düzeyde optimize edilir; ki bu yalnızca kapsamlı profilleme (profiling) ile mümkündür.

### Çeşitlemeler

Sabit boyutlu blok allocator tasarımının pek çok çeşitlemesi de vardır. İki popüler örnek, Linux gibi popüler kernel'lerde de kullanılan _slab allocator_ ve _buddy allocator_'dır. Aşağıda, bu iki tasarıma kısa bir giriş yapıyoruz.

#### Slab Allocator {#slab-allocator}

Bir [slab allocator]'ın arkasındaki fikir, doğrudan kernel'deki seçili tiplere karşılık gelen blok boyutları kullanmaktır. Bu sayede, o tiplerin ayırmaları bir blok boyutuna tam olarak sığar ve hiç bellek israf edilmez. Bazen, performansı daha da iyileştirmek için kullanılmayan bloklarda tip örneklerini önceden başlatmak bile mümkün olabilir.

[slab allocator]: https://en.wikipedia.org/wiki/Slab_allocation

Slab ayırma genellikle diğer allocator'larla birleştirilir. Örneğin, bellek israfını azaltmak amacıyla ayrılmış bir bloğu daha da bölmek için bir sabit boyutlu blok allocator'ıyla birlikte kullanılabilir. Tek bir büyük ayırmanın üzerine bir [nesne havuzu örüntüsü (object pool pattern)][object pool pattern] uygulamak için de sıklıkla kullanılır.

[object pool pattern]: https://en.wikipedia.org/wiki/Object_pool_pattern

#### Buddy Allocator {#buddy-allocator}

Serbest bırakılan blokları yönetmek için bir bağlı liste kullanmak yerine, [buddy allocator] tasarımı 2'nin kuvveti blok boyutlarıyla birlikte bir [ikili ağaç (binary tree)][binary tree] veri yapısı kullanır. Belirli bir boyutta yeni bir blok gerektiğinde, daha büyük boyutlu bir bloğu iki yarıya böler ve böylece ağaçta iki alt düğüm oluşturur. Bir blok tekrar serbest bırakıldığında, ağaçtaki komşu bloğu analiz edilir. Komşu da boşsa, iki blok iki katı boyutta bir blok oluşturmak için tekrar birleştirilir.

Bu birleştirme sürecinin avantajı, [dış parçalanmanın (external fragmentation)][external fragmentation] azaltılması ve böylece küçük serbest bırakılan blokların büyük bir ayırma için yeniden kullanılabilmesidir. Ayrıca bir yedek allocator kullanmaz, bu yüzden performans daha öngörülebilirdir. En büyük dezavantajı, yalnızca 2'nin kuvveti blok boyutlarının mümkün olmasıdır; bu da [iç parçalanma (internal fragmentation)][internal fragmentation] nedeniyle büyük miktarda israf edilen bellekle sonuçlanabilir. Bu nedenle, buddy allocator'lar genellikle ayrılmış bir bloğu birden çok daha küçük bloğa daha da bölmek için bir slab allocator'ıyla birleştirilir.

[buddy allocator]: https://en.wikipedia.org/wiki/Buddy_memory_allocation
[binary tree]: https://en.wikipedia.org/wiki/Binary_tree
[external fragmentation]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#External_fragmentation
[internal fragmentation]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#Internal_fragmentation


## Özet

Bu yazı, farklı allocator tasarımlarına genel bir bakış yaptı. Tek bir `next` işaretçisini artırarak belleği doğrusal olarak teslim eden temel bir [bump allocator]'ın nasıl uygulanacağını öğrendik. Bump ayırma çok hızlı olsa da, belleği yalnızca tüm ayırmalar serbest bırakıldıktan sonra yeniden kullanabilir. Bu nedenle, nadiren global allocator olarak kullanılır.

[bump allocator]: @/edition-2/posts/11-allocator-designs/index.tr.md#bump-allocator

Sonra, bir bağlı liste oluşturmak için serbest bırakılan bellek bloklarının kendilerini kullanan bir [bağlı liste allocator'ı][linked list allocator], yani _free list_ oluşturduk. Bu liste, farklı boyutlarda keyfi sayıda serbest bırakılan bloğu saklamayı mümkün kılar. Hiçbir bellek israfı meydana gelmese de, bir ayırma isteği listenin tam bir taranmasını gerektirebileceği için yaklaşım kötü performanstan muzdariptir. Uygulamamız ayrıca, bitişik serbest bırakılan blokları tekrar birleştirmediği için [dış parçalanmadan][external fragmentation] da muzdariptir.

[linked list allocator]: @/edition-2/posts/11-allocator-designs/index.tr.md#linked-list-allocator
[free list]: https://en.wikipedia.org/wiki/Free_list

Bağlı liste yaklaşımının performans sorunlarını düzeltmek için, sabit bir blok boyutu kümesini önceden tanımlayan bir [sabit boyutlu blok allocator'ı][fixed-size block allocator] oluşturduk. Her blok boyutu için ayrı bir [free list] vardır; böylece ayırmalar ve deallocation'lar yalnızca listenin başına ekleme/çıkarma yapması gerekir ve bu yüzden çok hızlıdır. Her ayırma bir sonraki daha büyük blok boyutuna yuvarlandığından, [iç parçalanma][internal fragmentation] nedeniyle bir miktar bellek israf edilir.

[fixed-size block allocator]: @/edition-2/posts/11-allocator-designs/index.tr.md#fixed-size-block-allocator

Farklı ödünleşimlere sahip çok daha fazla allocator tasarımı var. [Slab ayırma][Slab allocation], yaygın sabit boyutlu yapıların ayrılmasını optimize etmek için iyi çalışır, ancak her durumda uygulanabilir değildir. [Buddy ayırma][Buddy allocation], serbest bırakılan blokları tekrar birleştirmek için bir ikili ağaç kullanır, ancak yalnızca 2'nin kuvveti blok boyutlarını desteklediği için büyük miktarda bellek israf eder. Her kernel uygulamasının benzersiz bir iş yükü olduğunu, bu yüzden tüm durumlara uyan "en iyi" bir allocator tasarımı olmadığını da hatırlamak önemlidir.

[Slab allocation]: @/edition-2/posts/11-allocator-designs/index.tr.md#slab-allocator
[Buddy allocation]: @/edition-2/posts/11-allocator-designs/index.tr.md#buddy-allocator


## Sırada ne var?

Bu yazıyla, şimdilik bellek yönetimi uygulamamızı sonuçlandırıyoruz. Sonra, [_async/await_] biçiminde işbirlikçi çoklu görevle (cooperative multitasking) başlayarak [_çoklu görevi (multitasking)_][_multitasking_] incelemeye başlayacağız. Sonraki yazılarda, ardından [_thread'leri_][_threads_], [_çoklu işlemeyi (multiprocessing)_][_multiprocessing_] ve [_süreçleri (processes)_][_processes_] inceleyeceğiz.

[_multitasking_]: https://en.wikipedia.org/wiki/Computer_multitasking
[_threads_]: https://en.wikipedia.org/wiki/Thread_(computing)
[_processes_]: https://en.wikipedia.org/wiki/Process_(computing)
[_multiprocessing_]: https://en.wikipedia.org/wiki/Multiprocessing
[_async/await_]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html
