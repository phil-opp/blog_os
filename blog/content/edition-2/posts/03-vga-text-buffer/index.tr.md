+++
title = "VGA Metin Modu"
weight = 3
path = "tr/vga-text-mode"
date  = 2018-02-26

[extra]
chapter = "Bare Bones"

# Please update this when updating the translation
translation_based_on_commit = "211f460251cd332905225c93eb66b1aff9f4aefd"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

[VGA metin modu][VGA text mode], ekrana metin yazdırmanın basit bir yoludur. Bu yazıda, tüm güvensizliği (unsafety) ayrı bir modülde kapsülleyerek onun kullanımını güvenli ve basit hale getiren bir arayüz oluşturuyoruz. Ayrıca Rust'ın [biçimlendirme makrolarına][formatting macros] yönelik destek de uyguluyoruz.

[VGA text mode]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode
[formatting macros]: https://doc.rust-lang.org/std/fmt/#related-macros

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-03`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-03

<!-- toc -->

## VGA Metin Arabelleği
VGA metin modunda ekrana bir karakter yazdırmak için, onun VGA donanımının metin arabelleğine yazılması gerekir. VGA metin arabelleği, doğrudan ekrana işlenen, tipik olarak 25 satır ve 80 sütundan oluşan iki boyutlu bir dizidir. Her dizi girdisi, tek bir ekran karakterini aşağıdaki biçim aracılığıyla tanımlar:

| Bit(ler) | Değer            |
| -------- | ---------------- |
| 0-7      | ASCII kod noktası |
| 8-11     | Ön plan rengi    |
| 12-14    | Arka plan rengi  |
| 15       | Yanıp sönme      |

İlk bayt, [ASCII kodlamasıyla][ASCII encoding] yazdırılması gereken karakteri temsil eder. Daha açık olmak gerekirse, bu tam olarak ASCII değildir; bazı ek karakterler ve küçük değişikliklerle [_code page 437_] adlı bir karakter kümesidir. Basitlik için, bu yazıda ona ASCII karakter demeye devam edeceğiz.

[ASCII encoding]: https://en.wikipedia.org/wiki/ASCII
[_code page 437_]: https://en.wikipedia.org/wiki/Code_page_437

İkinci bayt, karakterin nasıl görüntüleneceğini tanımlar. İlk dört bit ön plan rengini, sonraki üç bit arka plan rengini ve son bit ise karakterin yanıp sönüp sönmeyeceğini tanımlar. Aşağıdaki renkler kullanılabilir:

| Sayı | Renk        | Sayı + Parlaklık Biti | Parlak Renk        |
| ---- | ----------- | --------------------- | ------------------ |
| 0x0  | Siyah       | 0x8                   | Koyu Gri           |
| 0x1  | Mavi        | 0x9                   | Açık Mavi          |
| 0x2  | Yeşil       | 0xa                   | Açık Yeşil         |
| 0x3  | Camgöbeği   | 0xb                   | Açık Camgöbeği     |
| 0x4  | Kırmızı     | 0xc                   | Açık Kırmızı       |
| 0x5  | Macenta     | 0xd                   | Pembe              |
| 0x6  | Kahverengi  | 0xe                   | Sarı               |
| 0x7  | Açık Gri    | 0xf                   | Beyaz              |

4. bit, örneğin maviyi açık maviye dönüştüren _parlaklık bitidir (bright bit)_. Arka plan rengi için bu bit, yanıp sönme biti olarak yeniden kullanılır.

VGA metin arabelleğine, `0xb8000` adresine [belleğe eşlenmiş G/Ç (memory-mapped I/O)][memory-mapped I/O] aracılığıyla erişilebilir. Bu, o adrese yapılan okuma ve yazmaların RAM'e erişmediği, doğrudan VGA donanımındaki metin arabelleğine eriştiği anlamına gelir. Yani onu, o adrese yapılan normal bellek işlemleri aracılığıyla okuyup yazabiliriz.

[memory-mapped I/O]: https://en.wikipedia.org/wiki/Memory-mapped_I/O

Belleğe eşlenmiş donanımın tüm normal RAM işlemlerini desteklemeyebileceğini unutmayın. Örneğin, bir cihaz yalnızca bayt bazında okumaları destekleyebilir ve bir `u64` okunduğunda çöp değer döndürebilir. Neyse ki metin arabelleği [normal okuma ve yazmaları destekler][supports normal reads and writes], bu yüzden onu özel bir şekilde ele almamıza gerek yok.

[supports normal reads and writes]: https://web.stanford.edu/class/cs140/projects/pintos/specs/freevga/vga/vgamem.htm#manip

## Bir Rust Modülü
Artık VGA arabelleğinin nasıl çalıştığını bildiğimize göre, yazdırma işlemini yönetmek için bir Rust modülü oluşturabiliriz:

```rust
// src/main.rs içinde
mod vga_buffer;
```

Bu modülün içeriği için yeni bir `src/vga_buffer.rs` dosyası oluşturuyoruz. Aşağıdaki tüm kodlar (aksi belirtilmedikçe) yeni modülümüzün içine girer.

### Renkler
İlk olarak, farklı renkleri bir enum kullanarak temsil ediyoruz:

```rust
// src/vga_buffer.rs içinde

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}
```
Her renk için sayıyı açıkça belirtmek amacıyla burada [C benzeri bir enum][C-like enum] kullanıyoruz. `repr(u8)` özniteliği sayesinde, her enum varyantı bir `u8` olarak saklanır. Aslında 4 bit yeterli olurdu, ancak Rust'ta bir `u4` tipi yok.

[C-like enum]: https://doc.rust-lang.org/rust-by-example/custom_types/enum/c_like.html

Normalde derleyici, kullanılmayan her varyant için bir uyarı verirdi. `#[allow(dead_code)]` özniteliğini kullanarak, `Color` enum'ı için bu uyarıları devre dışı bırakıyoruz.

[`Copy`], [`Clone`], [`Debug`], [`PartialEq`] ve [`Eq`] trait'lerini [türeterek (deriving)][deriving], tip için [copy semantiğini][copy semantics] etkinleştiriyor ve onu yazdırılabilir ve karşılaştırılabilir hale getiriyoruz.

[deriving]: https://doc.rust-lang.org/rust-by-example/trait/derive.html
[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[`Clone`]: https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html
[`Debug`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html
[`PartialEq`]: https://doc.rust-lang.org/nightly/core/cmp/trait.PartialEq.html
[`Eq`]: https://doc.rust-lang.org/nightly/core/cmp/trait.Eq.html
[copy semantics]: https://doc.rust-lang.org/1.30.0/book/first-edition/ownership.html#copy-types

Ön plan ve arka plan rengini belirten tam bir renk kodunu temsil etmek için, `u8`'in üzerine bir [newtype] oluşturuyoruz:

[newtype]: https://doc.rust-lang.org/rust-by-example/generics/new_types.html

```rust
// src/vga_buffer.rs içinde

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}
```
`ColorCode` struct'ı, ön plan ve arka plan rengini içeren tam renk baytını barındırır. Öncekine benzer şekilde, onun için `Copy` ve `Debug` trait'lerini türetiyoruz. `ColorCode`'un tam olarak bir `u8` ile aynı veri yerleşimine sahip olmasını sağlamak için [`repr(transparent)`] özniteliğini kullanıyoruz.

[`repr(transparent)`]: https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent

### Metin Arabelleği
Artık bir ekran karakterini ve metin arabelleğini temsil edecek yapıları ekleyebiliriz:

```rust
// src/vga_buffer.rs içinde

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```
Rust'ta varsayılan struct'larda alan sıralaması tanımsız olduğundan, [`repr(C)`] özniteliğine ihtiyacımız var. Bu öznitelik, struct'ın alanlarının tıpkı bir C struct'ındaki gibi yerleştirilmesini garanti eder ve böylece doğru alan sıralamasını güvence altına alır. `Buffer` struct'ı için, onun tek alanıyla aynı bellek yerleşimine sahip olmasını sağlamak amacıyla yine [`repr(transparent)`] kullanıyoruz.

[`repr(C)`]: https://doc.rust-lang.org/nightly/nomicon/other-reprs.html#reprc

Gerçekten ekrana yazmak için, şimdi bir writer (yazıcı) tipi oluşturuyoruz:

```rust
// src/vga_buffer.rs içinde

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}
```
Writer her zaman son satıra yazacak ve bir satır dolduğunda (veya `\n` durumunda) satırları yukarı kaydıracak. `column_position` alanı, son satırdaki mevcut konumu takip eder. Mevcut ön plan ve arka plan renkleri `color_code` tarafından belirtilir ve VGA arabelleğine bir referans `buffer` içinde saklanır. Burada, referansın ne kadar süre geçerli olduğunu derleyiciye bildirmek için [açık bir ömre (explicit lifetime)][explicit lifetime] ihtiyacımız olduğunu unutmayın. [`'static`] ömrü, referansın tüm program çalışma süresi boyunca geçerli olduğunu belirtir (VGA metin arabelleği için bu doğrudur).

[explicit lifetime]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#lifetime-annotation-syntax
[`'static`]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime

### Yazdırma
Artık arabelleğin karakterlerini değiştirmek için `Writer`'ı kullanabiliriz. İlk olarak, tek bir ASCII baytı yazmak için bir metot oluşturuyoruz:

```rust
// src/vga_buffer.rs içinde

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code,
                };
                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {/* TODO */}
}
```
Bayt, [yeni satır (newline)][newline] baytı `\n` ise, writer hiçbir şey yazdırmaz. Bunun yerine, daha sonra uygulayacağımız bir `new_line` metodunu çağırır. Diğer baytlar ise ikinci `match` durumunda ekrana yazdırılır.

[newline]: https://en.wikipedia.org/wiki/Newline

Bir bayt yazdırırken writer, mevcut satırın dolu olup olmadığını kontrol eder. Bu durumda, satırı kaydırmak için bir `new_line` çağrısı kullanılır. Ardından mevcut konuma arabelleğe yeni bir `ScreenChar` yazar. Son olarak, mevcut sütun konumu ilerletilir.

Tam dizeleri yazdırmak için, onları baytlara dönüştürüp tek tek yazdırabiliriz:

```rust
// src/vga_buffer.rs içinde

impl Writer {
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // yazdırılabilir ASCII baytı veya yeni satır
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // yazdırılabilir ASCII aralığının parçası değil
                _ => self.write_byte(0xfe),
            }

        }
    }
}
```

VGA metin arabelleği yalnızca ASCII'yi ve [code page 437]'nin ek baytlarını destekler. Rust dizeleri varsayılan olarak [UTF-8]'dir, bu yüzden VGA metin arabelleği tarafından desteklenmeyen baytlar içerebilirler. Yazdırılabilir ASCII baytlarını (bir yeni satır ya da boşluk karakteri ile `~` karakteri arasındaki herhangi bir şey) ve yazdırılamaz baytları ayırt etmek için bir `match` kullanıyoruz. Yazdırılamaz baytlar için, VGA donanımında `0xfe` onaltılık koduna sahip olan bir `■` karakteri yazdırıyoruz.

[code page 437]: https://en.wikipedia.org/wiki/Code_page_437
[UTF-8]: https://www.fileformat.info/info/unicode/utf8.htm

#### Deneyin!
Ekrana birkaç karakter yazmak için geçici bir fonksiyon oluşturabilirsiniz:

```rust
// src/vga_buffer.rs içinde

pub fn print_something() {
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };

    writer.write_byte(b'H');
    writer.write_string("ello ");
    writer.write_string("Wörld!");
}
```
Bu fonksiyon önce, `0xb8000` adresindeki VGA arabelleğine işaret eden yeni bir Writer oluşturur. Bunun söz dizimi biraz tuhaf görünebilir: İlk olarak, `0xb8000` tamsayısını değiştirilebilir bir [ham işaretçiye (raw pointer)][raw pointer] dönüştürüyoruz. Ardından onu (`*` aracılığıyla) dereference ederek ve hemen tekrar (`&mut` aracılığıyla) ödünç alarak değiştirilebilir bir referansa dönüştürüyoruz. Derleyici, ham işaretçinin geçerli olduğunu garanti edemediğinden, bu dönüştürme bir [`unsafe` bloğu][`unsafe` block] gerektirir.

[raw pointer]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`unsafe` block]: https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html

Ardından ona `b'H'` baytını yazar. `b` öneki, bir ASCII karakterini temsil eden bir [bayt değişmezi (byte literal)][byte literal] oluşturur. `"ello "` ve `"Wörld!"` dizelerini yazarak, `write_string` metodumuzu ve yazdırılamaz karakterlerin işlenmesini test ediyoruz. Çıktıyı görmek için, `print_something` fonksiyonunu `_start` fonksiyonumuzdan çağırmamız gerekir:

```rust
// src/main.rs içinde

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    vga_buffer::print_something();

    loop {}
}
```

Projemizi şimdi çalıştırdığımızda, ekranın _sol alt_ köşesinde sarı renkte bir `Hello W■■rld!` yazdırılmalı:

[byte literal]: https://doc.rust-lang.org/reference/tokens.html#byte-literals

![ekranın sol alt köşesinde sarı renkli bir `Hello W■■rld!` ile QEMU çıktısı](vga-hello.png)

`ö`'nün iki `■` karakteri olarak yazdırıldığına dikkat edin. Bunun nedeni, `ö`'nün [UTF-8]'de iki baytla temsil edilmesi ve bu baytların ikisinin de yazdırılabilir ASCII aralığına girmemesidir. Aslında bu, UTF-8'in temel bir özelliğidir: çok baytlı değerlerin tek tek baytları asla geçerli ASCII değildir.

### Volatile {#volatile}
Mesajımızın doğru yazdırıldığını az önce gördük. Ancak bu, daha agresif optimizasyon yapan gelecekteki Rust derleyicileriyle çalışmayabilir.

Sorun şu ki, biz yalnızca `Buffer`'a yazıyor ve ondan bir daha hiç okumuyoruz. Derleyici, gerçekten (normal RAM yerine) VGA arabellek belleğine eriştiğimizi bilmiyor ve bazı karakterlerin ekranda görünmesi gibi bir yan etkiden haberi yok. Bu yüzden bu yazma işlemlerinin gereksiz olduğuna ve atlanabileceğine karar verebilir. Bu hatalı optimizasyonu önlemek için, bu yazma işlemlerini _[volatile]_ olarak belirtmemiz gerekir. Bu, derleyiciye yazma işleminin yan etkileri olduğunu ve optimize edilerek kaldırılmaması gerektiğini bildirir.

[volatile]: https://en.wikipedia.org/wiki/Volatile_(computer_programming)

VGA arabelleği için volatile yazmaları kullanmak amacıyla, [volatile][volatile crate] kütüphanesini kullanıyoruz. Bu _crate_ (Rust dünyasında paketler bu şekilde adlandırılır), `read` ve `write` metotlarına sahip bir `Volatile` sarmalayıcı (wrapper) tipi sağlar. Bu metotlar dahili olarak core kütüphanesinin [read_volatile] ve [write_volatile] fonksiyonlarını kullanır ve böylece okuma/yazma işlemlerinin optimize edilerek kaldırılmamasını garanti eder.

[volatile crate]: https://docs.rs/volatile
[read_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.read_volatile.html
[write_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.write_volatile.html

`volatile` crate'ine bir bağımlılık eklemeyi, onu `Cargo.toml` dosyamızın `dependencies` bölümüne ekleyerek yapabiliriz:

```toml
# Cargo.toml içinde

[dependencies]
volatile = "0.2.6"
```

`volatile`'in `0.2.6` sürümünü belirttiğinizden emin olun. Crate'in daha yeni sürümleri bu yazıyla uyumlu değildir.
`0.2.6`, [semantik][semantic] sürüm numarasıdır. Daha fazla bilgi için cargo belgelerinin [Bağımlılıkları Belirtme][Specifying Dependencies] kılavuzuna bakın.

[semantic]: https://semver.org/
[Specifying Dependencies]: https://doc.crates.io/specifying-dependencies.html

Onu, VGA arabelleğine yapılan yazma işlemlerini volatile hale getirmek için kullanalım. `Buffer` tipimizi aşağıdaki gibi güncelliyoruz:

```rust
// src/vga_buffer.rs içinde

use volatile::Volatile;

struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```
Bir `ScreenChar` yerine, artık bir `Volatile<ScreenChar>` kullanıyoruz. (`Volatile` tipi [generic]'tir ve (neredeyse) herhangi bir tipi sarmalayabilir.) Bu, ona yanlışlıkla “normal” bir şekilde yazamayacağımızı garanti eder. Bunun yerine, artık `write` metodunu kullanmamız gerekir.

[generic]: https://doc.rust-lang.org/book/ch10-01-syntax.html

Bu, `Writer::write_byte` metodumuzu güncellememiz gerektiği anlamına gelir:

```rust
// src/vga_buffer.rs içinde

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                ...

                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                ...
            }
        }
    }
    ...
}
```

`=` kullanan tipik bir atama yerine, artık `write` metodunu kullanıyoruz. Artık derleyicinin bu yazma işlemini asla optimize ederek kaldırmayacağını garanti edebiliriz.

### Biçimlendirme Makroları
Rust'ın biçimlendirme makrolarını da desteklemek güzel olurdu. Bu sayede, tamsayılar veya float'lar gibi farklı tipleri kolayca yazdırabiliriz. Onları desteklemek için, [`core::fmt::Write`] trait'ini uygulamamız gerekir. Bu trait'in gerekli tek metodu `write_str`'dir; bu metot, yalnızca `fmt::Result` dönüş tipiyle, `write_string` metodumuza oldukça benzer:

[`core::fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

```rust
// src/vga_buffer.rs içinde

use core::fmt;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
```
`Ok(())`, yalnızca `()` tipini içeren bir `Ok` Result'tur.

Artık Rust'ın yerleşik `write!`/`writeln!` biçimlendirme makrolarını kullanabiliriz:

```rust
// src/vga_buffer.rs içinde

pub fn print_something() {
    use core::fmt::Write;
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };

    writer.write_byte(b'H');
    writer.write_string("ello! ");
    write!(writer, "The numbers are {} and {}", 42, 1.0/3.0).unwrap();
}
```

Şimdi ekranın altında bir `Hello! The numbers are 42 and 0.3333333333333333` görmelisiniz. `write!` çağrısı, kullanılmadığında bir uyarıya neden olan bir `Result` döndürür, bu yüzden onun üzerinde, bir hata oluşursa panic'e neden olan [`unwrap`] fonksiyonunu çağırıyoruz. VGA arabelleğine yapılan yazmalar asla başarısız olmadığı için, bizim durumumuzda bu bir sorun değildir.

[`unwrap`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.unwrap

### Yeni Satırlar
Şu anda, yeni satırları ve artık satıra sığmayan karakterleri yalnızca yok sayıyoruz. Bunun yerine, her karakteri bir satır yukarı taşımak (en üst satır silinir) ve son satırın başından yeniden başlamak istiyoruz. Bunu yapmak için, `Writer`'ın `new_line` metodu için bir uygulama ekliyoruz:

```rust
// src/vga_buffer.rs içinde

impl Writer {
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {/* TODO */}
}
```
Tüm ekran karakterleri üzerinde iterasyon yapıyor ve her karakteri bir satır yukarı taşıyoruz. Aralık gösteriminin (`..`) üst sınırının dışlayıcı (exclusive) olduğuna dikkat edin. Ayrıca 0. satırı atlıyoruz (ilk aralık `1`'den başlıyor), çünkü o, ekranın dışına kaydırılan satırdır.

Yeni satır kodunu tamamlamak için, `clear_row` metodunu ekliyoruz:

```rust
// src/vga_buffer.rs içinde

impl Writer {
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}
```
Bu metot, bir satırın tüm karakterlerinin üzerine bir boşluk karakteri yazarak onu temizler.

## Global Bir Arayüz {#a-global-interface}
Bir `Writer` örneğini etrafta taşımadan diğer modüllerden bir arayüz olarak kullanılabilecek global bir writer sağlamak için, statik bir `WRITER` oluşturmayı deniyoruz:

```rust
// src/vga_buffer.rs içinde

pub static WRITER: Writer = Writer {
    column_position: 0,
    color_code: ColorCode::new(Color::Yellow, Color::Black),
    buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
};
```

Ancak onu şimdi derlemeye çalışırsak, aşağıdaki hatalar oluşur:

```
error[E0015]: calls in statics are limited to constant functions, tuple structs and tuple variants
 --> src/vga_buffer.rs:7:17
  |
7 |     color_code: ColorCode::new(Color::Yellow, Color::Black),
  |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error[E0396]: raw pointers cannot be dereferenced in statics
 --> src/vga_buffer.rs:8:22
  |
8 |     buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  |                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ dereference of raw pointer in constant

error[E0017]: references in statics may only refer to immutable values
 --> src/vga_buffer.rs:8:22
  |
8 |     buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  |                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ statics require immutable values

error[E0017]: references in statics may only refer to immutable values
 --> src/vga_buffer.rs:8:13
  |
8 |     buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ statics require immutable values
```

Burada ne olduğunu anlamak için, statik değerlerin, çalışma zamanında başlatılan normal değişkenlerin aksine derleme zamanında başlatıldığını bilmemiz gerekir. Rust derleyicisinin bu tür başlatma ifadelerini değerlendiren bileşenine “[const evaluator]” denir. İşlevselliği hâlâ sınırlıdır, ancak onu genişletmek için, örneğin “[Allow panicking in constants]” RFC'sinde olduğu gibi devam eden çalışmalar vardır.

[const evaluator]: https://rustc-dev-guide.rust-lang.org/const-eval.html
[Allow panicking in constants]: https://github.com/rust-lang/rfcs/pull/2345

`ColorCode::new` ile ilgili sorun, [`const` fonksiyonları][`const` functions] kullanılarak çözülebilirdi; ancak buradaki temel sorun, Rust'ın const evaluator'ının ham işaretçileri derleme zamanında referanslara dönüştürememesidir. Belki bir gün çalışacak, ama o zamana kadar başka bir çözüm bulmamız gerekiyor.

[`const` functions]: https://doc.rust-lang.org/reference/const_eval.html#const-functions

### Lazy Statics {#lazy-statics}
Statik değerlerin, const olmayan fonksiyonlarla bir kerelik başlatılması Rust'ta yaygın bir sorundur. Neyse ki, [lazy_static] adlı bir crate'te zaten iyi bir çözüm mevcuttur. Bu crate, tembelce (lazily) başlatılan bir `static` tanımlayan bir `lazy_static!` makrosu sağlar. Değerini derleme zamanında hesaplamak yerine, `static` ilk kez erişildiğinde kendisini tembelce başlatır. Böylece başlatma çalışma zamanında gerçekleşir, bu yüzden keyfi olarak karmaşık başlatma kodu mümkündür.

[lazy_static]: https://docs.rs/lazy_static/1.0.1/lazy_static/

`lazy_static` crate'ini projemize ekleyelim:

```toml
# Cargo.toml içinde

[dependencies.lazy_static]
version = "1.0"
features = ["spin_no_std"]
```

Standart kütüphaneyi bağlamadığımız için `spin_no_std` özelliğine ihtiyacımız var.

`lazy_static` ile, statik `WRITER`'ımızı sorunsuzca tanımlayabiliriz:

```rust
// src/vga_buffer.rs içinde

use lazy_static::lazy_static;

lazy_static! {
    pub static ref WRITER: Writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };
}
```

Ancak bu `WRITER` oldukça işe yaramaz, çünkü değiştirilemez (immutable). Bu, ona hiçbir şey yazamayacağımız anlamına gelir (çünkü tüm yazma metotları `&mut self` alır). Olası bir çözüm, bir [değiştirilebilir statik (mutable static)][mutable static] kullanmak olurdu. Ama o zaman ona yapılan her okuma ve yazma unsafe olurdu, çünkü kolayca veri yarışlarına (data races) ve diğer kötü şeylere yol açabilirdi. `static mut` kullanmak kesinlikle önerilmez. Hatta onu [kaldırma][remove static mut] önerileri bile vardı. Peki alternatifler nelerdir? [İç değiştirilebilirlik (interior mutability)][interior mutability] sağlayan [RefCell] ya da hatta [UnsafeCell] gibi bir cell tipiyle değiştirilemez bir statik kullanmayı deneyebilirdik. Ancak bu tipler [Sync] değildir (haklı bir nedenle), bu yüzden onları statik değerlerde kullanamayız.

[mutable static]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable
[remove static mut]: https://internals.rust-lang.org/t/pre-rfc-remove-static-mut/1437
[RefCell]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html#keeping-track-of-borrows-at-runtime-with-refcellt
[UnsafeCell]: https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html
[interior mutability]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
[Sync]: https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html

### Spinlock'lar {#spinlocks}
Senkronize iç değiştirilebilirlik elde etmek için, standart kütüphanenin kullanıcıları [Mutex] kullanabilir. Mutex, kaynak zaten kilitliyken thread'leri bloklayarak karşılıklı dışlama (mutual exclusion) sağlar. Ancak temel kernel'imizin herhangi bir bloklama desteği, hatta bir thread kavramı bile yok, bu yüzden onu da kullanamayız. Ancak bilgisayar biliminde, hiçbir işletim sistemi özelliği gerektirmeyen gerçekten temel bir mutex türü vardır: [spinlock]. Bloklamak yerine, thread'ler sıkı bir döngüde onu tekrar tekrar kilitlemeye çalışır ve böylece mutex tekrar serbest kalana kadar CPU zamanı harcar.

[Mutex]: https://doc.rust-lang.org/nightly/std/sync/struct.Mutex.html
[spinlock]: https://en.wikipedia.org/wiki/Spinlock

Dönen bir mutex (spinning mutex) kullanmak için, bağımlılık olarak [spin crate]'ini ekleyebiliriz:

[spin crate]: https://crates.io/crates/spin

```toml
# Cargo.toml içinde
[dependencies]
spin = "0.5.2"
```

Ardından, statik `WRITER`'ımıza güvenli [iç değiştirilebilirlik][interior mutability] eklemek için dönen mutex'i kullanabiliriz:

```rust
// src/vga_buffer.rs içinde

use spin::Mutex;
...
lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}
```
Artık `print_something` fonksiyonunu silebilir ve doğrudan `_start` fonksiyonumuzdan yazdırabiliriz:

```rust
// src/main.rs içinde
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    write!(vga_buffer::WRITER.lock(), ", some numbers: {} {}", 42, 1.337).unwrap();

    loop {}
}
```
Fonksiyonlarını kullanabilmek için `fmt::Write` trait'ini içe aktarmamız gerekir.

### Güvenlik
Kodumuzda yalnızca tek bir unsafe bloğumuz olduğuna dikkat edin; bu blok, `0xb8000`'e işaret eden bir `Buffer` referansı oluşturmak için gereklidir. Sonrasında tüm işlemler güvenlidir. Rust, dizi erişimleri için varsayılan olarak sınır denetimi (bounds checking) kullanır, bu yüzden yanlışlıkla arabelleğin dışına yazamayız. Böylece, gerekli koşulları tip sistemine kodladık ve dışarıya güvenli bir arayüz sağlayabiliyoruz.

### Bir println Makrosu
Artık global bir writer'ımız olduğuna göre, kod tabanının her yerinden kullanılabilecek bir `println` makrosu ekleyebiliriz. Rust'ın [makro söz dizimi][macro syntax] biraz tuhaftır, bu yüzden sıfırdan bir makro yazmaya çalışmayacağız. Bunun yerine, standart kütüphanedeki [`println!` makrosunun][`println!` macro] kaynağına bakıyoruz:

[macro syntax]: https://doc.rust-lang.org/nightly/book/ch20-05-macros.html#declarative-macros-for-general-metaprogramming
[`println!` macro]: https://doc.rust-lang.org/nightly/std/macro.println!.html

```rust
#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}
```

Makrolar, `match` kollarına benzer şekilde, bir veya daha fazla kural aracılığıyla tanımlanır. `println` makrosunun iki kuralı vardır: İlk kural, argümansız çağrılar içindir; örneğin `println!()`, `print!("\n")` olarak genişletilir ve böylece yalnızca bir yeni satır yazdırır. İkinci kural, `println!("Hello")` veya `println!("Number: {}", 4)` gibi parametreli çağrılar içindir. O da, tüm argümanları ve sona eklenen bir yeni satır `\n` geçirerek `print!` makrosunun bir çağrısına genişletilir.

`#[macro_export]` özniteliği, makroyu tüm crate'e (yalnızca tanımlandığı modüle değil) ve dış crate'lere kullanılabilir kılar. Ayrıca makroyu crate kök dizinine yerleştirir; bu da makroyu `std::macros::println` yerine `use std::println` aracılığıyla içe aktarmamız gerektiği anlamına gelir.

[`print!` makrosu][`print!` macro] şöyle tanımlanır:

[`print!` macro]: https://doc.rust-lang.org/nightly/std/macro.print!.html

```rust
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
```

Makro, `io` modülündeki [`_print` fonksiyonunun][`_print` function] bir çağrısına genişler. [`$crate` değişkeni][`$crate` variable], diğer crate'lerde kullanıldığında `std`'ye genişleyerek makronun `std` crate'inin dışından da çalışmasını sağlar.

[`format_args` makrosu][`format_args` macro], geçirilen argümanlardan bir [fmt::Arguments] tipi oluşturur ve bu, `_print`'e geçirilir. libstd'nin [`_print` fonksiyonu][`_print` function], `print_to`'yu çağırır; bu da farklı `Stdout` cihazlarını desteklediği için oldukça karmaşıktır. Biz yalnızca VGA arabelleğine yazdırmak istediğimiz için o karmaşıklığa ihtiyacımız yok.

[`_print` function]: https://github.com/rust-lang/rust/blob/29f5c699b11a6a148f097f82eaa05202f8799bbc/src/libstd/io/stdio.rs#L698
[`$crate` variable]: https://doc.rust-lang.org/1.30.0/book/first-edition/macros.html#the-variable-crate
[`format_args` macro]: https://doc.rust-lang.org/nightly/std/macro.format_args.html
[fmt::Arguments]: https://doc.rust-lang.org/nightly/core/fmt/struct.Arguments.html

VGA arabelleğine yazdırmak için, `println!` ve `print!` makrolarını yalnızca kopyalıyor, ancak kendi `_print` fonksiyonumuzu kullanacak şekilde değiştiriyoruz:

```rust
// src/vga_buffer.rs içinde

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}
```

Orijinal `println` tanımından değiştirdiğimiz bir şey, `print!` makrosunun çağrılarının önüne de `$crate` koymamızdır. Bu, yalnızca `println` kullanmak istiyorsak `print!` makrosunu da içe aktarmamıza gerek olmamasını sağlar.

Standart kütüphanede olduğu gibi, her iki makroyu da crate'imizin her yerinde kullanılabilir kılmak için `#[macro_export]` özniteliğini ekliyoruz. Bunun makroları crate'in kök ad alanına (namespace) yerleştirdiğine dikkat edin, bu yüzden onları `use crate::vga_buffer::println` aracılığıyla içe aktarmak çalışmaz. Bunun yerine `use crate::println` yapmamız gerekir.

`_print` fonksiyonu, statik `WRITER`'ımızı kilitler ve onun üzerinde `write_fmt` metodunu çağırır. Bu metot, içe aktarmamız gereken `Write` trait'inden gelir. Sondaki ek `unwrap()`, yazdırma başarılı olmazsa panic'e neden olur. Ancak `write_str`'de her zaman `Ok` döndürdüğümüz için, bunun olmaması gerekir.

Makroların `_print`'i modülün dışından çağırabilmesi gerektiğinden, fonksiyonun public olması gerekir. Ancak bunu özel (private) bir uygulama detayı olarak kabul ettiğimiz için, onu üretilen belgelerden gizlemek amacıyla [`doc(hidden)` özniteliğini][`doc(hidden)` attribute] ekliyoruz.

[`doc(hidden)` attribute]: https://doc.rust-lang.org/nightly/rustdoc/write-documentation/the-doc-attribute.html#hidden

### `println` ile Hello World
Artık `_start` fonksiyonumuzda `println` kullanabiliriz:

```rust
// src/main.rs içinde

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    loop {}
}
```

Makroyu main fonksiyonunda içe aktarmamıza gerek olmadığına dikkat edin, çünkü o zaten kök ad alanında bulunur.

Beklendiği gibi, artık ekranda bir _“Hello World!”_ görüyoruz:

![“Hello World!” yazdıran QEMU](vga-hello-world.png)

### Panic Mesajlarını Yazdırma

Artık bir `println` makromuz olduğuna göre, onu panic fonksiyonumuzda panic mesajını ve panic'in konumunu yazdırmak için kullanabiliriz:

```rust
// main.rs içinde

/// Bu fonksiyon panic anında çağrılır.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
```

Şimdi `_start` fonksiyonumuza `panic!("Some panic message");` eklediğimizde, aşağıdaki çıktıyı alıyoruz:

![“panicked at 'Some panic message', src/main.rs:28:5” yazdıran QEMU](vga-panic.png)

Böylece yalnızca bir panic oluştuğunu değil, aynı zamanda panic mesajını ve kodun neresinde gerçekleştiğini de biliyoruz.

## Özet
Bu yazıda, VGA metin arabelleğinin yapısını ve ona `0xb8000` adresindeki bellek eşlemesi aracılığıyla nasıl yazılabileceğini öğrendik. Bu belleğe eşlenmiş arabelleğe yazmanın güvensizliğini kapsülleyen ve dışarıya güvenli ve kullanışlı bir arayüz sunan bir Rust modülü oluşturduk.

Cargo sayesinde, üçüncü taraf kütüphanelere bağımlılık eklemenin ne kadar kolay olduğunu da gördük. Eklediğimiz iki bağımlılık, `lazy_static` ve `spin`, OS geliştirmede çok kullanışlıdır ve onları gelecekteki yazılarda daha fazla yerde kullanacağız.

## Sırada ne var?
Bir sonraki yazı, Rust'ın yerleşik birim test (unit test) çerçevesinin nasıl kurulacağını açıklar. Ardından, bu yazıdaki VGA arabellek modülü için bazı temel birim testleri oluşturacağız.
