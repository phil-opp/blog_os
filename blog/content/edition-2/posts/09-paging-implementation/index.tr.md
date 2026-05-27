+++
title = "Paging Uygulaması"
weight = 9
path = "tr/paging-implementation"
date = 2019-03-14

[extra]
chapter = "Memory Management"

# Please update this when updating the translation
translation_based_on_commit = "32f629fb2dc193db0dc0657338bd0ddec5914f05"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

Bu yazı, kernel'imizde paging desteğinin nasıl uygulanacağını gösterir. Önce fiziksel sayfa tablosu frame'lerini kernel için erişilebilir kılmaya yönelik farklı teknikleri inceler ve bunların ilgili avantaj ve dezavantajlarını tartışır. Ardından bir adres çevirme fonksiyonu ve yeni bir eşleme oluşturan bir fonksiyon uygular.

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-09`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-09

<!-- toc -->

## Giriş

[Önceki yazı][previous post], paging kavramına bir giriş yaptı. Paging'i segmentasyonla karşılaştırarak gerekçelendirdi, paging'in ve sayfa tablolarının nasıl çalıştığını açıkladı ve ardından `x86_64`'ün 4 seviyeli sayfa tablosu tasarımını tanıttı. Bootloader'ın kernel'imiz için zaten bir sayfa tablosu hiyerarşisi kurduğunu öğrendik; bu da kernel'imizin zaten sanal adresler üzerinde çalıştığı anlamına geliyor. Bu, yasa dışı bellek erişimlerinin keyfi fiziksel belleği değiştirmek yerine page fault exception'larına neden olması nedeniyle güvenliği artırır.

[previous post]: @/edition-2/posts/08-paging-introduction/index.tr.md

Yazı, sayfa tabloları fiziksel bellekte saklandığı ve kernel'imiz zaten sanal adresler üzerinde çalıştığı için [sayfa tablolarına kernel'imizden erişemediğimiz][end of previous post] sorunuyla sona erdi. Bu yazı, sayfa tablosu frame'lerini kernel'imiz için erişilebilir kılmaya yönelik farklı yaklaşımları inceler. Her yaklaşımın avantaj ve dezavantajlarını tartışacak ve ardından kernel'imiz için bir yaklaşıma karar vereceğiz.

[end of previous post]: @/edition-2/posts/08-paging-introduction/index.tr.md#accessing-the-page-tables

Yaklaşımı uygulamak için bootloader'dan desteğe ihtiyacımız olacak, bu yüzden önce onu yapılandıracağız. Ardından, sanal adresleri fiziksel adreslere çevirmek için sayfa tablosu hiyerarşisinde dolaşan bir fonksiyon uygulayacağız. Son olarak, sayfa tablolarında nasıl yeni eşlemeler oluşturacağımızı ve yeni sayfa tabloları oluşturmak için kullanılmamış bellek frame'lerini nasıl bulacağımızı öğreneceğiz.

## Sayfa Tablolarına Erişmek {#accessing-page-tables}

Sayfa tablolarına kernel'imizden erişmek göründüğü kadar kolay değildir. Sorunu anlamak için, önceki yazıdaki örnek 4 seviyeli sayfa tablosu hiyerarşisine tekrar bakalım:

![Her sayfa tablosu fiziksel bellekte gösterilen örnek bir 4 seviyeli sayfa hiyerarşisi](../paging-introduction/x86_64-page-table-translation.svg)

Buradaki önemli nokta, her sayfa girdisinin bir sonraki tablonun _fiziksel_ adresini saklamasıdır. Bu, bu adresler için de bir çeviri çalıştırma ihtiyacını ortadan kaldırır; ki bu, performans için kötü olurdu ve kolayca bitmeyen çeviri döngülerine neden olabilirdi.

Bizim için sorun, kernel'imiz de sanal adresler üzerinde çalıştığı için fiziksel adreslere kernel'imizden doğrudan erişememizdir. Örneğin, `4 KiB` adresine eriştiğimizde, seviye 4 sayfa tablosunun saklandığı `4 KiB` _fiziksel_ adresine değil, `4 KiB` _sanal_ adresine erişiriz. `4 KiB` fiziksel adresine erişmek istediğimizde, bunu yalnızca ona eşlenmiş bir sanal adres aracılığıyla yapabiliriz.

Yani sayfa tablosu frame'lerine erişmek için, onlara bazı sanal sayfaları eşlememiz gerekir. Bu eşlemeleri oluşturmanın, hepsi de keyfi sayfa tablosu frame'lerine erişmemize olanak tanıyan farklı yolları vardır.

### Kimlik Eşleme (Identity Mapping)

Basit bir çözüm, **tüm sayfa tablolarını kimlik eşlemektir (identity map)**:

![Çeşitli sanal sayfaların aynı adresteki fiziksel frame'e eşlendiği bir sanal ve bir fiziksel adres alanı](identity-mapped-page-tables.svg)

Bu örnekte, çeşitli kimlik eşlenmiş sayfa tablosu frame'leri görüyoruz. Bu sayede, sayfa tablolarının fiziksel adresleri aynı zamanda geçerli sanal adreslerdir; böylece CR3 register'ından başlayarak tüm seviyelerdeki sayfa tablolarına kolayca erişebiliriz.

Ancak bu, sanal adres alanını dağınık hale getirir ve daha büyük boyutlu sürekli bellek bölgeleri bulmayı zorlaştırır. Örneğin, yukarıdaki grafikte, örneğin [bir dosyayı belleğe eşlemek][memory-mapping a file] için 1000&nbsp;KiB boyutunda bir sanal bellek bölgesi oluşturmak istediğimizi hayal edin. Bölgeye `28 KiB`'ta başlayamayız, çünkü `1004 KiB`'taki zaten eşlenmiş sayfayla çakışırdı. Bu yüzden, yeterince büyük bir eşlenmemiş alan bulana kadar daha ileriye bakmamız gerekir; örneğin `1008 KiB`'ta. Bu, [segmentasyondaki][segmentation] gibi benzer bir parçalanma sorunudur.

[memory-mapping a file]: https://en.wikipedia.org/wiki/Memory-mapped_file
[segmentation]: @/edition-2/posts/08-paging-introduction/index.tr.md#fragmentation

Aynı şekilde, yeni sayfa tabloları oluşturmayı çok daha zorlaştırır; çünkü karşılık gelen sayfaları zaten kullanımda olmayan fiziksel frame'ler bulmamız gerekir. Örneğin, belleğe eşlenmiş dosyamız için `1008 KiB`'ta başlayan _sanal_ 1000&nbsp;KiB'lık bellek bölgesini ayırdığımızı varsayalım. Artık `1000 KiB` ile `2008 KiB` arasında _fiziksel_ adrese sahip hiçbir frame'i kullanamayız, çünkü onu kimlik eşleyemeyiz.

### Sabit Bir Ofsette Eşleme

Sanal adres alanını dağınık hale getirme sorunundan kaçınmak için, **sayfa tablosu eşlemeleri için ayrı bir bellek bölgesi kullanabiliriz**. Yani sayfa tablosu frame'lerini kimlik eşlemek yerine, onları sanal adres alanında sabit bir ofsette eşleriz. Örneğin, ofset 10&nbsp;TiB olabilir:

![Kimlik eşlemesindeki şeklin aynısı, ancak her eşlenmiş sanal sayfa 10 TiB ofsetlidir.](page-tables-mapped-at-offset.svg)

`10 TiB..(10 TiB + fiziksel bellek boyutu)` aralığındaki sanal belleği yalnızca sayfa tablosu eşlemeleri için kullanarak, kimlik eşlemesinin çakışma sorunlarından kaçınırız. Sanal adres alanının böylesine büyük bir bölgesini ayırmak, yalnızca sanal adres alanı fiziksel bellek boyutundan çok daha büyükse mümkündür. Bu, 48-bit adres alanı 256&nbsp;TiB büyüklüğünde olduğu için x86_64'te bir sorun değildir.

Bu yaklaşımın hâlâ, yeni bir sayfa tablosu oluşturduğumuzda yeni bir eşleme oluşturmamız gerekmesi dezavantajı vardır. Ayrıca, diğer adres alanlarının sayfa tablolarına erişmeye izin vermez; ki bu, yeni bir süreç oluştururken yararlı olurdu.

### Tüm Fiziksel Belleği Eşleme {#map-the-complete-physical-memory}

Bu sorunları, yalnızca sayfa tablosu frame'lerini değil, **tüm fiziksel belleği eşleyerek** çözebiliriz:

![Ofset eşlemesindeki şeklin aynısı, ancak yalnızca sayfa tablosu frame'leri değil, her fiziksel frame'in bir eşlemesi var (10 TiB + X'te).](map-complete-physical-memory.svg)

Bu yaklaşım, kernel'imizin diğer adres alanlarının sayfa tablosu frame'leri dahil keyfi fiziksel belleğe erişmesine olanak tanır. Ayrılan sanal bellek aralığı öncekiyle aynı boyuttadır; tek fark artık eşlenmemiş sayfalar içermemesidir.

Bu yaklaşımın dezavantajı, fiziksel belleğin eşlemesini saklamak için ek sayfa tablolarına ihtiyaç duyulmasıdır. Bu sayfa tablolarının bir yerde saklanması gerekir, bu yüzden fiziksel belleğin bir kısmını kullanırlar; bu da az miktarda belleğe sahip cihazlarda bir sorun olabilir.

Ancak x86_64'te, eşleme için varsayılan 4&nbsp;KiB sayfalar yerine 2&nbsp;MiB boyutlu [huge page'ler][huge pages] kullanabiliriz. Bu sayede, 32&nbsp;GiB fiziksel belleği eşlemek sayfa tabloları için yalnızca 132&nbsp;KiB gerektirir; çünkü yalnızca bir seviye 3 tablosu ve 32 seviye 2 tablosu gereklidir. Huge page'ler ayrıca translation lookaside buffer'da (TLB) daha az girdi kullandıkları için önbellek açısından da daha verimlidir.

[huge pages]: https://en.wikipedia.org/wiki/Page_%28computer_memory%29#Multiple_page_sizes

### Geçici Eşleme

Çok az miktarda fiziksel belleğe sahip cihazlar için, sayfa tablosu frame'lerini yalnızca onlara erişmemiz gerektiğinde **geçici olarak eşleyebiliriz**. Geçici eşlemeleri oluşturabilmek için, yalnızca tek bir kimlik eşlenmiş seviye 1 tablosuna ihtiyacımız var:

![0. girdisini seviye 2 tablosu frame'ine eşleyen ve böylece o frame'i 0 adresli sayfaya eşleyen, kimlik eşlenmiş bir seviye 1 tablosuna sahip bir sanal ve bir fiziksel adres alanı](temporarily-mapped-page-tables.svg)

Bu grafikteki seviye 1 tablosu, sanal adres alanının ilk 2&nbsp;MiB'ını kontrol eder. Bunun nedeni, CR3 register'ından başlayıp seviye 4, seviye 3 ve seviye 2 sayfa tablolarındaki 0. girdiyi takip ederek ona ulaşılabilmesidir. `8` indeksli girdi, `32 KiB` adresindeki sanal sayfayı `32 KiB` adresindeki fiziksel frame'e eşler ve böylece seviye 1 tablosunun kendisini kimlik eşler. Grafik, bu kimlik eşlemesini `32 KiB`'taki yatay okla gösterir.

Kimlik eşlenmiş seviye 1 tablosuna yazarak, kernel'imiz en fazla 511 geçici eşleme oluşturabilir (512 eksi kimlik eşlemesi için gereken girdi). Yukarıdaki örnekte, kernel iki geçici eşleme oluşturdu:

- Seviye 1 tablosunun 0. girdisini `24 KiB` adresindeki frame'e eşleyerek, `0 KiB`'taki sanal sayfanın seviye 2 sayfa tablosunun fiziksel frame'ine geçici bir eşlemesini oluşturdu; kesik çizgili okla gösterilmiştir.
- Seviye 1 tablosunun 9. girdisini `4 KiB` adresindeki frame'e eşleyerek, `36 KiB`'taki sanal sayfanın seviye 4 sayfa tablosunun fiziksel frame'ine geçici bir eşlemesini oluşturdu; kesik çizgili okla gösterilmiştir.

Artık kernel, `0 KiB` sayfasına yazarak seviye 2 sayfa tablosuna ve `36 KiB` sayfasına yazarak seviye 4 sayfa tablosuna erişebilir.

Geçici eşlemelerle keyfi bir sayfa tablosu frame'ine erişme süreci şöyle olurdu:

- Kimlik eşlenmiş seviye 1 tablosunda boş bir girdi ara.
- O girdiyi, erişmek istediğimiz sayfa tablosunun fiziksel frame'ine eşle.
- Hedef frame'e, girdiye eşlenen sanal sayfa aracılığıyla eriş.
- Girdiyi tekrar kullanılmamış olarak ayarla ve böylece geçici eşlemeyi tekrar kaldır.

Bu yaklaşım, eşlemeleri oluşturmak için aynı 512 sanal sayfayı yeniden kullanır ve bu yüzden yalnızca 4&nbsp;KiB fiziksel bellek gerektirir. Dezavantajı, biraz zahmetli olmasıdır; özellikle yeni bir eşleme birden çok tablo seviyesinde değişiklik gerektirebileceği için, bu da yukarıdaki süreci birden çok kez tekrarlamamız gerekeceği anlamına gelir.

### Özyinelemeli Sayfa Tabloları {#recursive-page-tables}

Hiç ek sayfa tablosu gerektirmeyen bir başka ilginç yaklaşım, **sayfa tablosunu özyinelemeli (recursive) olarak eşlemektir**. Bu yaklaşımın arkasındaki fikir, seviye 4 sayfa tablosundan bir girdiyi seviye 4 tablosunun kendisine eşlemektir. Bunu yaparak, sanal adres alanının bir kısmını etkili bir şekilde ayırırız ve mevcut ve gelecekteki tüm sayfa tablosu frame'lerini o alana eşleriz.

Tüm bunların nasıl çalıştığını anlamak için bir örnek üzerinden gidelim:

![Her sayfa tablosu fiziksel bellekte gösterilen örnek bir 4 seviyeli sayfa hiyerarşisi. Seviye 4 sayfasının 511. girdisi, seviye 4 tablosunun kendi frame'i olan 4KiB frame'ine eşlenmiştir.](recursive-page-table.png)

[Bu yazının başındaki örnekten][example at the beginning of this post] tek fark, seviye 4 tablosundaki `511` indeksindeki ek girdidir; bu girdi, seviye 4 tablosunun kendi frame'i olan `4 KiB` fiziksel frame'ine eşlenmiştir.

[example at the beginning of this post]: #accessing-page-tables

CPU bir çeviride bu girdiyi takip ettiğinde, bir seviye 3 tablosuna değil, yine aynı seviye 4 tablosuna ulaşır. Bu, kendini çağıran özyinelemeli bir fonksiyona benzer, bu yüzden bu tabloya _özyinelemeli sayfa tablosu (recursive page table)_ denir. Önemli olan, CPU'nun seviye 4 tablosundaki her girdinin bir seviye 3 tablosuna işaret ettiğini varsaymasıdır, bu yüzden artık seviye 4 tablosunu bir seviye 3 tablosu olarak ele alır. Bu işe yarar, çünkü x86_64'te tüm seviyelerdeki tablolar tam olarak aynı düzene sahiptir.

Gerçek çeviriye başlamadan önce özyinelemeli girdiyi bir veya birden çok kez takip ederek, CPU'nun dolaştığı seviye sayısını etkili bir şekilde kısaltabiliriz. Örneğin, özyinelemeli girdiyi bir kez takip edip ardından seviye 3 tablosuna geçersek, CPU seviye 3 tablosunun bir seviye 2 tablosu olduğunu düşünür. Daha ileri gidildiğinde, seviye 2 tablosunu bir seviye 1 tablosu ve seviye 1 tablosunu eşlenmiş frame olarak ele alır. Bu, artık seviye 1 sayfa tablosunu okuyup yazabileceğimiz anlamına gelir; çünkü CPU onun eşlenmiş frame olduğunu düşünür. Aşağıdaki grafik beş çeviri adımını gösterir:

![Yukarıdaki örnek 4 seviyeli sayfa hiyerarşisi, 5 okla: CR4'ten seviye 4 tablosuna "Adım 0", seviye 4 tablosundan seviye 4 tablosuna "Adım 1", seviye 4 tablosundan seviye 3 tablosuna "Adım 2", seviye 3 tablosundan seviye 2 tablosuna "Adım 3" ve seviye 2 tablosundan seviye 1 tablosuna "Adım 4".](recursive-page-table-access-level-1.png)

Benzer şekilde, dolaşılan seviye sayısını ikiye düşürmek için çeviriye başlamadan önce özyinelemeli girdiyi iki kez takip edebiliriz:

![Aynı 4 seviyeli sayfa hiyerarşisi, şu 4 okla: CR4'ten seviye 4 tablosuna "Adım 0", seviye 4 tablosundan seviye 4 tablosuna "Adım 1&2", seviye 4 tablosundan seviye 3 tablosuna "Adım 3" ve seviye 3 tablosundan seviye 2 tablosuna "Adım 4".](recursive-page-table-access-level-2.png)

Adım adım gidelim: İlk olarak, CPU seviye 4 tablosundaki özyinelemeli girdiyi takip eder ve bir seviye 3 tablosuna ulaştığını düşünür. Ardından özyinelemeli girdiyi tekrar takip eder ve bir seviye 2 tablosuna ulaştığını düşünür. Ama gerçekte hâlâ seviye 4 tablosundadır. CPU şimdi farklı bir girdiyi takip ettiğinde, bir seviye 3 tablosuna iner, ancak zaten bir seviye 1 tablosunda olduğunu düşünür. Yani sonraki girdi bir seviye 2 tablosuna işaret ederken, CPU onun eşlenmiş frame'e işaret ettiğini düşünür; bu da seviye 2 tablosunu okuyup yazmamıza olanak tanır.

Seviye 3 ve 4 tablolarına erişim aynı şekilde çalışır. Seviye 3 tablosuna erişmek için, özyinelemeli girdiyi üç kez takip ederiz ve CPU'yu zaten bir seviye 1 tablosunda olduğunu düşünmesi için kandırırız. Ardından başka bir girdiyi takip eder ve CPU'nun eşlenmiş frame olarak ele aldığı bir seviye 3 tablosuna ulaşırız. Seviye 4 tablosunun kendisine erişmek için, CPU seviye 4 tablosunun kendisini eşlenmiş frame olarak ele alana kadar özyinelemeli girdiyi yalnızca dört kez takip ederiz (aşağıdaki grafikte mavi renkte).

![Aynı 4 seviyeli sayfa hiyerarşisi, şu 3 okla: CR4'ten seviye 4 tablosuna "Adım 0", seviye 4 tablosundan seviye 4 tablosuna "Adım 1,2,3" ve seviye 4 tablosundan seviye 3 tablosuna "Adım 4". Mavi renkte, seviye 4 tablosundan seviye 4 tablosuna alternatif "Adım 1,2,3,4" oku.](recursive-page-table-access-level-3.png)

Bu kavramı kafanızda oturtmak biraz zaman alabilir, ama pratikte oldukça iyi çalışır.

Aşağıdaki bölümde, özyinelemeli girdiyi bir veya birden çok kez takip etmek için sanal adreslerin nasıl oluşturulacağını açıklıyoruz. Uygulamamız için özyinelemeli paging kullanmayacağız, bu yüzden yazıya devam etmek için onu okumanıza gerek yok. İlginizi çekiyorsa, genişletmek için yalnızca _"Adres Hesaplama"_ya tıklayın.

---

<details>
<summary><h4>Adres Hesaplama</h4></summary>

Gerçek çeviriden önce özyinelemeli girdiyi bir veya birden çok kez takip ederek tüm seviyelerdeki tablolara erişebileceğimizi gördük. Dört seviyenin tablolarına yönelik indeksler doğrudan sanal adresten türetildiğinden, bu teknik için özel sanal adresler oluşturmamız gerekir. Hatırlayın, sayfa tablosu indeksleri adresten şu şekilde türetilir:

![0–12 bitleri sayfa ofseti, 12–21 bitleri seviye 1 indeksi, 21–30 bitleri seviye 2 indeksi, 30–39 bitleri seviye 3 indeksi ve 39–48 bitleri seviye 4 indeksidir](../paging-introduction/x86_64-table-indices-from-address.svg)

Belirli bir sayfayı eşleyen seviye 1 sayfa tablosuna erişmek istediğimizi varsayalım. Yukarıda öğrendiğimiz gibi, bu, seviye 4, seviye 3 ve seviye 2 indeksleriyle devam etmeden önce özyinelemeli girdiyi bir kez takip etmemiz gerektiği anlamına gelir. Bunu yapmak için, adresin her bloğunu bir blok sağa kaydırıyor ve orijinal seviye 4 indeksini özyinelemeli girdinin indeksine ayarlıyoruz:

![0–12 bitleri seviye 1 tablosu frame'ine ofset, 12–21 bitleri seviye 2 indeksi, 21–30 bitleri seviye 3 indeksi, 30–39 bitleri seviye 4 indeksi ve 39–48 bitleri özyinelemeli girdinin indeksidir](table-indices-from-address-recursive-level-1.svg)

O sayfanın seviye 2 tablosuna erişmek için, her indeks bloğunu iki blok sağa kaydırıyor ve hem orijinal seviye 4 indeksinin hem de orijinal seviye 3 indeksinin bloklarını özyinelemeli girdinin indeksine ayarlıyoruz:

![0–12 bitleri seviye 2 tablosu frame'ine ofset, 12–21 bitleri seviye 3 indeksi, 21–30 bitleri seviye 4 indeksi ve 30–39 bitleri ile 39–48 bitleri özyinelemeli girdinin indeksidir](table-indices-from-address-recursive-level-2.svg)

Seviye 3 tablosuna erişim, her bloğu üç blok sağa kaydırarak ve orijinal seviye 4, seviye 3 ve seviye 2 adres blokları için özyinelemeli indeksi kullanarak çalışır:

![0–12 bitleri seviye 3 tablosu frame'ine ofset, 12–21 bitleri seviye 4 indeksi ve 21–30, 30–39 ile 39–48 bitleri özyinelemeli girdinin indeksidir](table-indices-from-address-recursive-level-3.svg)

Son olarak, seviye 4 tablosuna her bloğu dört blok sağa kaydırarak ve ofset hariç tüm adres blokları için özyinelemeli indeksi kullanarak erişebiliriz:

![0–12 bitleri seviye 1 tablosu frame'ine ofset ve 12–21, 21–30, 30–39 ile 39–48 bitleri özyinelemeli girdinin indeksidir](table-indices-from-address-recursive-level-4.svg)

Artık dört seviyenin hepsinin sayfa tabloları için sanal adresler hesaplayabiliriz. İndeksini, bir sayfa tablosu girdisinin boyutu olan 8 ile çarparak, tam olarak belirli bir sayfa tablosu girdisine işaret eden bir adres bile hesaplayabiliriz.

Aşağıdaki tablo, farklı türlerdeki frame'lere erişmek için adres yapısını özetler:

Şunun için Sanal Adres | Adres Yapısı ([sekizli][octal])
---------------------- | -------------------------------
Sayfa                  | `0o_SSSSSS_AAA_BBB_CCC_DDD_EEEE`
Seviye 1 Tablosu Girdisi | `0o_SSSSSS_RRR_AAA_BBB_CCC_DDDD`
Seviye 2 Tablosu Girdisi | `0o_SSSSSS_RRR_RRR_AAA_BBB_CCCC`
Seviye 3 Tablosu Girdisi | `0o_SSSSSS_RRR_RRR_RRR_AAA_BBBB`
Seviye 4 Tablosu Girdisi | `0o_SSSSSS_RRR_RRR_RRR_RRR_AAAA`

[octal]: https://en.wikipedia.org/wiki/Octal

Burada `AAA` seviye 4 indeksi, `BBB` seviye 3 indeksi, `CCC` seviye 2 indeksi ve `DDD` eşlenmiş frame'in seviye 1 indeksidir; `EEEE` ise ona olan ofsettir. `RRR` özyinelemeli girdinin indeksidir. Bir indeks (üç basamak) bir ofsete (dört basamak) dönüştürüldüğünde, bu onu 8 (bir sayfa tablosu girdisinin boyutu) ile çarparak yapılır. Bu ofsetle, elde edilen adres doğrudan ilgili sayfa tablosu girdisine işaret eder.

`SSSSSS` işaret genişletme bitleridir; yani hepsi 47. bitin kopyalarıdır. Bu, x86_64 mimarisinde geçerli adresler için özel bir gereksinimdir. Bunu [önceki yazıda][sign extension] açıkladık.

[sign extension]: @/edition-2/posts/08-paging-introduction/index.tr.md#paging-on-x86-64

Adresleri temsil etmek için [sekizli (octal)][octal] sayılar kullanıyoruz, çünkü her sekizli karakter üç biti temsil eder; bu da farklı sayfa tablosu seviyelerinin 9-bit indekslerini açıkça ayırmamıza olanak tanır. Bu, her karakterin dört biti temsil ettiği onaltılık sistemle mümkün değildir.

##### Rust Kodunda

Bu tür adresleri Rust kodunda oluşturmak için bit düzeyinde işlemler kullanabilirsiniz:

```rust
// karşılık gelen sayfa tablolarına erişmek istediğiniz sanal adres
let addr: usize = […];

let r = 0o777; // özyinelemeli indeks
let sign = 0o177777 << 48; // işaret genişletme

// çevirmek istediğimiz adresin sayfa tablosu indekslerini al
let l4_idx = (addr >> 39) & 0o777; // seviye 4 indeksi
let l3_idx = (addr >> 30) & 0o777; // seviye 3 indeksi
let l2_idx = (addr >> 21) & 0o777; // seviye 2 indeksi
let l1_idx = (addr >> 12) & 0o777; // seviye 1 indeksi
let page_offset = addr & 0o7777;

// tablo adreslerini hesapla
let level_4_table_addr =
    sign | (r << 39) | (r << 30) | (r << 21) | (r << 12);
let level_3_table_addr =
    sign | (r << 39) | (r << 30) | (r << 21) | (l4_idx << 12);
let level_2_table_addr =
    sign | (r << 39) | (r << 30) | (l4_idx << 21) | (l3_idx << 12);
let level_1_table_addr =
    sign | (r << 39) | (l4_idx << 30) | (l3_idx << 21) | (l2_idx << 12);
```

Yukarıdaki kod, `0o777` (511) indeksli son seviye 4 girdisinin özyinelemeli olarak eşlendiğini varsayar. Şu anda durum böyle değil, bu yüzden kod henüz çalışmaz. Bootloader'a özyinelemeli eşlemeyi kurmasını nasıl söyleyeceğinizi aşağıda görün.

Bit düzeyinde işlemleri elle gerçekleştirmeye alternatif olarak, çeşitli sayfa tablosu işlemleri için güvenli soyutlamalar sağlayan `x86_64` crate'inin [`RecursivePageTable`] tipini kullanabilirsiniz. Örneğin, aşağıdaki kod bir sanal adresin eşlenmiş fiziksel adresine nasıl çevrileceğini gösterir:

[`RecursivePageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.RecursivePageTable.html

```rust
// src/memory.rs içinde

use x86_64::structures::paging::{Mapper, Page, PageTable, RecursivePageTable};
use x86_64::{VirtAddr, PhysAddr};

/// Seviye 4 adresinden bir RecursivePageTable örneği oluşturur.
let level_4_table_addr = […];
let level_4_table_ptr = level_4_table_addr as *mut PageTable;
let recursive_page_table = unsafe {
    let level_4_table = &mut *level_4_table_ptr;
    RecursivePageTable::new(level_4_table).unwrap();
}


/// Verilen sanal adres için fiziksel adresi al
let addr: u64 = […]
let addr = VirtAddr::new(addr);
let page: Page = Page::containing_address(addr);

// çeviriyi gerçekleştir
let frame = recursive_page_table.translate_page(page);
frame.map(|frame| frame.start_address() + u64::from(addr.page_offset()))
```

Yine, bu kod için geçerli bir özyinelemeli eşleme gereklidir. Böyle bir eşlemeyle, eksik olan `level_4_table_addr`, ilk kod örneğindeki gibi hesaplanabilir.

</details>

---

Özyinelemeli Paging, bir sayfa tablosundaki tek bir eşlemenin ne kadar güçlü olabileceğini gösteren ilginç bir tekniktir. Uygulaması nispeten kolaydır ve yalnızca minimal miktarda kurulum gerektirir (yalnızca tek bir özyinelemeli girdi), bu yüzden paging ile ilk deneyler için iyi bir seçimdir.

Ancak bazı dezavantajları da vardır:

- Büyük miktarda sanal bellek işgal eder (512&nbsp;GiB). Bu, büyük 48-bit adres alanında büyük bir sorun değildir, ancak optimal olmayan önbellek davranışına yol açabilir.
- Yalnızca şu anda aktif olan adres alanına kolayca erişmeye izin verir. Diğer adres alanlarına erişim, özyinelemeli girdiyi değiştirerek hâlâ mümkündür, ancak geri geçiş için geçici bir eşleme gereklidir. Bunu nasıl yapacağımızı (güncel olmayan) [_Remap The Kernel_] yazısında anlattık.
- x86'nın sayfa tablosu biçimine büyük ölçüde dayanır ve diğer mimarilerde çalışmayabilir.

[_Remap The Kernel_]: https://os.phil-opp.com/remap-the-kernel/#overview

## Bootloader Desteği

Tüm bu yaklaşımlar, kurulumları için sayfa tablosu değişiklikleri gerektirir. Örneğin, fiziksel bellek için eşlemeler oluşturulması veya seviye 4 tablosunun bir girdisinin özyinelemeli olarak eşlenmesi gerekir. Sorun, sayfa tablolarına erişmenin mevcut bir yolu olmadan bu gerekli eşlemeleri oluşturamamamızdır.

Bu, kernel'imizin üzerinde çalıştığı sayfa tablolarını oluşturan bootloader'ın yardımına ihtiyacımız olduğu anlamına gelir. Bootloader'ın sayfa tablolarına erişimi vardır, bu yüzden ihtiyaç duyduğumuz herhangi bir eşlemeyi oluşturabilir. Mevcut uygulamasında, `bootloader` crate'i yukarıdaki yaklaşımlardan ikisi için, [cargo özellikleri (features)][cargo features] aracılığıyla kontrol edilen destek sağlar:

[cargo features]: https://doc.rust-lang.org/cargo/reference/features.html#the-features-section

- `map_physical_memory` özelliği, tüm fiziksel belleği sanal adres alanında bir yere eşler. Böylece kernel tüm fiziksel belleğe erişebilir ve [_Tüm Fiziksel Belleği Eşleme_](#map-the-complete-physical-memory) yaklaşımını izleyebilir.
- `recursive_page_table` özelliğiyle, bootloader seviye 4 sayfa tablosunun bir girdisini özyinelemeli olarak eşler. Bu, kernel'in sayfa tablolarına [_Özyinelemeli Sayfa Tabloları_](#recursive-page-tables) bölümünde açıklandığı gibi erişmesine olanak tanır.

Kernel'imiz için ilk yaklaşımı seçiyoruz, çünkü basit, platformdan bağımsız ve daha güçlü (sayfa tablosu olmayan frame'lere de erişime izin verir). Gereken bootloader desteğini etkinleştirmek için, `bootloader` bağımlılığımıza `map_physical_memory` özelliğini ekliyoruz:

```toml
[dependencies]
bootloader = { version = "0.9", features = ["map_physical_memory"]}
```

Bu özellik etkinken, bootloader tüm fiziksel belleği kullanılmayan bir sanal adres aralığına eşler. Sanal adres aralığını kernel'imize bildirmek için, bootloader bir _önyükleme bilgisi (boot information)_ yapısı geçirir.

### Önyükleme Bilgisi (Boot Information) {#boot-information}

`bootloader` crate'i, kernel'imize geçirdiği tüm bilgileri içeren bir [`BootInfo`] struct'ı tanımlar. Struct hâlâ erken bir aşamadadır, bu yüzden gelecekteki [semver uyumsuz][semver-incompatible] bootloader sürümlerine güncellerken bazı bozulmalar bekleyin. `map_physical_memory` özelliği etkinken, şu anda iki alanı vardır: `memory_map` ve `physical_memory_offset`:

[`BootInfo`]: https://docs.rs/bootloader/0.9/bootloader/bootinfo/struct.BootInfo.html
[semver-incompatible]: https://doc.rust-lang.org/stable/cargo/reference/specifying-dependencies.html#caret-requirements

- `memory_map` alanı, kullanılabilir fiziksel belleğe genel bir bakış içerir. Bu, kernel'imize sistemde ne kadar fiziksel bellek bulunduğunu ve hangi bellek bölgelerinin VGA donanımı gibi cihazlar için ayrıldığını söyler. Bellek haritası BIOS veya UEFI firmware'inden sorgulanabilir, ancak yalnızca önyükleme sürecinin çok erken aşamasında. Bu nedenle, bootloader tarafından sağlanmalıdır; çünkü kernel'in onu daha sonra alma yolu yoktur. Bellek haritasına bu yazının ilerleyen kısımlarında ihtiyaç duyacağız.
- `physical_memory_offset`, bize fiziksel bellek eşlemesinin sanal başlangıç adresini söyler. Bu ofseti bir fiziksel adrese ekleyerek, karşılık gelen sanal adresi elde ederiz. Bu, kernel'imizden keyfi fiziksel belleğe erişmemize olanak tanır.
- Bu fiziksel bellek ofseti, Cargo.toml'a bir `[package.metadata.bootloader]` tablosu ekleyip `physical-memory-offset = "0x0000f00000000000"` (veya başka herhangi bir değer) alanını ayarlayarak özelleştirilebilir. Ancak, bootloader'ın ofsetin ötesindeki alanla, yani daha önce başka erken fiziksel adreslere eşlemiş olabileceği alanlarla çakışmaya başlayan fiziksel adres değerleriyle karşılaşırsa panic yapabileceğini unutmayın. Yani genel olarak değer ne kadar yüksek olursa (> 1 TiB) o kadar iyidir.

Bootloader, `BootInfo` struct'ını kernel'imize, `_start` fonksiyonumuza bir `&'static BootInfo` argümanı biçiminde geçirir. Bu argümanı henüz fonksiyonumuzda bildirmedik, bu yüzden onu ekleyelim:

```rust
// src/main.rs içinde

use bootloader::BootInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! { // yeni argüman
    […]
}
```

Bu argümanı daha önce dışarıda bırakmak bir sorun değildi, çünkü x86_64 çağırma kuralı ilk argümanı bir CPU register'ında geçirir. Böylece, bildirilmediğinde argüman yalnızca yok sayılır. Ancak, yanlışlıkla yanlış bir argüman tipi kullansaydık bu bir sorun olurdu; çünkü derleyici giriş noktası fonksiyonumuzun doğru tip imzasını bilmez.

### `entry_point` Makrosu

`_start` fonksiyonumuz bootloader'dan harici olarak çağrıldığından, fonksiyon imzamızın hiçbir kontrolü yapılmaz. Bu, hiçbir derleme hatası olmadan onun keyfi argümanlar almasına izin verebileceğimiz, ancak çalışma zamanında başarısız olacağı veya tanımsız davranışa neden olacağı anlamına gelir.

Giriş noktası fonksiyonunun her zaman bootloader'ın beklediği doğru imzaya sahip olduğundan emin olmak için, `bootloader` crate'i bir Rust fonksiyonunu giriş noktası olarak tanımlamanın tip denetimli bir yolunu sağlayan bir [`entry_point`] makrosu sunar. Giriş noktası fonksiyonumuzu bu makroyu kullanacak şekilde yeniden yazalım:

[`entry_point`]: https://docs.rs/bootloader/0.6.4/bootloader/macro.entry_point.html

```rust
// src/main.rs içinde

use bootloader::{BootInfo, entry_point};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    […]
}
```

Giriş noktamız için artık `extern "C"` veya `no_mangle` kullanmamıza gerek yok; çünkü makro gerçek alt seviye `_start` giriş noktasını bizim için tanımlar. `kernel_main` fonksiyonu artık tamamen normal bir Rust fonksiyonudur, bu yüzden onun için keyfi bir ad seçebiliriz. Önemli olan, tip denetimli olmasıdır; böylece yanlış bir fonksiyon imzası kullandığımızda, örneğin bir argüman ekleyerek veya argüman tipini değiştirerek, bir derleme hatası oluşur.

Aynı değişikliği `lib.rs`'imizde de yapalım:

```rust
// src/lib.rs içinde

#[cfg(test)]
use bootloader::{entry_point, BootInfo};

#[cfg(test)]
entry_point!(test_kernel_main);

/// `cargo test` için giriş noktası
#[cfg(test)]
fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
    // önceki gibi
    init();
    test_main();
    hlt_loop();
}
```

Giriş noktası yalnızca test modunda kullanıldığından, tüm öğelere `#[cfg(test)]` özniteliğini ekliyoruz. Test giriş noktamıza, `main.rs`'imizin `kernel_main`'i ile karışıklığı önlemek için belirgin `test_kernel_main` adını veriyoruz. `BootInfo` parametresini şimdilik kullanmıyoruz, bu yüzden kullanılmayan değişken uyarısını susturmak için parametre adının önüne bir `_` koyuyoruz.

## Uygulama

Artık fiziksel belleğe erişimimiz olduğuna göre, nihayet sayfa tablosu kodumuzu uygulamaya başlayabiliriz. İlk olarak, kernel'imizin üzerinde çalıştığı şu anda aktif olan sayfa tablolarına bir göz atacağız. İkinci adımda, verilen bir sanal adresin eşlendiği fiziksel adresi döndüren bir çeviri fonksiyonu oluşturacağız. Son adım olarak, yeni bir eşleme oluşturmak için sayfa tablolarını değiştirmeyi deneyeceğiz.

Başlamadan önce, kodumuz için yeni bir `memory` modülü oluşturuyoruz:

```rust
// src/lib.rs içinde

pub mod memory;
```

Modül için, boş bir `src/memory.rs` dosyası oluşturuyoruz.

### Sayfa Tablolarına Erişmek {#accessing-the-page-tables}

[Önceki yazının sonunda][end of the previous post], kernel'imizin üzerinde çalıştığı sayfa tablolarına bir göz atmaya çalıştık, ancak `CR3` register'ının işaret ettiği fiziksel frame'e erişemediğimiz için başarısız olduk. Aktif seviye 4 sayfa tablosuna bir referans döndüren bir `active_level_4_table` fonksiyonu oluşturarak artık oradan devam edebiliriz:

[end of the previous post]: @/edition-2/posts/08-paging-introduction/index.tr.md#accessing-the-page-tables

```rust
// src/memory.rs içinde

use x86_64::{
    structures::paging::PageTable,
    VirtAddr,
};

/// Aktif seviye 4 tablosuna değiştirilebilir bir referans döndürür.
///
/// Bu fonksiyon unsafe'tir, çünkü çağıranın tüm fiziksel belleğin geçirilen
/// `physical_memory_offset`'te sanal belleğe eşlendiğini garanti etmesi
/// gerekir. Ayrıca, `&mut` referansları takma adlamaktan (aliasing) kaçınmak
/// için bu fonksiyon yalnızca bir kez çağrılmalıdır (takma adlama tanımsız
/// davranıştır).
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}
```

İlk olarak, aktif seviye 4 tablosunun fiziksel frame'ini `CR3` register'ından okuyoruz. Ardından onun fiziksel başlangıç adresini alıyor, bir `u64`'e dönüştürüyor ve sayfa tablosu frame'inin eşlendiği sanal adresi elde etmek için `physical_memory_offset`'e ekliyoruz. Son olarak, sanal adresi `as_mut_ptr` metodu aracılığıyla bir `*mut PageTable` ham işaretçisine dönüştürüyor ve ardından ondan unsafe bir şekilde bir `&mut PageTable` referansı oluşturuyoruz. Bu yazının ilerleyen kısmında sayfa tablolarını değiştireceğimiz için bir `&` referansı yerine bir `&mut` referansı oluşturuyoruz.

Artık bu fonksiyonu seviye 4 tablosunun girdilerini yazdırmak için kullanabiliriz:

```rust
// src/main.rs içinde

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory::active_level_4_table;
    use x86_64::VirtAddr;

    println!("Hello World{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let l4_table = unsafe { active_level_4_table(phys_mem_offset) };

    for (i, entry) in l4_table.iter().enumerate() {
        if !entry.is_unused() {
            println!("L4 Entry {}: {:?}", i, entry);
        }
    }

    // önceki gibi
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

İlk olarak, `BootInfo` struct'ının `physical_memory_offset`'ini bir [`VirtAddr`]'e dönüştürüyor ve onu `active_level_4_table` fonksiyonuna geçiriyoruz. Ardından, sayfa tablosu girdileri üzerinde iterasyon yapmak için `iter` fonksiyonunu ve her elemana ek olarak bir `i` indeksi eklemek için [`enumerate`] kombinatörünü kullanıyoruz. Yalnızca boş olmayan girdileri yazdırıyoruz, çünkü 512 girdinin hepsi ekrana sığmazdı.

[`VirtAddr`]: https://docs.rs/x86_64/0.14.2/x86_64/addr/struct.VirtAddr.html
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate

Onu çalıştırdığımızda, aşağıdaki çıktıyı görüyoruz:

![QEMU; girdi 0 (0x2000, PRESENT, WRITABLE, ACCESSED), girdi 1 (0x894000, PRESENT, WRITABLE, ACCESSED, DIRTY), girdi 31 (0x88e000, PRESENT, WRITABLE, ACCESSED, DIRTY), girdi 175 (0x891000, PRESENT, WRITABLE, ACCESSED, DIRTY) ve girdi 504 (0x897000, PRESENT, WRITABLE, ACCESSED, DIRTY) yazdırıyor](qemu-print-level-4-table.png)

Hepsi farklı seviye 3 tablolarına eşlenen çeşitli boş olmayan girdiler olduğunu görüyoruz. Bu kadar çok bölge var, çünkü kernel kodu, kernel stack'i, fiziksel bellek eşlemesi ve önyükleme bilgisinin hepsi ayrı bellek alanları kullanır.

Sayfa tablolarında daha ileri dolaşmak ve bir seviye 3 tablosuna bakmak için, bir girdinin eşlenmiş frame'ini alıp onu tekrar bir sanal adrese dönüştürebiliriz:

```rust
// src/main.rs'teki `for` döngüsünde

use x86_64::structures::paging::PageTable;

if !entry.is_unused() {
    println!("L4 Entry {}: {:?}", i, entry);

    // girdiden fiziksel adresi al ve onu dönüştür
    let phys = entry.frame().unwrap().start_address();
    let virt = phys.as_u64() + boot_info.physical_memory_offset;
    let ptr = VirtAddr::new(virt).as_mut_ptr();
    let l3_table: &PageTable = unsafe { &*ptr };

    // seviye 3 tablosunun boş olmayan girdilerini yazdır
    for (i, entry) in l3_table.iter().enumerate() {
        if !entry.is_unused() {
            println!("  L3 Entry {}: {:?}", i, entry);
        }
    }
}
```

Seviye 2 ve seviye 1 tablolarına bakmak için, o süreci seviye 3 ve seviye 2 girdileri için tekrarlarız. Tahmin edebileceğiniz gibi, bu çok hızlı bir şekilde çok ayrıntılı (verbose) hale gelir, bu yüzden tam kodu burada göstermiyoruz.

Sayfa tablolarında elle dolaşmak ilginçtir, çünkü CPU'nun çeviriyi nasıl gerçekleştirdiğini anlamaya yardımcı olur. Ancak çoğu zaman, yalnızca verilen bir sanal adres için eşlenmiş fiziksel adresle ilgileniriz, bu yüzden bunun için bir fonksiyon oluşturalım.

### Adresleri Çevirmek

Bir sanal adresi fiziksel bir adrese çevirmek için, eşlenmiş frame'e ulaşana kadar dört seviyeli sayfa tablosunda dolaşmamız gerekir. Bu çeviriyi gerçekleştiren bir fonksiyon oluşturalım:

```rust
// src/memory.rs içinde

use x86_64::PhysAddr;

/// Verilen sanal adresi eşlenmiş fiziksel adrese çevirir, ya da adres
/// eşlenmemişse `None` döndürür.
///
/// Bu fonksiyon unsafe'tir, çünkü çağıranın tüm fiziksel belleğin geçirilen
/// `physical_memory_offset`'te sanal belleğe eşlendiğini garanti etmesi
/// gerekir.
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    translate_addr_inner(addr, physical_memory_offset)
}
```

`unsafe`'in kapsamını sınırlamak için fonksiyonu güvenli bir `translate_addr_inner` fonksiyonuna iletiyoruz. Yukarıda belirttiğimiz gibi, Rust bir `unsafe fn`'in tüm gövdesini büyük bir unsafe blok gibi ele alır. Özel (private) güvenli bir fonksiyonu çağırarak, her `unsafe` işlemini tekrar açık hale getiriyoruz.

Özel iç fonksiyon, gerçek uygulamayı içerir:

```rust
// src/memory.rs içinde

/// `translate_addr` tarafından çağrılan özel fonksiyon.
///
/// Rust, unsafe fonksiyonların tüm gövdesini bir unsafe blok olarak ele aldığı
/// için, `unsafe`'in kapsamını sınırlamak amacıyla bu fonksiyon güvenlidir. Bu
/// fonksiyona bu modülün dışından yalnızca `unsafe fn` aracılığıyla
/// ulaşılabilir olmalıdır.
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    // aktif seviye 4 frame'ini CR3 register'ından oku
    let (level_4_table_frame, _) = Cr3::read();

    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    let mut frame = level_4_table_frame;

    // çok seviyeli sayfa tablosunda dolaş
    for &index in &table_indexes {
        // frame'i bir sayfa tablosu referansına dönüştür
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe {&*table_ptr};

        // sayfa tablosu girdisini oku ve `frame`'i güncelle
        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    // sayfa ofsetini ekleyerek fiziksel adresi hesapla
    Some(frame.start_address() + u64::from(addr.page_offset()))
}
```

`active_level_4_table` fonksiyonumuzu yeniden kullanmak yerine, seviye 4 frame'ini `CR3` register'ından tekrar okuyoruz. Bunu, bu prototip uygulamayı basitleştirdiği için yapıyoruz. Endişelenmeyin, birazdan daha iyi bir çözüm oluşturacağız.

`VirtAddr` struct'ı, dört seviyenin sayfa tablolarına yönelik indeksleri hesaplamak için zaten metotlar sağlar. Bu indeksleri küçük bir dizide saklıyoruz, çünkü bu, sayfa tablolarında bir `for` döngüsü kullanarak dolaşmamıza olanak tanır. Döngünün dışında, fiziksel adresi daha sonra hesaplamak için son ziyaret edilen `frame`'i hatırlıyoruz. `frame`, iterasyon sırasında sayfa tablosu frame'lerine ve son iterasyondan sonra, yani seviye 1 girdisini takip ettikten sonra, eşlenmiş frame'e işaret eder.

Döngünün içinde, frame'i bir sayfa tablosu referansına dönüştürmek için yine `physical_memory_offset`'i kullanıyoruz. Ardından mevcut sayfa tablosunun girdisini okuyor ve eşlenmiş frame'i almak için [`PageTableEntry::frame`] fonksiyonunu kullanıyoruz. Girdi bir frame'e eşlenmemişse, `None` döndürürüz. Girdi bir huge 2&nbsp;MiB veya 1&nbsp;GiB sayfa eşliyorsa, şimdilik panic yaparız.

[`PageTableEntry::frame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page_table/struct.PageTableEntry.html#method.frame

Çeviri fonksiyonumuzu bazı adresleri çevirerek test edelim:

```rust
// src/main.rs içinde

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // yeni içe aktarma
    use blog_os::memory::translate_addr;

    […] // hello world ve blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

    let addresses = [
        // kimlik eşlenmiş vga arabellek sayfası
        0xb8000,
        // bir kod sayfası
        0x201008,
        // bir stack sayfası
        0x0100_0020_1a10,
        // fiziksel adres 0'a eşlenmiş sanal adres
        boot_info.physical_memory_offset,
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let phys = unsafe { translate_addr(virt, phys_mem_offset) };
        println!("{:?} -> {:?}", virt, phys);
    }

    […] // test_main(), "it did not crash" yazdırma ve hlt_loop()
}
```

Onu çalıştırdığımızda, aşağıdaki çıktıyı görüyoruz:

![0xb8000 -> 0xb8000, 0x201008 -> 0x401008, 0x10000201a10 -> 0x279a10, "panicked at 'huge pages not supported'"](qemu-translate-addr.png)

Beklendiği gibi, kimlik eşlenmiş `0xb8000` adresi aynı fiziksel adrese çevriliyor. Kod sayfası ve stack sayfası, bootloader'ın kernel'imiz için ilk eşlemeyi nasıl oluşturduğuna bağlı olarak bazı keyfi fiziksel adreslere çevriliyor. Çeviriden sonra son 12 bitin her zaman aynı kaldığını belirtmekte fayda var; bu mantıklıdır, çünkü bu bitler [_sayfa ofsetidir_][_page offset_] ve çevirinin bir parçası değildir.

[_page offset_]: @/edition-2/posts/08-paging-introduction/index.tr.md#paging-on-x86-64

Her fiziksel adrese `physical_memory_offset` eklenerek erişilebileceğinden, `physical_memory_offset` adresinin kendisinin çevirisi fiziksel adres `0`'a işaret etmelidir. Ancak, eşleme verimlilik için huge page'ler kullandığı ve bu henüz uygulamamızda desteklenmediği için çeviri başarısız olur.

### `OffsetPageTable` Kullanmak {#using-offsetpagetable}

Sanal adresleri fiziksel adreslere çevirmek bir OS kernel'inde yaygın bir görevdir, bu yüzden `x86_64` crate'i bunun için bir soyutlama sağlar. Uygulama, `translate_addr`'ın yanı sıra huge page'leri ve diğer çeşitli sayfa tablosu fonksiyonlarını zaten destekler, bu yüzden aşağıda kendi uygulamamıza huge page desteği eklemek yerine onu kullanacağız.

Soyutlamanın temelinde, çeşitli sayfa tablosu eşleme fonksiyonlarını tanımlayan iki trait vardır:

- [`Mapper`] trait'i sayfa boyutu üzerinde generic'tir ve sayfalar üzerinde çalışan fonksiyonlar sağlar. Örnekler, verilen bir sayfayı aynı boyutta bir frame'e çeviren [`translate_page`] ve sayfa tablosunda yeni bir eşleme oluşturan [`map_to`]'dur.
- [`Translate`] trait'i, birden çok sayfa boyutuyla çalışan fonksiyonlar sağlar; örneğin [`translate_addr`] veya genel [`translate`].

[`Mapper`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html
[`translate_page`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html#tymethod.translate_page
[`map_to`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html#method.map_to
[`Translate`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Translate.html
[`translate_addr`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Translate.html#method.translate_addr
[`translate`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Translate.html#tymethod.translate

Trait'ler yalnızca arayüzü tanımlar, herhangi bir uygulama sağlamazlar. `x86_64` crate'i şu anda trait'leri farklı gereksinimlerle uygulayan üç tip sağlar. [`OffsetPageTable`] tipi, tüm fiziksel belleğin sanal adres alanına bir ofsette eşlendiğini varsayar. [`MappedPageTable`] biraz daha esnektir: Yalnızca her sayfa tablosu frame'inin sanal adres alanına hesaplanabilir bir adreste eşlenmesini gerektirir. Son olarak, [`RecursivePageTable`] tipi, sayfa tablosu frame'lerine [özyinelemeli sayfa tabloları](#recursive-page-tables) aracılığıyla erişmek için kullanılabilir.

[`OffsetPageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.OffsetPageTable.html
[`MappedPageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MappedPageTable.html
[`RecursivePageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.RecursivePageTable.html

Bizim durumumuzda, bootloader tüm fiziksel belleği `physical_memory_offset` değişkeni tarafından belirtilen bir sanal adreste eşler, bu yüzden `OffsetPageTable` tipini kullanabiliriz. Onu başlatmak için, `memory` modülümüzde yeni bir `init` fonksiyonu oluşturuyoruz:

```rust
use x86_64::structures::paging::OffsetPageTable;

/// Yeni bir OffsetPageTable başlatır.
///
/// Bu fonksiyon unsafe'tir, çünkü çağıranın tüm fiziksel belleğin geçirilen
/// `physical_memory_offset`'te sanal belleğe eşlendiğini garanti etmesi
/// gerekir. Ayrıca, `&mut` referansları takma adlamaktan kaçınmak için bu
/// fonksiyon yalnızca bir kez çağrılmalıdır (takma adlama tanımsız davranıştır).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

// özel yap
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{…}
```

Fonksiyon, `physical_memory_offset`'i bir argüman olarak alır ve `'static` ömrüne sahip yeni bir `OffsetPageTable` örneği döndürür. Bu, örneğin kernel'imizin tüm çalışma süresi boyunca geçerli kaldığı anlamına gelir. Fonksiyon gövdesinde, önce seviye 4 sayfa tablosuna değiştirilebilir bir referans almak için `active_level_4_table` fonksiyonunu çağırıyoruz. Ardından bu referansla [`OffsetPageTable::new`] fonksiyonunu çağırıyoruz. İkinci parametre olarak, `new` fonksiyonu fiziksel belleğin eşlemesinin başladığı sanal adresi bekler; bu da `physical_memory_offset` değişkeninde verilir.

[`OffsetPageTable::new`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.OffsetPageTable.html#method.new

`active_level_4_table` fonksiyonu, birden çok kez çağrıldığında kolayca takma adlanmış değiştirilebilir referanslara yol açabileceği ve bu da tanımsız davranışa neden olabileceği için bundan sonra yalnızca `init` fonksiyonundan çağrılmalıdır. Bu nedenle, `pub` belirtecini kaldırarak fonksiyonu özel yapıyoruz.

Artık kendi `memory::translate_addr` fonksiyonumuz yerine `Translate::translate_addr` metodunu kullanabiliriz. `kernel_main`'imizde yalnızca birkaç satırı değiştirmemiz gerekiyor:

```rust
// src/main.rs içinde

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // yeni: farklı içe aktarmalar
    use blog_os::memory;
    use x86_64::{structures::paging::Translate, VirtAddr};

    […] // hello world ve blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    // yeni: bir mapper başlat
    let mapper = unsafe { memory::init(phys_mem_offset) };

    let addresses = […]; // öncekiyle aynı

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        // yeni: `mapper.translate_addr` metodunu kullan
        let phys = mapper.translate_addr(virt);
        println!("{:?} -> {:?}", virt, phys);
    }

    […] // test_main(), "it did not crash" yazdırma ve hlt_loop()
}
```

Sağladığı [`translate_addr`] metodunu kullanmak için `Translate` trait'ini içe aktarmamız gerekir.

Onu şimdi çalıştırdığımızda, öncekiyle aynı çeviri sonuçlarını görüyoruz; farkı, huge page çevirisinin artık çalışmasıdır:

![0xb8000 -> 0xb8000, 0x201008 -> 0x401008, 0x10000201a10 -> 0x279a10, 0x18000000000 -> 0x0](qemu-mapper-translate-addr.png)

Beklendiği gibi, `0xb8000`'in ve kod ile stack adreslerinin çevirileri kendi çeviri fonksiyonumuzdaki gibi aynı kalıyor. Buna ek olarak, artık `physical_memory_offset` sanal adresinin `0x0` fiziksel adresine eşlendiğini görüyoruz.

`MappedPageTable` tipinin çeviri fonksiyonunu kullanarak, huge page desteğini uygulama işinden kurtuluyoruz. Ayrıca, bir sonraki bölümde kullanacağımız `map_to` gibi diğer sayfa fonksiyonlarına da erişimimiz var.

Bu noktada, artık `memory::translate_addr` ve `memory::translate_addr_inner` fonksiyonlarımıza ihtiyacımız yok, bu yüzden onları silebiliriz.

### Yeni Bir Eşleme Oluşturmak

Şimdiye kadar, herhangi bir şeyi değiştirmeden yalnızca sayfa tablolarına baktık. Daha önce eşlenmemiş bir sayfa için yeni bir eşleme oluşturarak bunu değiştirelim.

Uygulamamız için [`Mapper`] trait'inin [`map_to`] fonksiyonunu kullanacağız, bu yüzden önce o fonksiyona bir göz atalım. Belgeler bize onun dört argüman aldığını söylüyor: eşlemek istediğimiz sayfa, sayfanın eşlenmesi gereken frame, sayfa tablosu girdisi için bir bayrak kümesi ve bir `frame_allocator`. Frame allocator gereklidir, çünkü verilen sayfayı eşlemek ek sayfa tabloları oluşturmayı gerektirebilir; bunlar da destek deposu (backing storage) olarak kullanılmamış frame'lere ihtiyaç duyar.

[`map_to`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.Mapper.html#tymethod.map_to
[`Mapper`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.Mapper.html

#### Bir `create_example_mapping` Fonksiyonu {#a-create-example-mapping-function}

Uygulamamızın ilk adımı, verilen bir sanal sayfayı VGA metin arabelleğinin fiziksel frame'i olan `0xb8000`'e eşleyen yeni bir `create_example_mapping` fonksiyonu oluşturmaktır. O frame'i seçiyoruz, çünkü eşlemenin doğru oluşturulup oluşturulmadığını kolayca test etmemize olanak tanıyor: Yalnızca yeni eşlenen sayfaya yazmamız ve yazmanın ekranda görünüp görünmediğine bakmamız yeterli.

`create_example_mapping` fonksiyonu şöyle görünür:

```rust
// src/memory.rs içinde

use x86_64::{
    PhysAddr,
    structures::paging::{Page, PhysFrame, Mapper, Size4KiB, FrameAllocator}
};

/// Verilen sayfa için `0xb8000` frame'ine örnek bir eşleme oluşturur.
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe {
        // FIXME: bu güvenli değil, bunu yalnızca test için yapıyoruz
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_to_result.expect("map_to failed").flush();
}
```

Eşlenmesi gereken `page`'e ek olarak, fonksiyon bir `OffsetPageTable` örneğine değiştirilebilir bir referans ve bir `frame_allocator` bekler. `frame_allocator` parametresi, [`FrameAllocator`] trait'ini uygulayan tüm tipler üzerinde [generic] olmak için [`impl Trait`][impl-trait-arg] söz dizimini kullanır. Trait, hem standart 4&nbsp;KiB sayfalarla hem de huge 2&nbsp;MiB/1&nbsp;GiB sayfalarla çalışmak için [`PageSize`] trait'i üzerinde generic'tir. Yalnızca 4&nbsp;KiB'lık bir eşleme oluşturmak istiyoruz, bu yüzden generic parametreyi `Size4KiB` olarak ayarlıyoruz.

[impl-trait-arg]: https://doc.rust-lang.org/book/ch10-02-traits.html#traits-as-parameters
[generic]: https://doc.rust-lang.org/book/ch10-00-generics.html
[`FrameAllocator`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.FrameAllocator.html
[`PageSize`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/trait.PageSize.html

[`map_to`] metodu unsafe'tir, çünkü çağıranın frame'in zaten kullanımda olmadığından emin olması gerekir. Bunun nedeni, aynı frame'i iki kez eşlemenin tanımsız davranışa yol açabilmesidir; örneğin iki farklı `&mut` referansı aynı fiziksel bellek konumuna işaret ettiğinde. Bizim durumumuzda, zaten eşlenmiş olan VGA metin arabelleği frame'ini yeniden kullanıyoruz, bu yüzden gereken koşulu çiğniyoruz. Ancak, `create_example_mapping` fonksiyonu yalnızca geçici bir test fonksiyonudur ve bu yazıdan sonra kaldırılacaktır, bu yüzden sorun değil. Güvensizliği bize hatırlatmak için satıra bir `FIXME` yorumu koyuyoruz.

`page` ve `unused_frame`'e ek olarak, `map_to` metodu eşleme için bir bayrak kümesi ve birazdan açıklanacak `frame_allocator`'a bir referans alır. Bayraklar için, tüm geçerli girdiler için gerekli olduğu için `PRESENT` bayrağını ve eşlenen sayfayı yazılabilir kılmak için `WRITABLE` bayrağını ayarlıyoruz. Tüm olası bayrakların listesi için, önceki yazının [_Sayfa Tablosu Biçimi_][_Page Table Format_] bölümüne bakın.

[_Page Table Format_]: @/edition-2/posts/08-paging-introduction/index.tr.md#page-table-format

[`map_to`] fonksiyonu başarısız olabilir, bu yüzden bir [`Result`] döndürür. Bu yalnızca sağlam (robust) olması gerekmeyen bazı örnek kod olduğundan, bir hata oluştuğunda panic yapmak için yalnızca [`expect`] kullanıyoruz. Başarı durumunda, fonksiyon, yeni eşlenen sayfayı translation lookaside buffer'dan (TLB) [`flush`] metoduyla temizlemenin kolay bir yolunu sağlayan bir [`MapperFlush`] tipi döndürür. `Result` gibi, bu tip de yanlışlıkla onu kullanmayı unuttuğumuzda bir uyarı yaymak için [`#[must_use]`][must_use] özniteliğini kullanır.

[`Result`]: https://doc.rust-lang.org/core/result/enum.Result.html
[`expect`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.expect
[`MapperFlush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html
[`flush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html#method.flush
[must_use]: https://doc.rust-lang.org/std/result/#results-must-be-used

#### Sahte (dummy) Bir `FrameAllocator`

`create_example_mapping`'i çağırabilmek için, önce `FrameAllocator` trait'ini uygulayan bir tip oluşturmamız gerekir. Yukarıda belirtildiği gibi, trait, `map_to` tarafından gerekli olduklarında yeni sayfa tabloları için frame'ler ayırmaktan sorumludur.

Basit durumla başlayalım ve yeni sayfa tabloları oluşturmamıza gerek olmadığını varsayalım. Bu durum için, her zaman `None` döndüren bir frame allocator yeterlidir. Eşleme fonksiyonumuzu test etmek için böyle bir `EmptyFrameAllocator` oluşturuyoruz:

```rust
// src/memory.rs içinde

/// Her zaman `None` döndüren bir FrameAllocator.
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}
```

`FrameAllocator`'ı uygulamak unsafe'tir, çünkü uygulayanın, allocator'ın yalnızca kullanılmamış frame'ler verdiğini garanti etmesi gerekir. Aksi takdirde tanımsız davranış meydana gelebilir; örneğin iki sanal sayfa aynı fiziksel frame'e eşlendiğinde. `EmptyFrameAllocator`'ımız yalnızca `None` döndürür, bu yüzden bu durumda bu bir sorun değildir.

#### Bir Sanal Sayfa Seçmek

Artık `create_example_mapping` fonksiyonumuza geçirebileceğimiz basit bir frame allocator'ımız var. Ancak allocator her zaman `None` döndürür, bu yüzden bu yalnızca eşlemeyi oluşturmak için ek sayfa tablosu frame'leri gerekmediğinde çalışır. Ek sayfa tablosu frame'lerinin ne zaman gerekli olduğunu ve ne zaman olmadığını anlamak için bir örnek düşünelim:

![Tek bir eşlenmiş sayfaya ve dört seviyenin hepsinin sayfa tablolarına sahip bir sanal ve bir fiziksel adres alanı](required-page-frames-example.svg)

Grafik, solda sanal adres alanını, sağda fiziksel adres alanını ve aralarında sayfa tablolarını gösterir. Sayfa tabloları, kesik çizgilerle gösterilen fiziksel bellek frame'lerinde saklanır. Sanal adres alanı, mavi renkle işaretlenmiş, `0x803fe00000` adresinde tek bir eşlenmiş sayfa içerir. Bu sayfayı frame'ine çevirmek için, CPU 36&nbsp;KiB adresindeki frame'e ulaşana kadar 4 seviyeli sayfa tablosunda yürür.

Buna ek olarak, grafik VGA metin arabelleğinin fiziksel frame'ini kırmızıyla gösterir. Hedefimiz, `create_example_mapping` fonksiyonumuzu kullanarak daha önce eşlenmemiş bir sanal sayfayı bu frame'e eşlemektir. `EmptyFrameAllocator`'ımız her zaman `None` döndürdüğünden, eşlemeyi allocator'dan ek frame'ler gerekmeyecek şekilde oluşturmak istiyoruz. Bu, eşleme için seçtiğimiz sanal sayfaya bağlıdır.

Grafik, sanal adres alanında her ikisi de sarıyla işaretlenmiş iki aday sayfa gösterir. Bir sayfa, eşlenmiş sayfadan (mavi renkte) 3 sayfa önce olan `0x803fdfd000` adresindedir. Seviye 4 ve seviye 3 sayfa tablosu indeksleri mavi sayfayla aynı olsa da, seviye 2 ve seviye 1 indeksleri farklıdır ([önceki yazıya][page-table-indices] bakın). Seviye 2 tablosuna olan farklı indeks, bu sayfa için farklı bir seviye 1 tablosu kullanıldığı anlamına gelir. Bu seviye 1 tablosu henüz var olmadığından, örnek eşlememiz için o sayfayı seçersek onu oluşturmamız gerekirdi; bu da ek bir kullanılmamış fiziksel frame gerektirirdi. Buna karşılık, `0x803fe02000` adresindeki ikinci aday sayfanın bu sorunu yoktur, çünkü mavi sayfayla aynı seviye 1 sayfa tablosunu kullanır. Böylece, gereken tüm sayfa tabloları zaten vardır.

[page-table-indices]: @/edition-2/posts/08-paging-introduction/index.tr.md#paging-on-x86-64

Özetle, yeni bir eşleme oluşturmanın zorluğu, eşlemek istediğimiz sanal sayfaya bağlıdır. En kolay durumda, sayfa için seviye 1 sayfa tablosu zaten vardır ve yalnızca tek bir girdi yazmamız gerekir. En zor durumda, sayfa henüz hiçbir seviye 3'ün var olmadığı bir bellek bölgesindedir, bu yüzden önce yeni seviye 3, seviye 2 ve seviye 1 sayfa tabloları oluşturmamız gerekir.

`create_example_mapping` fonksiyonumuzu `EmptyFrameAllocator` ile çağırmak için, tüm sayfa tablolarının zaten var olduğu bir sayfa seçmemiz gerekir. Böyle bir sayfa bulmak için, bootloader'ın kendisini sanal adres alanının ilk megabaytına yüklemesi gerçeğinden yararlanabiliriz. Bu, bu bölgedeki tüm sayfalar için geçerli bir seviye 1 tablosunun var olduğu anlamına gelir. Böylece, örnek eşlememiz için bu bellek bölgesindeki herhangi bir kullanılmamış sayfayı, örneğin `0` adresindeki sayfayı seçebiliriz. Normalde, bu sayfa, bir null işaretçinin dereference edilmesinin bir page fault'a neden olmasını garanti etmek için kullanılmamış kalmalıdır, bu yüzden bootloader'ın onu eşlenmemiş bıraktığını biliyoruz.

#### Eşlemeyi Oluşturmak

Artık `create_example_mapping` fonksiyonumuzu çağırmak için gereken tüm parametrelere sahibiz, bu yüzden `kernel_main` fonksiyonumuzu sanal adres `0`'daki sayfayı eşleyecek şekilde değiştirelim. Sayfayı VGA metin arabelleğinin frame'ine eşlediğimiz için, sonrasında onun aracılığıyla ekrana yazabilmemiz gerekir. Uygulama şöyle görünür:

```rust
// src/main.rs içinde

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory;
    use x86_64::{structures::paging::Page, VirtAddr}; // yeni içe aktarma

    […] // hello world ve blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = memory::EmptyFrameAllocator;

    // kullanılmamış bir sayfa eşle
    let page = Page::containing_address(VirtAddr::new(0));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    // yeni eşleme aracılığıyla ekrana `New!` dizesini yaz
    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e)};

    […] // test_main(), "it did not crash" yazdırma ve hlt_loop()
}
```

Önce, `mapper` ve `frame_allocator` örneklerine değiştirilebilir bir referansla `create_example_mapping` fonksiyonumuzu çağırarak `0` adresindeki sayfa için eşlemeyi oluşturuyoruz. Bu, sayfayı VGA metin arabelleği frame'ine eşler, bu yüzden ona yapılan herhangi bir yazmayı ekranda görmeliyiz.

Ardından sayfayı bir ham işaretçiye dönüştürüyor ve `400` ofsetine bir değer yazıyoruz. Sayfanın başına yazmıyoruz, çünkü VGA arabelleğinin en üst satırı bir sonraki `println` tarafından doğrudan ekrandan dışarı kaydırılır. Beyaz bir arka planda _"New!"_ dizesini temsil eden `0x_f021_f077_f065_f04e` değerini yazıyoruz. [_"VGA Metin Modu"_ yazısında][in the _“VGA Text Mode”_ post] öğrendiğimiz gibi, VGA arabelleğine yapılan yazmalar volatile olmalıdır, bu yüzden [`write_volatile`] metodunu kullanıyoruz.

[in the _“VGA Text Mode”_ post]: @/edition-2/posts/03-vga-text-buffer/index.tr.md#volatile
[`write_volatile`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write_volatile

Onu QEMU'da çalıştırdığımızda, aşağıdaki çıktıyı görüyoruz:

![Ekranın ortasında dört tamamen beyaz hücreyle "It did not crash!" yazdıran QEMU](qemu-new-mapping.png)

Ekrandaki _"New!"_, sayfa `0`'a yapılan yazmamızdan kaynaklanır; bu da sayfa tablolarında başarıyla yeni bir eşleme oluşturduğumuz anlamına gelir.

O eşlemeyi oluşturmak yalnızca, `0` adresindeki sayfadan sorumlu seviye 1 tablosu zaten var olduğu için çalıştı. Henüz hiçbir seviye 1 tablosunun var olmadığı bir sayfayı eşlemeye çalıştığımızda, `map_to` fonksiyonu başarısız olur; çünkü `EmptyFrameAllocator` ile frame'ler ayırarak yeni sayfa tabloları oluşturmaya çalışır. Bunun, `0` yerine `0xdeadbeaf000` sayfasını eşlemeye çalıştığımızda olduğunu görebiliriz:

```rust
// src/main.rs içinde

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    […]
    let page = Page::containing_address(VirtAddr::new(0xdeadbeaf000));
    […]
}
```

Onu çalıştırdığımızda, aşağıdaki hata mesajıyla bir panic oluşur:

```
panicked at 'map_to failed: FrameAllocationFailed', /…/result.rs:999:5
```

Henüz seviye 1 sayfa tablosu olmayan sayfaları eşlemek için, düzgün bir `FrameAllocator` oluşturmamız gerekir. Peki hangi frame'lerin kullanılmamış olduğunu ve ne kadar fiziksel bellek mevcut olduğunu nasıl biliriz?

### Frame'leri Ayırmak

Yeni sayfa tabloları oluşturmak için, düzgün bir frame allocator oluşturmamız gerekir. Bunu yapmak için, bootloader tarafından `BootInfo` struct'ının bir parçası olarak geçirilen `memory_map`'i kullanıyoruz:

```rust
// src/memory.rs içinde

use bootloader::bootinfo::MemoryMap;

/// Bootloader'ın bellek haritasından kullanılabilir frame'ler döndüren bir FrameAllocator.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Geçirilen bellek haritasından bir FrameAllocator oluşturur.
    ///
    /// Bu fonksiyon unsafe'tir, çünkü çağıranın geçirilen bellek haritasının
    /// geçerli olduğunu garanti etmesi gerekir. Ana gereksinim, onda `USABLE`
    /// olarak işaretlenmiş tüm frame'lerin gerçekten kullanılmamış olmasıdır.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }
}
```

Struct'ın iki alanı vardır: Bootloader tarafından geçirilen bellek haritasına `'static` bir referans ve allocator'ın döndürmesi gereken bir sonraki frame'in numarasını takip eden bir `next` alanı.

[_Önyükleme Bilgisi_](#boot-information) bölümünde açıkladığımız gibi, bellek haritası BIOS/UEFI firmware'i tarafından sağlanır. Yalnızca önyükleme sürecinin çok erken aşamasında sorgulanabilir, bu yüzden bootloader ilgili fonksiyonları zaten bizim için çağırır. Bellek haritası, her bellek bölgesinin başlangıç adresini, uzunluğunu ve tipini (örneğin kullanılmamış, ayrılmış vb.) içeren [`MemoryRegion`] struct'larından oluşan bir listeden oluşur.

`init` fonksiyonu, verilen bir bellek haritasıyla bir `BootInfoFrameAllocator` başlatır. `next` alanı `0` ile başlatılır ve aynı frame'i iki kez döndürmekten kaçınmak için her frame ayırmada artırılır. Bellek haritasının kullanılabilir frame'lerinin başka bir yerde zaten kullanılıp kullanılmadığını bilmediğimiz için, `init` fonksiyonumuz çağırandan ek garantiler istemek üzere `unsafe` olmalıdır.

#### Bir `usable_frames` Metodu

`FrameAllocator` trait'ini uygulamadan önce, bellek haritasını kullanılabilir frame'lerden oluşan bir iterator'a dönüştüren yardımcı bir metot ekliyoruz:

```rust
// src/memory.rs içinde

use bootloader::bootinfo::MemoryRegionType;

impl BootInfoFrameAllocator {
    /// Bellek haritasında belirtilen kullanılabilir frame'ler üzerinde bir iterator döndürür.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // bellek haritasından kullanılabilir bölgeleri al
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        // her bölgeyi adres aralığına eşle
        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());
        // frame başlangıç adreslerinden oluşan bir iterator'a dönüştür
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // başlangıç adreslerinden `PhysFrame` tipleri oluştur
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}
```

Bu fonksiyon, başlangıçtaki `MemoryMap`'i kullanılabilir fiziksel frame'lerden oluşan bir iterator'a dönüştürmek için iterator kombinatör metotlarını kullanır:

- İlk olarak, bellek haritasını [`MemoryRegion`]'lardan oluşan bir iterator'a dönüştürmek için `iter` metodunu çağırıyoruz.
- Ardından, herhangi bir ayrılmış veya başka şekilde kullanılamaz bölgeyi atlamak için [`filter`] metodunu kullanıyoruz. Bootloader, oluşturduğu tüm eşlemeler için bellek haritasını günceller, bu yüzden kernel'imiz tarafından kullanılan (kod, veri veya stack) ya da önyükleme bilgisini saklamak için kullanılan frame'ler zaten `InUse` veya benzeri olarak işaretlenmiştir. Böylece, `Usable` frame'lerin başka bir yerde kullanılmadığından emin olabiliriz.
- Sonrasında, bellek bölgelerinden oluşan iterator'ımızı adres aralıklarından oluşan bir iterator'a dönüştürmek için [`map`] kombinatörünü ve Rust'ın [aralık söz dizimini][range syntax] kullanıyoruz.
- Ardından, adres aralıklarını frame başlangıç adreslerinden oluşan bir iterator'a dönüştürmek için [`flat_map`]'i kullanıyor, [`step_by`] kullanarak her 4096. adresi seçiyoruz. 4096 bayt (= 4&nbsp;KiB) sayfa boyutu olduğundan, her frame'in başlangıç adresini elde ederiz. Bootloader tüm kullanılabilir bellek alanlarını sayfa hizalı yapar, bu yüzden burada herhangi bir hizalama veya yuvarlama koduna ihtiyacımız yok. `map` yerine [`flat_map`] kullanarak, bir `Iterator<Item = Iterator<Item = u64>>` yerine bir `Iterator<Item = u64>` elde ederiz.
- Son olarak, bir `Iterator<Item = PhysFrame>` oluşturmak için başlangıç adreslerini `PhysFrame` tiplerine dönüştürürüz.

[`MemoryRegion`]: https://docs.rs/bootloader/0.6.4/bootloader/bootinfo/struct.MemoryRegion.html
[`filter`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.filter
[`map`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.map
[range syntax]: https://doc.rust-lang.org/core/ops/struct.Range.html
[`step_by`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.step_by
[`flat_map`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.flat_map

Fonksiyonun dönüş tipi [`impl Trait`] özelliğini kullanır. Bu sayede, [`Iterator`] trait'ini `PhysFrame` öğe tipiyle uygulayan bir tip döndürdüğümüzü belirtebilir, ancak somut dönüş tipini adlandırmamız gerekmez. Burada bu önemlidir, çünkü somut tip adlandırılamayan closure tiplerine bağlı olduğu için onu adlandırama_yız_.

[`impl Trait`]: https://doc.rust-lang.org/book/ch10-02-traits.html#returning-types-that-implement-traits
[`Iterator`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html

#### `FrameAllocator` Trait'ini Uygulamak

Artık `FrameAllocator` trait'ini uygulayabiliriz:

```rust
// src/memory.rs içinde

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
```

İlk olarak, bellek haritasından kullanılabilir frame'lerden oluşan bir iterator almak için `usable_frames` metodunu kullanıyoruz. Ardından, `self.next` indeksli frame'i almak (böylece `(self.next - 1)` frame'i atlamak) için [`Iterator::nth`] fonksiyonunu kullanıyoruz. O frame'i döndürmeden önce, bir sonraki çağrıda izleyen frame'i döndürmemiz için `self.next`'i bir artırıyoruz.

[`Iterator::nth`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.nth

Bu uygulama pek optimal değildir, çünkü `usable_frame` allocator'ını her ayırmada yeniden oluşturur. Bunun yerine iterator'ı doğrudan bir struct alanı olarak saklamak daha iyi olurdu. O zaman `nth` metoduna ihtiyacımız olmaz ve her ayırmada yalnızca [`next`] çağırabilirdik. Bu yaklaşımın sorunu, şu anda bir struct alanında bir `impl Trait` tipi saklamanın mümkün olmamasıdır. [_Adlandırılmış varoluşsal tipler (named existential types)_] tam olarak uygulandığında bir gün işe yarayabilir.

[`next`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#tymethod.next
[_named existential types_]: https://github.com/rust-lang/rfcs/pull/2071

#### `BootInfoFrameAllocator`'ı Kullanmak

Artık `kernel_main` fonksiyonumuzu, bir `EmptyFrameAllocator` yerine bir `BootInfoFrameAllocator` örneği geçirecek şekilde değiştirebiliriz:

```rust
// src/main.rs içinde

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory::BootInfoFrameAllocator;
    […]
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    […]
}
```

Boot info frame allocator ile, eşleme başarılı olur ve ekranda yine beyaz üzerine siyah _"New!"_'i görürüz. Perde arkasında, `map_to` metodu eksik sayfa tablolarını şu şekilde oluşturur:

- Kullanılmamış bir frame ayırmak için geçirilen `frame_allocator`'ı kullan.
- Yeni, boş bir sayfa tablosu oluşturmak için frame'i sıfırla.
- Daha yüksek seviye tablonun girdisini o frame'e eşle.
- Bir sonraki tablo seviyesiyle devam et.

`create_example_mapping` fonksiyonumuz yalnızca bazı örnek kod olsa da, artık keyfi sayfalar için yeni eşlemeler oluşturabiliyoruz. Bu, gelecekteki yazılarda bellek ayırmak veya çoklu thread (multithreading) uygulamak için olmazsa olmaz olacaktır.

Bu noktada, yanlışlıkla tanımsız davranışa yol açmaktan kaçınmak için, [yukarıda](#a-create-example-mapping-function) açıklandığı gibi `create_example_mapping` fonksiyonunu tekrar silmeliyiz.

## Özet

Bu yazıda, sayfa tablolarının fiziksel frame'lerine erişmeye yönelik farklı teknikleri öğrendik; kimlik eşleme, tüm fiziksel belleğin eşlenmesi, geçici eşleme ve özyinelemeli sayfa tabloları dahil. Basit, taşınabilir ve güçlü olduğu için tüm fiziksel belleği eşlemeyi seçtik.

Sayfa tablosu erişimi olmadan fiziksel belleği kernel'imizden eşleyemeyiz, bu yüzden bootloader'dan desteğe ihtiyacımız var. `bootloader` crate'i, gereken eşlemeyi isteğe bağlı cargo crate özellikleri aracılığıyla oluşturmayı destekler. Gereken bilgiyi kernel'imize, giriş noktası fonksiyonumuza bir `&BootInfo` argümanı biçiminde geçirir.

Uygulamamız için, önce bir çeviri fonksiyonu uygulamak amacıyla sayfa tablolarında elle dolaştık ve ardından `x86_64` crate'inin `MappedPageTable` tipini kullandık. Ayrıca sayfa tablosunda nasıl yeni eşlemeler oluşturacağımızı ve bootloader tarafından geçirilen bellek haritasının üzerine gerekli `FrameAllocator`'ı nasıl oluşturacağımızı öğrendik.

## Sırada ne var?

Bir sonraki yazı, kernel'imiz için bir heap bellek bölgesi oluşturacak; bu da [bellek ayırmamıza][allocate memory] ve çeşitli [koleksiyon tiplerini][collection types] kullanmamıza olanak tanıyacak.

[allocate memory]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html
[collection types]: https://doc.rust-lang.org/alloc/collections/index.html
