+++
title = "Desabilitando SIMD"
weight = 2
path = "pt-BR/disable-simd"
template = "edition-2/extra.html"

[extra]
# Please update this when updating the translation
translation_based_on_commit = "9d079e6d3e03359469d6cf1759bb1a196d8a11ac"
# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

Instruções [Single Instruction Multiple Data (SIMD)] são capazes de realizar uma operação (por exemplo, adição) simultaneamente em múltiplas palavras de dados, o que pode acelerar programas significativamente. A arquitetura `x86_64` suporta vários padrões SIMD:

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

<!-- more -->

- [MMX]: O conjunto de instruções _Multi Media Extension_ foi introduzido em 1997 e define oito registradores de 64 bits chamados `mm0` até `mm7`. Esses registradores são apenas aliases para os registradores da [unidade de ponto flutuante x87].
- [SSE]: O conjunto de instruções _Streaming SIMD Extensions_ foi introduzido em 1999. Em vez de reutilizar os registradores de ponto flutuante, ele adiciona um conjunto de registradores completamente novo. Os dezesseis novos registradores são chamados `xmm0` até `xmm15` e têm 128 bits cada.
- [AVX]: As _Advanced Vector Extensions_ são extensões que aumentam ainda mais o tamanho dos registradores multimídia. Os novos registradores são chamados `ymm0` até `ymm15` e têm 256 bits cada. Eles estendem os registradores `xmm`, então por exemplo `xmm0` é a metade inferior de `ymm0`.

[MMX]: https://en.wikipedia.org/wiki/MMX_(instruction_set)
[unidade de ponto flutuante x87]: https://en.wikipedia.org/wiki/X87
[SSE]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions
[AVX]: https://en.wikipedia.org/wiki/Advanced_Vector_Extensions

Ao usar tais padrões SIMD, programas frequentemente podem acelerar significativamente. Bons compiladores são capazes de transformar loops normais em tal código SIMD automaticamente através de um processo chamado [auto-vetorização].

[auto-vetorização]: https://en.wikipedia.org/wiki/Automatic_vectorization

No entanto, os grandes registradores SIMD levam a problemas em kernels de SO. A razão é que o kernel tem que fazer backup de todos os registradores que usa para a memória em cada interrupção de hardware, porque eles precisam ter seus valores originais quando o programa interrompido continua. Então, se o kernel usa registradores SIMD, ele tem que fazer backup de muito mais dados (512-1600 bytes), o que diminui notavelmente o desempenho. Para evitar esta perda de desempenho, queremos desabilitar os recursos `sse` e `mmx` (o recurso `avx` é desabilitado por padrão).

Podemos fazer isso através do campo `features` na nossa especificação de alvo. Para desabilitar os recursos `mmx` e `sse`, nós os adicionamos prefixados com um menos:

```json
"features": "-mmx,-sse"
```

## Ponto Flutuante
Infelizmente para nós, a arquitetura `x86_64` usa registradores SSE para operações de ponto flutuante. Assim, todo uso de ponto flutuante com SSE desabilitado causa um erro no LLVM. O problema é que a biblioteca core do Rust já usa floats (por exemplo, ela implementa traits para `f32` e `f64`), então evitar floats no nosso kernel não é suficiente.

Felizmente, o LLVM tem suporte para um recurso `soft-float` que emula todas as operações de ponto flutuante através de funções de software baseadas em inteiros normais. Isso torna possível usar floats no nosso kernel sem SSE; será apenas um pouco mais lento.

Para ativar o recurso `soft-float` para o nosso kernel, nós o adicionamos à linha `features` na nossa especificação de alvo, prefixado com um mais:

```json
"features": "-mmx,-sse,+soft-float"
```