+++
title = "Paging'e Giriş"
weight = 8
path = "tr/paging-introduction"
date = 2019-01-14

[extra]
chapter = "Memory Management"

# Please update this when updating the translation
translation_based_on_commit = "9753695744854686a6b80012c89b0d850a44b4b0"

# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

Bu yazı, işletim sistemimiz için de kullanacağımız çok yaygın bir bellek yönetimi şeması olan _paging_'i tanıtıyor. Bellek yalıtımının neden gerekli olduğunu, _segmentasyonun_ nasıl çalıştığını, _sanal belleğin_ ne olduğunu ve paging'in bellek parçalanması sorunlarını nasıl çözdüğünü açıklıyor. Ayrıca x86_64 mimarisindeki çok seviyeli sayfa tablolarının düzenini de inceliyor.

<!-- more -->

Bu blog [GitHub] üzerinde açık biçimde geliştirilmektedir. Herhangi bir sorun veya sorunuz varsa lütfen orada bir issue açın. Ayrıca [sayfanın en altına][at the bottom] yorum bırakabilirsiniz. Bu yazının eksiksiz kaynak kodu [`post-08`][post branch] dalında bulunabilir.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-08

<!-- toc -->

## Bellek Koruması

Bir işletim sisteminin ana görevlerinden biri, programları birbirinden yalıtmaktır. Örneğin, web tarayıcınızın metin düzenleyicinize müdahale edebilmesi gerekmez. Bu hedefe ulaşmak için, işletim sistemleri bir sürecin bellek alanlarına diğer süreçler tarafından erişilememesini sağlamak amacıyla donanım işlevselliğinden yararlanır. Donanıma ve OS uygulamasına bağlı olarak farklı yaklaşımlar vardır.

Örnek olarak, bazı ARM Cortex-M işlemcilerin (gömülü sistemler için kullanılır) bir [_Memory Protection Unit_] (MPU) vardır; bu, farklı erişim izinlerine (örneğin erişim yok, salt okunur, okuma-yazma) sahip az sayıda (örneğin 8) bellek bölgesi tanımlamanıza olanak tanır. Her bellek erişiminde, MPU adresin doğru erişim izinlerine sahip bir bölgede olduğundan emin olur ve aksi takdirde bir exception fırlatır. Her süreç değişiminde bölgeleri ve erişim izinlerini değiştirerek, işletim sistemi her sürecin yalnızca kendi belleğine eriştiğinden emin olabilir ve böylece süreçleri birbirinden yalıtır.

[_Memory Protection Unit_]: https://developer.arm.com/docs/ddi0337/e/memory-protection-unit/about-the-mpu

x86'da donanım, bellek korumasına yönelik iki farklı yaklaşımı destekler: [segmentasyon][segmentation] ve [paging].

[segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation
[paging]: https://en.wikipedia.org/wiki/Virtual_memory#Paged_virtual_memory

## Segmentasyon

Segmentasyon, 1978'de, başlangıçta adreslenebilir bellek miktarını artırmak için tanıtıldı. O zamanki durum, CPU'ların yalnızca 16-bit adresler kullanması ve bunun adreslenebilir bellek miktarını 64&nbsp;KiB ile sınırlamasıydı. Bu 64&nbsp;KiB'tan fazlasını erişilebilir kılmak için, her biri bir ofset adres içeren ek segment register'ları tanıtıldı. CPU bu ofseti her bellek erişiminde otomatik olarak ekledi; böylece 1&nbsp;MiB'a kadar bellek erişilebilir oldu.

Segment register'ı, bellek erişiminin türüne bağlı olarak CPU tarafından otomatik olarak seçilir: Komutları getirmek için kod segmenti `CS` kullanılır ve stack işlemleri (push/pop) için stack segmenti `SS` kullanılır. Diğer komutlar veri segmenti `DS`'yi veya ekstra segment `ES`'yi kullanır. Daha sonra, serbestçe kullanılabilen iki ek segment register'ı, `FS` ve `GS` eklendi.

Segmentasyonun ilk sürümünde, segment register'ları doğrudan ofseti içeriyordu ve hiçbir erişim denetimi yapılmıyordu. Bu daha sonra [_protected mode_]'un tanıtılmasıyla değiştirildi. CPU bu modda çalıştığında, segment tanımlayıcıları yerel veya global bir [_tanımlayıcı tabloya (descriptor table)_][_descriptor table_] bir indeks içerir; bu tablo da – bir ofset adrese ek olarak – segment boyutunu ve erişim izinlerini içerir. Bellek erişimlerini sürecin kendi bellek alanlarıyla sınırlayan ayrı global/yerel tanımlayıcı tabloları her süreç için yükleyerek, OS süreçleri birbirinden yalıtabilir.

[_protected mode_]: https://en.wikipedia.org/wiki/X86_memory_segmentation#Protected_mode
[_descriptor table_]: https://en.wikipedia.org/wiki/Global_Descriptor_Table

Bellek adreslerini gerçek erişimden önce değiştirerek, segmentasyon halihazırda artık neredeyse her yerde kullanılan bir tekniği kullanıyordu: _sanal bellek_.

### Sanal Bellek

Sanal belleğin arkasındaki fikir, bellek adreslerini alttaki fiziksel depolama cihazından soyutlamaktır. Depolama cihazına doğrudan erişmek yerine, önce bir çeviri adımı gerçekleştirilir. Segmentasyon için çeviri adımı, aktif segmentin ofset adresini eklemektir. Ofseti `0x1111000` olan bir segmentte `0x1234000` bellek adresine erişen bir program hayal edin: Gerçekte erişilen adres `0x2345000`'dir.

İki adres tipini ayırt etmek için, çeviriden önceki adreslere _sanal_ ve çeviriden sonraki adreslere _fiziksel_ denir. Bu iki tür adres arasındaki önemli bir fark, fiziksel adreslerin benzersiz olması ve her zaman aynı belirgin bellek konumuna atıfta bulunmasıdır. Sanal adresler ise çeviri fonksiyonuna bağlıdır. İki farklı sanal adresin aynı fiziksel adrese atıfta bulunması tamamen mümkündür. Ayrıca, aynı sanal adresler farklı çeviri fonksiyonları kullandıklarında farklı fiziksel adreslere atıfta bulunabilir.

Bu özelliğin yararlı olduğu bir örnek, aynı programı paralel olarak iki kez çalıştırmaktır:


![0–150 adresli iki sanal adres alanı, biri 100–250'ye, diğeri 300–450'ye çevrilir](segmentation-same-program-twice.svg)

Burada aynı program iki kez, ancak farklı çeviri fonksiyonlarıyla çalışır. İlk örneğin segment ofseti 100'dür; böylece 0–150 sanal adresleri 100–250 fiziksel adreslerine çevrilir. İkinci örneğin ofseti 300'dür; bu da 0–150 sanal adreslerini 300–450 fiziksel adreslerine çevirir. Bu, her iki programın da aynı kodu çalıştırmasına ve birbirine müdahale etmeden aynı sanal adresleri kullanmasına olanak tanır.

Başka bir avantaj, programların artık tamamen farklı sanal adresler kullansalar bile keyfi fiziksel bellek konumlarına yerleştirilebilmesidir. Böylece OS, programları yeniden derlemeye gerek kalmadan mevcut belleğin tamamından yararlanabilir.

### Parçalanma (Fragmentation) {#fragmentation}

Sanal ve fiziksel adresler arasındaki ayrım, segmentasyonu gerçekten güçlü kılar. Ancak parçalanma (fragmentation) sorunu vardır. Örnek olarak, yukarıda gördüğümüz programın üçüncü bir kopyasını çalıştırmak istediğimizi hayal edin:

![Üç sanal adres alanı, ancak üçüncüsü için yeterli sürekli alan yok](segmentation-fragmentation.svg)

Yeterinden fazla boş bellek mevcut olmasına rağmen, programın üçüncü örneğini sanal belleğe çakışmadan eşlemenin bir yolu yoktur. Sorun, _sürekli_ belleğe ihtiyaç duymamız ve küçük boş parçaları kullanamamamızdır.

Bu parçalanmayla mücadele etmenin bir yolu, çalıştırmayı duraklatmak, belleğin kullanılan kısımlarını birbirine yaklaştırmak, çeviriyi güncellemek ve ardından çalıştırmaya devam etmektir:

![Birleştirme (defragmentation) sonrası üç sanal adres alanı](segmentation-fragmentation-compacted.svg)

Artık programımızın üçüncü örneğini başlatmak için yeterli sürekli alan var.

Bu birleştirme (defragmentation) sürecinin dezavantajı, performansı düşüren büyük miktarda belleği kopyalaması gerekmesidir. Ayrıca, bellek çok parçalanmadan önce düzenli olarak yapılması gerekir. Bu, programlar rastgele zamanlarda duraklatıldığı ve yanıt vermez hale gelebileceği için performansı öngörülemez kılar.

Parçalanma sorunu, segmentasyonun çoğu sistem tarafından artık kullanılmamasının nedenlerinden biridir. Aslında, segmentasyon x86'da 64-bit modda artık desteklenmiyor bile. Bunun yerine, parçalanma sorunundan tamamen kaçınan _paging_ kullanılır.

## Paging

Fikir, hem sanal hem de fiziksel bellek alanını küçük, sabit boyutlu bloklara bölmektir. Sanal bellek alanının bloklarına _sayfa (page)_, fiziksel adres alanının bloklarına ise _frame_ denir. Her sayfa ayrı ayrı bir frame'e eşlenebilir; bu da daha büyük bellek bölgelerini sürekli olmayan fiziksel frame'lere bölmeyi mümkün kılar.

Bunun avantajı, parçalanmış bellek alanı örneğini tekrar gözden geçirir, ancak bu kez segmentasyon yerine paging kullanırsak görünür hale gelir:

![Paging ile, üçüncü program örneği birçok küçük fiziksel alana bölünebilir.](paging-fragmentation.svg)

Bu örnekte 50 baytlık bir sayfa boyutuna sahibiz; bu da bellek bölgelerimizin her birinin üç sayfaya bölündüğü anlamına gelir. Her sayfa ayrı ayrı bir frame'e eşlenir, bu yüzden sürekli bir sanal bellek bölgesi sürekli olmayan fiziksel frame'lere eşlenebilir. Bu, programın üçüncü örneğini öncesinde herhangi bir birleştirme yapmadan başlatmamıza olanak tanır.

### Gizli Parçalanma

Segmentasyona kıyasla paging, birkaç büyük, değişken boyutlu bölge yerine birçok küçük, sabit boyutlu bellek bölgesi kullanır. Her frame aynı boyuta sahip olduğundan, kullanılamayacak kadar küçük olan hiçbir frame yoktur, bu yüzden hiçbir parçalanma meydana gelmez.

Ya da hiçbir parçalanma meydana gelmiyor _gibi görünür_. Hâlâ bir tür gizli parçalanma vardır; _iç parçalanma (internal fragmentation)_ adı verilen şey. İç parçalanma, her bellek bölgesinin sayfa boyutunun tam bir katı olmaması nedeniyle meydana gelir. Yukarıdaki örnekte 101 boyutlu bir program hayal edin: Yine de 50 boyutlu üç sayfaya ihtiyaç duyardı, bu yüzden gerekenden 49 bayt daha fazla yer kaplardı. İki tür parçalanmayı ayırt etmek için, segmentasyon kullanılırken meydana gelen parçalanma türüne _dış parçalanma (external fragmentation)_ denir.

İç parçalanma talihsiz bir durumdur, ancak genellikle segmentasyonda meydana gelen dış parçalanmadan daha iyidir. Yine de bellek israf eder, ancak birleştirme gerektirmez ve parçalanma miktarını öngörülebilir kılar (ortalama olarak bellek bölgesi başına yarım sayfa).

### Sayfa Tabloları

Potansiyel olarak milyonlarca sayfanın her birinin ayrı ayrı bir frame'e eşlendiğini gördük. Bu eşleme bilgisinin bir yerde saklanması gerekir. Segmentasyon, her aktif bellek bölgesi için ayrı bir segment seçici register'ı kullanır; bu, register'lardan çok daha fazla sayfa olduğu için paging için mümkün değildir. Bunun yerine paging, eşleme bilgisini saklamak için _sayfa tablosu (page table)_ adı verilen bir tablo yapısı kullanır.

Yukarıdaki örneğimiz için sayfa tabloları şöyle görünürdü:

![Üç sayfa tablosu, her program örneği için bir tane. Örnek 1 için eşleme 0->100, 50->150, 100->200'dür. Örnek 2 için 0->300, 50->350, 100->400'dür. Örnek 3 için 0->250, 50->450, 100->500'dür.](paging-page-tables.svg)

Her program örneğinin kendi sayfa tablosuna sahip olduğunu görüyoruz. Şu anda aktif olan tabloya bir işaretçi, özel bir CPU register'ında saklanır. `x86`'da bu register'ın adı `CR3`'tür. Her program örneğini çalıştırmadan önce bu register'a doğru sayfa tablosuna işaretçiyi yüklemek işletim sisteminin görevidir.

Her bellek erişiminde, CPU tablo işaretçisini register'dan okur ve erişilen sayfa için eşlenmiş frame'i tabloda arar. Bu tamamen donanımda yapılır ve çalışan programa tamamen görünmezdir. Çeviri sürecini hızlandırmak için, birçok CPU mimarisinin son çevirilerin sonuçlarını hatırlayan özel bir önbelleği vardır.

Mimariye bağlı olarak, sayfa tablosu girdileri bir bayraklar (flags) alanında erişim izinleri gibi öznitelikleri de saklayabilir. Yukarıdaki örnekte, "r/w" bayrağı sayfayı hem okunabilir hem yazılabilir kılar.

### Çok Seviyeli Sayfa Tabloları

Az önce gördüğümüz basit sayfa tablolarının daha büyük adres alanlarında bir sorunu vardır: bellek israf ederler. Örneğin, `0`, `1_000_000`, `1_000_050` ve `1_000_100` olmak üzere dört sanal sayfa kullanan bir program hayal edin (`_`'yi binlik ayırıcı olarak kullanıyoruz):

![Sayfa 0 frame 0'a ve `1_000_000`–`1_000_150` sayfaları frame 100–250'ye eşlenmiş](single-level-page-table.svg)

Yalnızca 4 fiziksel frame'e ihtiyaç duyar, ancak sayfa tablosunun bir milyondan fazla girdisi vardır. Boş girdileri atlayamayız, çünkü o zaman CPU çeviri sürecinde doğru girdiye doğrudan atlayamaz (örneğin, dördüncü sayfanın dördüncü girdiyi kullandığı artık garanti edilmez).

İsraf edilen belleği azaltmak için bir **iki seviyeli sayfa tablosu** kullanabiliriz. Fikir, farklı adres bölgeleri için farklı sayfa tabloları kullanmamızdır. _Seviye 2_ sayfa tablosu adı verilen ek bir tablo, adres bölgeleri ile (seviye 1) sayfa tabloları arasındaki eşlemeyi içerir.

Bu en iyi bir örnekle açıklanır. Her seviye 1 sayfa tablosunun `10_000` boyutunda bir bölgeden sorumlu olduğunu tanımlayalım. O zaman yukarıdaki örnek eşleme için aşağıdaki tablolar var olurdu:

![Sayfa 0, seviye 2 sayfa tablosunun 0. girdisine işaret eder; o da seviye 1 sayfa tablosu T1'e işaret eder. T1'in ilk girdisi frame 0'a işaret eder; diğer girdiler boştur. `1_000_000`–`1_000_150` sayfaları, seviye 2 sayfa tablosunun 100. girdisine işaret eder; o da farklı bir seviye 1 sayfa tablosu T2'ye işaret eder. T2'nin ilk üç girdisi frame 100–250'ye işaret eder; diğer girdiler boştur.](multilevel-page-table.svg)

Sayfa 0, ilk `10_000` baytlık bölgeye düşer, bu yüzden seviye 2 sayfa tablosunun ilk girdisini kullanır. Bu girdi, sayfa `0`'ın frame `0`'a işaret ettiğini belirten seviye 1 sayfa tablosu T1'e işaret eder.

`1_000_000`, `1_000_050` ve `1_000_100` sayfalarının hepsi 100. `10_000` baytlık bölgeye düşer, bu yüzden seviye 2 sayfa tablosunun 100. girdisini kullanırlar. Bu girdi, üç sayfayı frame `100`, `150` ve `200`'e eşleyen farklı bir seviye 1 sayfa tablosu T2'ye işaret eder. Seviye 1 tablolardaki sayfa adresinin bölge ofsetini içermediğine dikkat edin. Örneğin, sayfa `1_000_050` için girdi yalnızca `50`'dir.

Seviye 2 tablosunda hâlâ 100 boş girdimiz var, ancak önceki bir milyon boş girdiden çok daha az. Bu tasarrufun nedeni, `10_000` ile `1_000_000` arasındaki eşlenmemiş bellek bölgeleri için seviye 1 sayfa tabloları oluşturmamıza gerek olmamasıdır.

İki seviyeli sayfa tabloları ilkesi üç, dört veya daha fazla seviyeye genişletilebilir. O zaman sayfa tablosu register'ı en yüksek seviye tabloya işaret eder; o bir sonraki alt seviye tabloya, o bir sonraki alt seviyeye işaret eder ve bu böyle devam eder. Seviye 1 sayfa tablosu ise eşlenmiş frame'e işaret eder. İlke genel olarak _çok seviyeli (multilevel)_ veya _hiyerarşik_ sayfa tablosu olarak adlandırılır.

Artık paging'in ve çok seviyeli sayfa tablolarının nasıl çalıştığını bildiğimize göre, paging'in x86_64 mimarisinde nasıl uygulandığına bakabiliriz (aşağıda CPU'nun 64-bit modda çalıştığını varsayıyoruz).

## x86_64'te Paging {#paging-on-x86-64}

x86_64 mimarisi 4 seviyeli bir sayfa tablosu ve 4&nbsp;KiB'lık bir sayfa boyutu kullanır. Her sayfa tablosu, seviyeden bağımsız olarak, sabit 512 girdi boyutuna sahiptir. Her girdi 8 bayt boyutundadır, bu yüzden her tablo 512 * 8&nbsp;B = 4&nbsp;KiB büyüklüğündedir ve böylece tam olarak bir sayfaya sığar.

Her seviye için sayfa tablosu indeksi doğrudan sanal adresten türetilir:

![0–12 bitleri sayfa ofseti, 12–21 bitleri seviye 1 indeksi, 21–30 bitleri seviye 2 indeksi, 30–39 bitleri seviye 3 indeksi ve 39–48 bitleri seviye 4 indeksidir](x86_64-table-indices-from-address.svg)

Her tablo indeksinin 9 bitten oluştuğunu görüyoruz; bu mantıklıdır, çünkü her tablonun 2^9 = 512 girdisi vardır. En düşük 12 bit, 4&nbsp;KiB sayfadaki ofsettir (2^12 bayt = 4&nbsp;KiB). 48'den 64'e kadar olan bitler atılır; bu da x86_64'ün yalnızca 48-bit adresleri desteklediği için aslında gerçekten 64-bit olmadığı anlamına gelir.

48'den 64'e kadar olan bitler atılsa da, keyfi değerlere ayarlanamazlar. Bunun yerine, adresleri benzersiz tutmak ve 5 seviyeli sayfa tablosu gibi gelecekteki genişletmelere izin vermek için bu aralıktaki tüm bitler 47. bitin kopyaları olmalıdır. Buna _işaret genişletme (sign-extension)_ denir, çünkü [iki'ye tümleyendeki işaret genişletmeye][sign extension in two's complement] çok benzer. Bir adres doğru şekilde işaret genişletilmediğinde, CPU bir exception fırlatır.

[sign extension in two's complement]: https://en.wikipedia.org/wiki/Two's_complement#Sign_extension

Yakın tarihli "Ice Lake" Intel CPU'larının, sanal adresleri 48-bit'ten 57-bit'e genişletmek için isteğe bağlı olarak [5 seviyeli sayfa tablolarını][5-level page tables] desteklediğini belirtmekte fayda var. Kernel'imizi belirli bir CPU için optimize etmenin bu aşamada anlamlı olmadığı düşünüldüğünde, bu yazıda yalnızca standart 4 seviyeli sayfa tablolarıyla çalışacağız.

[5-level page tables]: https://en.wikipedia.org/wiki/Intel_5-level_paging

### Örnek Çeviri

Çeviri sürecinin ayrıntılı olarak nasıl çalıştığını anlamak için bir örnek üzerinden gidelim:

![Her sayfa tablosu fiziksel bellekte gösterilen 4 seviyeli bir sayfa hiyerarşisi örneği](x86_64-page-table-translation.svg)

4 seviyeli sayfa tablosunun kökü olan, şu anda aktif olan seviye 4 sayfa tablosunun fiziksel adresi `CR3` register'ında saklanır. Her sayfa tablosu girdisi daha sonra bir sonraki seviye tablonun fiziksel frame'ine işaret eder. Seviye 1 tablosunun girdisi ise eşlenmiş frame'e işaret eder. Sayfa tablolarındaki tüm adreslerin sanal değil fiziksel olduğuna dikkat edin; çünkü aksi takdirde CPU'nun bu adresleri de çevirmesi gerekirdi (bu da bitmeyen bir özyinelemeye neden olabilir).

Yukarıdaki sayfa tablosu hiyerarşisi iki sayfayı (mavi renkte) eşler. Sayfa tablosu indekslerinden, bu iki sayfanın sanal adreslerinin `0x803FE7F000` ve `0x803FE00000` olduğunu çıkarabiliriz. Program `0x803FE7F5CE` adresinden okumaya çalıştığında ne olduğunu görelim. Önce adresi ikiliye çeviriyor ve adres için sayfa tablosu indekslerini ve sayfa ofsetini belirliyoruz:

![İşaret genişletme bitlerinin hepsi 0, seviye 4 indeksi 1, seviye 3 indeksi 0, seviye 2 indeksi 511, seviye 1 indeksi 127 ve sayfa ofseti 0x5ce'dir](x86_64-page-table-translation-addresses.png)

Bu indekslerle, artık adres için eşlenmiş frame'i belirlemek üzere sayfa tablosu hiyerarşisinde yürüyebiliriz:

- Seviye 4 tablosunun adresini `CR3` register'ından okuyarak başlıyoruz.
- Seviye 4 indeksi 1'dir, bu yüzden o tablonun 1 indeksli girdisine bakarız; bu da bize seviye 3 tablosunun 16&nbsp;KiB adresinde saklandığını söyler.
- Seviye 3 tablosunu o adresten yükler ve 0 indeksli girdiye bakarız; bu da bizi 24&nbsp;KiB'taki seviye 2 tablosuna yönlendirir.
- Seviye 2 indeksi 511'dir, bu yüzden seviye 1 tablosunun adresini öğrenmek için o sayfanın son girdisine bakarız.
- Seviye 1 tablosunun 127 indeksli girdisi aracılığıyla, sonunda sayfanın 12&nbsp;KiB'lık frame'e, ya da onaltılıkta 0x3000'e eşlendiğini öğreniriz.
- Son adım, fiziksel adresi elde etmek için sayfa ofsetini frame adresine eklemektir: 0x3000 + 0x5ce = 0x35ce.

![5 ek ok içeren aynı örnek 4 seviyeli sayfa hiyerarşisi: CR3 register'ından seviye 4 tablosuna "Adım 0", seviye 4 girdisinden seviye 3 tablosuna "Adım 1", seviye 3 girdisinden seviye 2 tablosuna "Adım 2", seviye 2 girdisinden seviye 1 tablosuna "Adım 3" ve seviye 1 tablosundan eşlenmiş frame'lere "Adım 4".](x86_64-page-table-translation-steps.svg)

Seviye 1 tablosundaki sayfanın izinleri `r`'dir; bu da salt okunur anlamına gelir. Donanım bu izinleri zorunlu kılar ve o sayfaya yazmaya çalışırsak bir exception fırlatırdı. Daha yüksek seviye sayfalardaki izinler, daha düşük seviyelerdeki olası izinleri kısıtlar; yani seviye 3 girdisini salt okunur olarak ayarlarsak, alt seviyeler okuma/yazma izinleri belirtse bile bu girdiyi kullanan hiçbir sayfa yazılabilir olamaz.

Bu örnekte her tablonun yalnızca tek bir örneği kullanılmış olsa da, her adres alanında tipik olarak her seviyeden birden çok örnek olduğunu belirtmek önemlidir. En fazla şunlar vardır:

- bir seviye 4 tablosu,
- 512 seviye 3 tablosu (çünkü seviye 4 tablosunun 512 girdisi vardır),
- 512 * 512 seviye 2 tablosu (çünkü 512 seviye 3 tablosunun her birinin 512 girdisi vardır) ve
- 512 * 512 * 512 seviye 1 tablosu (her seviye 2 tablosu için 512 girdi).

### Sayfa Tablosu Biçimi {#page-table-format}

x86_64 mimarisindeki sayfa tabloları temelde 512 girdiden oluşan bir dizidir. Rust söz dizimiyle:

```rust
#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}
```

`repr` özniteliğinin belirttiği gibi, sayfa tablolarının sayfa hizalı olması, yani 4&nbsp;KiB'lık bir sınırda hizalanması gerekir. Bu gereksinim, bir sayfa tablosunun her zaman tam bir sayfayı doldurmasını garanti eder ve girdileri çok kompakt kılan bir optimizasyona olanak tanır.

Her girdi 8 bayt (64 bit) büyüklüğündedir ve aşağıdaki biçime sahiptir:

Bit(ler) | Ad | Anlam
-------- | -- | -----
0 | present | sayfa şu anda bellekte
1 | writable | bu sayfaya yazmaya izin verilir
2 | user accessible | ayarlanmamışsa, bu sayfaya yalnızca kernel modu kodu erişebilir
3 | write-through caching | yazmalar doğrudan belleğe gider
4 | disable cache | bu sayfa için önbellek kullanılmaz
5 | accessed | bu sayfa kullanıldığında CPU bu biti ayarlar
6 | dirty | bu sayfaya bir yazma gerçekleştiğinde CPU bu biti ayarlar
7 | huge page/null | P1 ve P4'te 0 olmalıdır, P3'te 1&nbsp;GiB'lık bir sayfa oluşturur, P2'de 2&nbsp;MiB'lık bir sayfa oluşturur
8 | global | adres alanı değişiminde sayfa önbelleklerden temizlenmez (CR4 register'ının PGE biti ayarlı olmalıdır)
9-11 | available | OS tarafından serbestçe kullanılabilir
12-51 | physical address | frame'in veya bir sonraki sayfa tablosunun sayfa hizalı 52 bitlik fiziksel adresi
52-62 | available | OS tarafından serbestçe kullanılabilir
63 | no execute | bu sayfada kod çalıştırmayı yasakla (EFER register'ındaki NXE biti ayarlı olmalıdır)

Fiziksel frame adresini saklamak için yalnızca 12–51 bitlerinin kullanıldığını görüyoruz. Kalan bitler bayrak olarak kullanılır veya işletim sistemi tarafından serbestçe kullanılabilir. Bu mümkündür, çünkü her zaman 4096 baytlık hizalı bir adrese işaret ederiz; ya sayfa hizalı bir sayfa tablosuna ya da eşlenmiş bir frame'in başlangıcına. Bu, 0–11 bitlerinin her zaman sıfır olduğu anlamına gelir, bu yüzden bu bitleri saklamak için bir neden yoktur; çünkü donanım, adresi kullanmadan önce onları sıfıra ayarlayabilir. Aynı şey 52–63 bitleri için de geçerlidir, çünkü x86_64 mimarisi yalnızca 52-bit fiziksel adresleri destekler (yalnızca 48-bit sanal adresleri desteklemesine benzer şekilde).

Mevcut bayraklara daha yakından bakalım:

- `present` bayrağı, eşlenmiş sayfaları eşlenmemiş olanlardan ayırır. Ana bellek dolduğunda sayfaları geçici olarak diske takas etmek (swap out) için kullanılabilir. Sayfaya sonradan erişildiğinde, _page fault_ adı verilen özel bir exception meydana gelir; işletim sistemi buna eksik sayfayı diskten yeniden yükleyerek ve ardından programa devam ederek tepki verebilir.
- `writable` ve `no execute` bayrakları, sırasıyla sayfanın içeriğinin yazılabilir olup olmadığını veya çalıştırılabilir komutlar içerip içermediğini kontrol eder.
- `accessed` ve `dirty` bayrakları, sayfaya bir okuma veya yazma gerçekleştiğinde CPU tarafından otomatik olarak ayarlanır. Bu bilgiden işletim sistemi yararlanabilir; örneğin hangi sayfaların takas edileceğine veya sayfa içeriğinin son disk kaydından bu yana değiştirilip değiştirilmediğine karar vermek için.
- `write-through caching` ve `disable cache` bayrakları, her sayfa için önbelleklerin ayrı ayrı kontrol edilmesine olanak tanır.
- `user accessible` bayrağı, bir sayfayı kullanıcı alanı (userspace) koduna kullanılabilir kılar; aksi takdirde yalnızca CPU kernel modundayken erişilebilir. Bu özellik, bir kullanıcı alanı programı çalışırken kernel'i eşlenmiş tutarak [sistem çağrılarını (system calls)][system calls] daha hızlı yapmak için kullanılabilir. Ancak [Spectre] güvenlik açığı, kullanıcı alanı programlarının yine de bu sayfaları okumasına olanak tanıyabilir.
- `global` bayrağı, donanıma bir sayfanın tüm adres alanlarında kullanılabilir olduğunu ve bu yüzden adres alanı değişimlerinde çeviri önbelleğinden (aşağıdaki TLB hakkındaki bölüme bakın) kaldırılmasına gerek olmadığını bildirir. Bu bayrak, kernel kodunu tüm adres alanlarına eşlemek için genellikle temizlenmiş bir `user accessible` bayrağıyla birlikte kullanılır.
- `huge page` bayrağı, seviye 2 veya seviye 3 sayfa tablolarının girdilerinin doğrudan eşlenmiş bir frame'e işaret etmesine izin vererek daha büyük boyutlu sayfaların oluşturulmasına olanak tanır. Bu bit ayarlıyken, sayfa boyutu 512 faktörüyle artar: seviye 2 girdileri için ya 2&nbsp;MiB = 512 * 4&nbsp;KiB ya da seviye 3 girdileri için hatta 1&nbsp;GiB = 512 * 2&nbsp;MiB olur. Daha büyük sayfalar kullanmanın avantajı, çeviri önbelleğinin daha az satırına ve daha az sayfa tablosuna ihtiyaç duyulmasıdır.

[system calls]: https://en.wikipedia.org/wiki/System_call
[Spectre]: https://en.wikipedia.org/wiki/Spectre_(security_vulnerability)

`x86_64` crate'i [sayfa tabloları][page tables] ve [girdileri][entries] için tipler sağlar, bu yüzden bu yapıları kendimiz oluşturmamıza gerek yok.

[page tables]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page_table/struct.PageTable.html
[entries]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page_table/struct.PageTableEntry.html

### Translation Lookaside Buffer {#the-translation-lookaside-buffer}

4 seviyeli bir sayfa tablosu, sanal adreslerin çevirisini pahalı kılar; çünkü her çeviri dört bellek erişimi gerektirir. Performansı iyileştirmek için, x86_64 mimarisi son birkaç çeviriyi _translation lookaside buffer_ (TLB) adı verilen şeyde önbelleğe alır. Bu, çeviri hâlâ önbellekteyken çeviriyi atlamaya olanak tanır.

Diğer CPU önbelleklerinin aksine, TLB tamamen şeffaf değildir ve sayfa tablolarının içeriği değiştiğinde çevirileri güncellemez veya kaldırmaz. Bu, kernel'in bir sayfa tablosunu her değiştirdiğinde TLB'yi elle güncellemesi gerektiği anlamına gelir. Bunu yapmak için, belirtilen sayfanın çevirisini TLB'den kaldıran [`invlpg`] ("invalidate page") adı verilen özel bir CPU komutu vardır; böylece çeviri bir sonraki erişimde sayfa tablosundan yeniden yüklenir. TLB, bir adres alanı değişimini taklit eden `CR3` register'ı yeniden yüklenerek de tamamen temizlenebilir (flush). `x86_64` crate'i, her iki varyant için de [`tlb` modülünde][`tlb` module] Rust fonksiyonları sağlar.

[`invlpg`]: https://www.felixcloutier.com/x86/INVLPG.html
[`tlb` module]: https://docs.rs/x86_64/0.14.2/x86_64/instructions/tlb/index.html

Her sayfa tablosu değişikliğinde TLB'yi temizlemeyi (flush) hatırlamak önemlidir; çünkü aksi takdirde CPU eski çeviriyi kullanmaya devam edebilir, bu da hata ayıklaması çok zor olan belirsiz (non-deterministic) hatalara yol açabilir.

## Uygulama

Henüz değinmediğimiz bir şey: **Kernel'imiz zaten paging üzerinde çalışıyor**. ["Minimal Bir Rust Kernel'i"]["A minimal Rust kernel"] yazısında eklediğimiz bootloader, kernel'imizin her sayfasını bir fiziksel frame'e eşleyen 4 seviyeli bir paging hiyerarşisini zaten kurmuştu. Bootloader bunu yapar, çünkü paging x86_64'te 64-bit modda zorunludur.

["A minimal Rust kernel"]: @/edition-2/posts/02-minimal-rust-kernel/index.tr.md#creating-a-bootimage

Bu, kernel'imizde kullandığımız her bellek adresinin bir sanal adres olduğu anlamına gelir. VGA arabelleğine `0xb8000` adresinde erişmek yalnızca, bootloader o bellek sayfasını _kimlik eşlediği (identity mapped)_ için çalıştı; bu da `0xb8000` sanal sayfasını `0xb8000` fiziksel frame'ine eşlediği anlamına gelir.

Paging, kernel'imizi şimdiden nispeten güvenli kılar; çünkü sınırların dışındaki her bellek erişimi, rastgele fiziksel belleğe yazmak yerine bir page fault exception'ına neden olur. Bootloader, her sayfa için doğru erişim izinlerini bile ayarlar; bu da yalnızca kod içeren sayfaların çalıştırılabilir ve yalnızca veri sayfalarının yazılabilir olduğu anlamına gelir.

### Page Fault'lar

Kernel'imizin dışındaki bir belleğe erişerek bir page fault'a neden olmaya çalışalım. İlk olarak, bir page fault handler'ı oluşturup onu IDT'mizde kaydediyoruz; böylece genel bir [double fault] yerine bir page fault exception'ı görürüz:

[double fault]: @/edition-2/posts/06-double-faults/index.tr.md

```rust
// src/interrupts.rs içinde

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        […]

        idt.page_fault.set_handler_fn(page_fault_handler); // yeni

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

[`CR2`] register'ı, bir page fault'ta CPU tarafından otomatik olarak ayarlanır ve page fault'a neden olan erişilen sanal adresi içerir. Onu okuyup yazdırmak için `x86_64` crate'inin [`Cr2::read`] fonksiyonunu kullanıyoruz. [`PageFaultErrorCode`] tipi, page fault'a neden olan bellek erişiminin türü hakkında, örneğin bunun bir okuma mı yoksa yazma işlemi tarafından mı oluştuğu gibi, daha fazla bilgi sağlar. Bu nedenle onu da yazdırıyoruz. Page fault'u çözmeden çalıştırmaya devam edemeyiz, bu yüzden sonunda bir [`hlt_loop`]'a giriyoruz.

[`CR2`]: https://en.wikipedia.org/wiki/Control_register#CR2
[`Cr2::read`]: https://docs.rs/x86_64/0.14.2/x86_64/registers/control/struct.Cr2.html#method.read
[`PageFaultErrorCode`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.PageFaultErrorCode.html
[LLVM bug]: https://github.com/rust-lang/rust/issues/57270
[`hlt_loop`]: @/edition-2/posts/07-hardware-interrupts/index.tr.md#the-hlt-instruction

Artık kernel'imizin dışındaki bir belleğe erişmeyi deneyebiliriz:

```rust
// src/main.rs içinde

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    // yeni
    let ptr = 0xdeadbeaf as *mut u8;
    unsafe { *ptr = 42; }

    // önceki gibi
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

Onu çalıştırdığımızda, page fault handler'ımızın çağrıldığını görüyoruz:

![EXCEPTION: Page Fault, Accessed Address: VirtAddr(0xdeadbeaf), Error Code: CAUSED_BY_WRITE, InterruptStackFrame: {…}](qemu-page-fault.png)

`CR2` register'ı gerçekten de erişmeye çalıştığımız adres olan `0xdeadbeaf`'i içeriyor. Hata kodu, [`CAUSED_BY_WRITE`] aracılığıyla bize fault'un bir yazma işlemi gerçekleştirilmeye çalışılırken meydana geldiğini söylüyor. [Ayarlı _olmayan_ bitler][`PageFaultErrorCode`] aracılığıyla bize daha fazlasını da söylüyor. Örneğin, `PROTECTION_VIOLATION` bayrağının ayarlı olmaması, page fault'un hedef sayfa mevcut olmadığı için meydana geldiği anlamına gelir.

[`CAUSED_BY_WRITE`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.PageFaultErrorCode.html#associatedconstant.CAUSED_BY_WRITE

Mevcut komut işaretçisinin `0x2031b2` olduğunu görüyoruz, bu yüzden bu adresin bir kod sayfasına işaret ettiğini biliyoruz. Kod sayfaları bootloader tarafından salt okunur eşlenir, bu yüzden bu adresten okumak çalışır, ancak yazmak bir page fault'a neden olur. Bunu, `0xdeadbeaf` işaretçisini `0x2031b2` olarak değiştirerek deneyebilirsiniz:

```rust
// Not: Gerçek adres sizin için farklı olabilir. Page fault handler'ınızın
// bildirdiği adresi kullanın.
let ptr = 0x2031b2 as *mut u8;

// bir kod sayfasından oku
unsafe { let x = *ptr; }
println!("read worked");

// bir kod sayfasına yaz
unsafe { *ptr = 42; }
println!("write worked");
```

Son satırı yorum satırı haline getirerek, okuma erişiminin çalıştığını, ancak yazma erişiminin bir page fault'a neden olduğunu görürüz:

![QEMU çıktısı: "read worked, EXCEPTION: Page Fault, Accessed Address: VirtAddr(0x2031b2), Error Code: PROTECTION_VIOLATION | CAUSED_BY_WRITE, InterruptStackFrame: {…}"](qemu-page-fault-protection.png)

_"read worked"_ mesajının yazdırıldığını görüyoruz; bu da okuma işleminin herhangi bir hataya neden olmadığını gösteriyor. Ancak _"write worked"_ mesajı yerine bir page fault meydana geliyor. Bu sefer [`CAUSED_BY_WRITE`] bayrağına ek olarak [`PROTECTION_VIOLATION`] bayrağı da ayarlanmış; bu da sayfanın mevcut olduğunu, ancak işleme onda izin verilmediğini gösteriyor. Bu durumda, kod sayfaları salt okunur eşlendiği için sayfaya yazmaya izin verilmiyor.

[`PROTECTION_VIOLATION`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.PageFaultErrorCode.html#associatedconstant.PROTECTION_VIOLATION

### Sayfa Tablolarına Erişmek {#accessing-the-page-tables}

Kernel'imizin nasıl eşlendiğini tanımlayan sayfa tablolarına bir göz atmaya çalışalım:

```rust
// src/main.rs içinde

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    use x86_64::registers::control::Cr3;

    let (level_4_page_table, _) = Cr3::read();
    println!("Level 4 page table at: {:?}", level_4_page_table.start_address());

    […] // test_main(), println(…) ve hlt_loop()
}
```

`x86_64`'ün [`Cr3::read`] fonksiyonu, şu anda aktif olan seviye 4 sayfa tablosunu `CR3` register'ından döndürür. Bir [`PhysFrame`] ve bir [`Cr3Flags`] tipinden oluşan bir tuple döndürür. Yalnızca frame ile ilgilendiğimiz için, tuple'ın ikinci elemanını yok sayıyoruz.

[`Cr3::read`]: https://docs.rs/x86_64/0.14.2/x86_64/registers/control/struct.Cr3.html#method.read
[`PhysFrame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/frame/struct.PhysFrame.html
[`Cr3Flags`]: https://docs.rs/x86_64/0.14.2/x86_64/registers/control/struct.Cr3Flags.html

Onu çalıştırdığımızda, aşağıdaki çıktıyı görüyoruz:

```
Level 4 page table at: PhysAddr(0x1000)
```

Yani şu anda aktif olan seviye 4 sayfa tablosu, [`PhysAddr`] sarmalayıcı tipinin belirttiği gibi, _fiziksel_ bellekte `0x1000` adresinde saklanıyor. Şimdi soru şu: bu tabloya kernel'imizden nasıl erişebiliriz?

[`PhysAddr`]: https://docs.rs/x86_64/0.14.2/x86_64/addr/struct.PhysAddr.html

Paging aktifken fiziksel belleğe doğrudan erişmek mümkün değildir, çünkü aksi takdirde programlar bellek korumasını kolayca atlayıp diğer programların belleğine erişebilirdi. Yani tabloya erişmenin tek yolu, `0x1000` adresindeki fiziksel frame'e eşlenmiş bir sanal sayfa aracılığıyladır. Sayfa tablosu frame'leri için eşlemeler oluşturma sorunu genel bir sorundur, çünkü kernel'in sayfa tablolarına düzenli olarak erişmesi gerekir; örneğin yeni bir thread için bir stack ayırırken.

Bu soruna yönelik çözümler bir sonraki yazıda ayrıntılı olarak açıklanmaktadır.

## Özet

Bu yazı iki bellek koruma tekniğini tanıttı: segmentasyon ve paging. Birincisi değişken boyutlu bellek bölgeleri kullanır ve dış parçalanmadan muzdaripken, ikincisi sabit boyutlu sayfalar kullanır ve erişim izinleri üzerinde çok daha ince taneli kontrole olanak tanır.

Paging, sayfalar için eşleme bilgisini bir veya daha fazla seviyeye sahip sayfa tablolarında saklar. x86_64 mimarisi 4 seviyeli sayfa tabloları ve 4&nbsp;KiB'lık bir sayfa boyutu kullanır. Donanım otomatik olarak sayfa tablolarında yürür ve elde edilen çevirileri translation lookaside buffer'da (TLB) önbelleğe alır. Bu arabellek şeffaf bir şekilde güncellenmez ve sayfa tablosu değişikliklerinde elle temizlenmesi (flush) gerekir.

Kernel'imizin zaten paging üzerinde çalıştığını ve yasa dışı bellek erişimlerinin page fault exception'larına neden olduğunu öğrendik. Şu anda aktif olan sayfa tablolarına erişmeye çalıştık, ancak bunu yapamadık; çünkü CR3 register'ı kernel'imizden doğrudan erişemeyeceğimiz bir fiziksel adres saklar.

## Sırada ne var?

Bir sonraki yazı, kernel'imizde paging için desteğin nasıl uygulanacağını açıklar. Kernel'imizden fiziksel belleğe erişmenin farklı yollarını sunar; bu da kernel'imizin üzerinde çalıştığı sayfa tablolarına erişmeyi mümkün kılar. Bu noktada, sanal adresleri fiziksel adreslere çevirmek ve sayfa tablolarında yeni eşlemeler oluşturmak için fonksiyonlar uygulayabiliriz.
