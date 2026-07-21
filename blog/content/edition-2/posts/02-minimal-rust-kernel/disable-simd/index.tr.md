+++
title = "SIMD'yi Devre Dışı Bırakmak"
weight = 2
path = "tr/disable-simd"
template = "edition-2/extra.html"

[extra]
# Please update this when updating the translation
translation_based_on_commit = "1132d7a3835dc6c0b3fd8f6b45c9295a9bc1f837"
# GitHub usernames of the people that translated this post
translators = ["rhotav"]
+++

[Single Instruction Multiple Data (SIMD)] komutları, bir işlemi (örneğin toplama) aynı anda birden çok veri sözcüğü üzerinde gerçekleştirebilir; bu da programları önemli ölçüde hızlandırabilir. `x86_64` mimarisi çeşitli SIMD standartlarını destekler:

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

<!-- more -->

- [MMX]: _Multi Media Extension_ komut kümesi 1997'de tanıtıldı ve `mm0`'dan `mm7`'ye kadar adlandırılan sekiz adet 64-bit register tanımlar. Bu register'lar yalnızca [x87 kayan nokta birimi (floating point unit)][x87 floating point unit] register'larının takma adlarıdır.
- [SSE]: _Streaming SIMD Extensions_ komut kümesi 1999'da tanıtıldı. Kayan nokta register'larını yeniden kullanmak yerine, tamamen yeni bir register kümesi ekler. `xmm0`'dan `xmm15`'e kadar adlandırılan on altı yeni register vardır ve her biri 128 bittir.
- [AVX]: _Advanced Vector Extensions_, multimedya register'larının boyutunu daha da artıran uzantılardır. Yeni register'lar `ymm0`'dan `ymm15`'e kadar adlandırılır ve her biri 256 bittir. `xmm` register'larını genişletirler, böylece örneğin `xmm0`, `ymm0`'ın alt yarısıdır.

[MMX]: https://en.wikipedia.org/wiki/MMX_(instruction_set)
[x87 floating point unit]: https://en.wikipedia.org/wiki/X87
[SSE]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions
[AVX]: https://en.wikipedia.org/wiki/Advanced_Vector_Extensions

Bu tür SIMD standartlarını kullanarak programlar genellikle önemli ölçüde hızlanabilir. İyi derleyiciler, [otomatik vektörleştirme (auto-vectorization)][auto-vectorization] adı verilen bir süreç aracılığıyla normal döngüleri otomatik olarak bu tür SIMD koduna dönüştürebilir.

[auto-vectorization]: https://en.wikipedia.org/wiki/Automatic_vectorization

Ancak büyük SIMD register'ları, OS kernel'lerinde sorunlara yol açar. Bunun nedeni, kernel'in her donanım interrupt'ında kullandığı tüm register'ları belleğe yedeklemesi gerektiğidir; çünkü kesintiye uğramış program devam ettiğinde bunların orijinal değerlerine sahip olması gerekir. Yani kernel SIMD register'larını kullanırsa, çok daha fazla veriyi (512–1600 bayt) yedeklemesi gerekir ve bu da performansı gözle görülür biçimde düşürür. Bu performans kaybını önlemek için `sse` ve `mmx` özelliklerini devre dışı bırakmak istiyoruz (`avx` özelliği zaten varsayılan olarak devre dışıdır).

Bunu, hedef belirtimimizdeki `features` alanı aracılığıyla yapabiliriz. `mmx` ve `sse` özelliklerini devre dışı bırakmak için, onları önlerine eksi koyarak ekliyoruz:

```json
"features": "-mmx,-sse"
```

## Kayan Nokta (Floating Point)
Bizim için ne yazık ki, `x86_64` mimarisi kayan nokta işlemleri için SSE register'larını kullanır. Dolayısıyla, SSE devre dışıyken kayan noktanın her kullanımı LLVM'de bir hataya neden olur. Sorun, Rust'ın core kütüphanesinin zaten float'ları kullanmasıdır (örneğin `f32` ve `f64` için trait'ler uygular), bu yüzden kernel'imizde float'lardan kaçınmak yeterli değildir.

Neyse ki LLVM, tüm kayan nokta işlemlerini normal tamsayılara dayalı yazılım fonksiyonları aracılığıyla öykünen bir `soft-float` özelliğine destek sağlar. Bu, kernel'imizde SSE olmadan float kullanmayı mümkün kılar; sadece biraz daha yavaş olur.

Kernel'imiz için `soft-float` özelliğini açmak amacıyla, onu hedef belirtimimizdeki `features` satırına önüne artı koyarak ekliyoruz:

```json
"features": "-mmx,-sse,+soft-float"
```
