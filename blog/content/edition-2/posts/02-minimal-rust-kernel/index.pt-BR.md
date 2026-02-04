+++
title = "Um Kernel Rust Mínimo"
weight = 2
path = "pt-BR/minimal-rust-kernel"
date = 2018-02-10

[extra]
chapter = "O Básico"
# Please update this when updating the translation
translation_based_on_commit = "95d4fbd54c6b0e5a874981558c0cc1fe85d31606"
# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

Neste post, criamos um kernel Rust mínimo de 64 bits para a arquitetura x86. Construímos sobre o [binário Rust independente] do post anterior para criar uma imagem de disco inicializável que imprime algo na tela.

[binário Rust independente]: @/edition-2/posts/01-freestanding-rust-binary/index.pt-BR.md

<!-- more -->

Este blog é desenvolvido abertamente no [GitHub]. Se você tiver algum problema ou dúvida, abra um issue lá. Você também pode deixar comentários [na parte inferior]. O código-fonte completo desta publicação pode ser encontrado na branch [`post-02`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[na parte inferior]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-02

<!-- toc -->

## O Processo de Boot
Quando você liga um computador, ele começa a executar código de firmware que está armazenado na [ROM] da placa-mãe. Este código executa um [teste automático de inicialização], detecta a RAM disponível e pré-inicializa a CPU e o hardware. Depois, ele procura por um disco inicializável e começa a inicializar o kernel do sistema operacional.

[ROM]: https://en.wikipedia.org/wiki/Read-only_memory
[teste automático de inicialização]: https://en.wikipedia.org/wiki/Power-on_self-test

No x86, existem dois padrões de firmware: o "Basic Input/Output System" (**[BIOS]**) e o mais novo "Unified Extensible Firmware Interface" (**[UEFI]**). O padrão BIOS é antigo e ultrapassado, mas simples e bem suportado em qualquer máquina x86 desde os anos 1980. UEFI, em contraste, é mais moderno e tem muito mais recursos, mas é mais complexo de configurar (na minha opinião, pelo menos).

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[UEFI]: https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

Atualmente, fornecemos apenas suporte para BIOS, mas suporte para UEFI também está planejado. Se você gostaria de nos ajudar com isso, confira o [issue no Github](https://github.com/phil-opp/blog_os/issues/349).

### Boot BIOS
Quase todos os sistemas x86 têm suporte para boot BIOS, incluindo máquinas mais novas baseadas em UEFI que usam um BIOS emulado. Isso é ótimo, porque você pode usar a mesma lógica de boot em todas as máquinas do último século. Mas essa ampla compatibilidade é ao mesmo tempo a maior desvantagem do boot BIOS, porque significa que a CPU é colocada em um modo de compatibilidade de 16 bits chamado [modo real] antes do boot, para que bootloaders arcaicos dos anos 1980 ainda funcionem.

Mas vamos começar do início:

Quando você liga um computador, ele carrega o BIOS de uma memória flash especial localizada na placa-mãe. O BIOS executa rotinas de teste automático e inicialização do hardware, então procura por discos inicializáveis. Se ele encontra um, o controle é transferido para seu _bootloader_, que é uma porção de 512 bytes de código executável armazenado no início do disco. A maioria dos bootloaders é maior que 512 bytes, então os bootloaders são comumente divididos em um primeiro estágio pequeno, que cabe em 512 bytes, e um segundo estágio, que é subsequentemente carregado pelo primeiro estágio.

O bootloader tem que determinar a localização da imagem do kernel no disco e carregá-la na memória. Ele também precisa mudar a CPU do [modo real] de 16 bits primeiro para o [modo protegido] de 32 bits, e então para o [modo longo] de 64 bits, onde registradores de 64 bits e a memória principal completa estão disponíveis. Seu terceiro trabalho é consultar certas informações (como um mapa de memória) do BIOS e passá-las ao kernel do SO.

[modo real]: https://en.wikipedia.org/wiki/Real_mode
[modo protegido]: https://en.wikipedia.org/wiki/Protected_mode
[modo longo]: https://en.wikipedia.org/wiki/Long_mode
[segmentação de memória]: https://en.wikipedia.org/wiki/X86_memory_segmentation

Escrever um bootloader é um pouco trabalhoso, pois requer linguagem assembly e muitos passos pouco intuitivos como "escrever este valor mágico neste registrador do processador". Portanto, não cobrimos a criação de bootloader neste post e em vez disso fornecemos uma ferramenta chamada [bootimage] que anexa automaticamente um bootloader ao seu kernel.

[bootimage]: https://github.com/rust-osdev/bootimage

Se você estiver interessado em construir seu próprio bootloader: Fique ligado, um conjunto de posts sobre este tópico já está planejado! <!-- , confira nossos posts "_[Writing a Bootloader]_", onde explicamos em detalhes como um bootloader é construído. -->

#### O Padrão Multiboot
Para evitar que todo sistema operacional implemente seu próprio bootloader, que é compatível apenas com um único SO, a [Free Software Foundation] criou um padrão de bootloader aberto chamado [Multiboot] em 1995. O padrão define uma interface entre o bootloader e o sistema operacional, para que qualquer bootloader compatível com Multiboot possa carregar qualquer sistema operacional compatível com Multiboot. A implementação de referência é o [GNU GRUB], que é o bootloader mais popular para sistemas Linux.

[Free Software Foundation]: https://en.wikipedia.org/wiki/Free_Software_Foundation
[Multiboot]: https://wiki.osdev.org/Multiboot
[GNU GRUB]: https://en.wikipedia.org/wiki/GNU_GRUB

Para tornar um kernel compatível com Multiboot, basta inserir um chamado [cabeçalho Multiboot] no início do arquivo do kernel. Isso torna muito fácil inicializar um SO a partir do GRUB. No entanto, o GRUB e o padrão Multiboot também têm alguns problemas:

[cabeçalho Multiboot]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format

- Eles suportam apenas o modo protegido de 32 bits. Isso significa que você ainda tem que fazer a configuração da CPU para mudar para o modo longo de 64 bits.
- Eles são projetados para tornar o bootloader simples em vez do kernel. Por exemplo, o kernel precisa ser vinculado com um [tamanho de página padrão ajustado], porque o GRUB não consegue encontrar o cabeçalho Multiboot caso contrário. Outro exemplo é que as [informações de boot], que são passadas ao kernel, contêm muitas estruturas dependentes de arquitetura em vez de fornecer abstrações limpas.
- Tanto o GRUB quanto o padrão Multiboot são documentados apenas esparsamente.
- O GRUB precisa estar instalado no sistema host para criar uma imagem de disco inicializável a partir do arquivo do kernel. Isso torna o desenvolvimento no Windows ou Mac mais difícil.

[tamanho de página padrão ajustado]: https://wiki.osdev.org/Multiboot#Multiboot_2
[informações de boot]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format

Por causa dessas desvantagens, decidimos não usar o GRUB ou o padrão Multiboot. No entanto, planejamos adicionar suporte Multiboot à nossa ferramenta [bootimage], para que seja possível carregar seu kernel em um sistema GRUB também. Se você estiver interessado em escrever um kernel compatível com Multiboot, confira a [primeira edição] desta série de blog.

[primeira edição]: @/edition-1/_index.md

### UEFI

(Não fornecemos suporte UEFI no momento, mas adoraríamos! Se você gostaria de ajudar, por favor nos diga no [issue do Github](https://github.com/phil-opp/blog_os/issues/349).)

## Um Kernel Mínimo
Agora que sabemos aproximadamente como um computador inicializa, é hora de criar nosso próprio kernel mínimo. Nosso objetivo é criar uma imagem de disco que imprima um "Hello World!" na tela quando inicializada. Fazemos isso estendendo o [binário Rust independente] do post anterior.

Como você deve se lembrar, construímos o binário independente através do `cargo`, mas dependendo do sistema operacional, precisávamos de nomes de ponto de entrada e flags de compilação diferentes. Isso ocorre porque o `cargo` compila para o _sistema host_ por padrão, ou seja, o sistema em que você está executando. Isso não é algo que queremos para nosso kernel, porque um kernel que executa em cima de, por exemplo, Windows, não faz muito sentido. Em vez disso, queremos compilar para um _sistema alvo_ claramente definido.

### Instalando o Rust Nightly
O Rust tem três canais de lançamento: _stable_, _beta_ e _nightly_. O Livro do Rust explica a diferença entre esses canais muito bem, então dê uma olhada [aqui](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html#choo-choo-release-channels-and-riding-the-trains). Para construir um sistema operacional, precisaremos de alguns recursos experimentais que estão disponíveis apenas no canal nightly, então precisamos instalar uma versão nightly do Rust.

Para gerenciar instalações do Rust, eu recomendo fortemente o [rustup]. Ele permite instalar compiladores nightly, beta e stable lado a lado e facilita a atualização deles. Com rustup, você pode usar um compilador nightly para o diretório atual executando `rustup override set nightly`. Alternativamente, você pode adicionar um arquivo chamado `rust-toolchain` com o conteúdo `nightly` ao diretório raiz do projeto. Você pode verificar que tem uma versão nightly instalada executando `rustc --version`: O número da versão deve conter `-nightly` no final.

[rustup]: https://www.rustup.rs/

O compilador nightly nos permite optar por vários recursos experimentais usando as chamadas _feature flags_ no topo do nosso arquivo. Por exemplo, poderíamos habilitar a [macro `asm!`] experimental para assembly inline adicionando `#![feature(asm)]` no topo do nosso `main.rs`. Note que tais recursos experimentais são completamente instáveis, o que significa que versões futuras do Rust podem alterá-los ou removê-los sem aviso prévio. Por esta razão, só os usaremos se absolutamente necessário.

[macro `asm!`]: https://doc.rust-lang.org/stable/reference/inline-assembly.html

### Especificação de Alvo
O Cargo suporta diferentes sistemas alvo através do parâmetro `--target`. O alvo é descrito por uma chamada _[target triple]_, que descreve a arquitetura da CPU, o vendor, o sistema operacional e a [ABI]. Por exemplo, o target triple `x86_64-unknown-linux-gnu` descreve um sistema com uma CPU `x86_64`, sem vendor claro, e um sistema operacional Linux com a ABI GNU. O Rust suporta [muitos target triples diferentes][platform-support], incluindo `arm-linux-androideabi` para Android ou [`wasm32-unknown-unknown` para WebAssembly](https://www.hellorust.com/setup/wasm-target/).

[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[ABI]: https://stackoverflow.com/a/2456882
[platform-support]: https://forge.rust-lang.org/release/platform-support.html
[custom-targets]: https://doc.rust-lang.org/nightly/rustc/targets/custom.html

Para nosso sistema alvo, no entanto, precisamos de alguns parâmetros de configuração especiais (por exemplo, nenhum SO subjacente), então nenhum dos [target triples existentes][platform-support] se encaixa. Felizmente, o Rust nos permite definir [nosso próprio alvo][custom-targets] através de um arquivo JSON. Por exemplo, um arquivo JSON que descreve o target `x86_64-unknown-linux-gnu` se parece com isto:

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

A maioria dos campos é exigida pelo LLVM para gerar código para aquela plataforma. Por exemplo, o campo [`data-layout`] define o tamanho de vários tipos integer, floating point e pointer. Então há campos que o Rust usa para compilação condicional, como `target-pointer-width`. O terceiro tipo de campo define como a crate deve ser construída. Por exemplo, o campo `pre-link-args` especifica argumentos passados ao [linker].

[`data-layout`]: https://llvm.org/docs/LangRef.html#data-layout
[linker]: https://en.wikipedia.org/wiki/Linker_(computing)

Também visamos sistemas `x86_64` com nosso kernel, então nossa especificação de alvo será muito similar à acima. Vamos começar criando um arquivo `x86_64-blog_os.json` (escolha qualquer nome que você goste) com o conteúdo comum:

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

Note que mudamos o SO no `llvm-target` e no campo `os` para `none`, porque executaremos em bare metal.

Adicionamos as seguintes entradas relacionadas à compilação:

```json
"linker-flavor": "ld.lld",
"linker": "rust-lld",
```

Em vez de usar o linker padrão da plataforma (que pode não suportar alvos Linux), usamos o linker multiplataforma [LLD] que vem com o Rust para vincular nosso kernel.

[LLD]: https://lld.llvm.org/

```json
"panic-strategy": "abort",
```

Esta configuração especifica que o alvo não suporta [stack unwinding] no panic, então em vez disso o programa deve abortar diretamente. Isso tem o mesmo efeito que a opção `panic = "abort"` no nosso Cargo.toml, então podemos removê-la de lá. (Note que, em contraste com a opção Cargo.toml, esta opção de alvo também se aplica quando recompilamos a biblioteca `core` mais adiante neste post. Então, mesmo se você preferir manter a opção Cargo.toml, certifique-se de incluir esta opção.)

[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php

```json
"disable-redzone": true,
```

Estamos escrevendo um kernel, então precisaremos lidar com interrupções em algum momento. Para fazer isso com segurança, temos que desabilitar uma certa otimização do ponteiro de stack chamada _"red zone"_, porque ela causaria corrupção do stack caso contrário. Para mais informações, veja nosso post separado sobre [desabilitando a red zone].

[desabilitando a red zone]: @/edition-2/posts/02-minimal-rust-kernel/disable-red-zone/index.md

```json
"features": "-mmx,-sse,+soft-float",
```

O campo `features` habilita/desabilita recursos do alvo. Desabilitamos os recursos `mmx` e `sse` prefixando-os com um menos e habilitamos o recurso `soft-float` prefixando-o com um mais. Note que não deve haver espaços entre flags diferentes, caso contrário o LLVM falha ao interpretar a string de features.

Os recursos `mmx` e `sse` determinam suporte para instruções [Single Instruction Multiple Data (SIMD)], que frequentemente podem acelerar programas significativamente. No entanto, usar os grandes registradores SIMD em kernels de SO leva a problemas de desempenho. A razão é que o kernel precisa restaurar todos os registradores ao seu estado original antes de continuar um programa interrompido. Isso significa que o kernel tem que salvar o estado SIMD completo na memória principal em cada chamada de sistema ou interrupção de hardware. Como o estado SIMD é muito grande (512-1600 bytes) e interrupções podem ocorrer com muita frequência, essas operações adicionais de salvar/restaurar prejudicam consideravelmente o desempenho. Para evitar isso, desabilitamos SIMD para nosso kernel (não para aplicações executando em cima!).

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

Um problema com desabilitar SIMD é que operações de ponto flutuante em `x86_64` exigem registradores SIMD por padrão. Para resolver este problema, adicionamos o recurso `soft-float`, que emula todas as operações de ponto flutuante através de funções de software baseadas em inteiros normais.

Para mais informações, veja nosso post sobre [desabilitando SIMD](@/edition-2/posts/02-minimal-rust-kernel/disable-simd/index.md).

```json
"rustc-abi": "x86-softfloat"
```

Como queremos usar o recurso `soft-float`, também precisamos dizer ao compilador Rust `rustc` que queremos usar a ABI correspondente. Podemos fazer isso definindo o campo `rustc-abi` para `x86-softfloat`.

#### Juntando Tudo
Nosso arquivo de especificação de alvo agora se parece com isto:

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
    "rustc-abi": "x86-softfloat"
}
```

### Construindo nosso Kernel
Compilar para nosso novo alvo usará convenções Linux, já que o linker-flavor ld.lld instrui o llvm a compilar com a flag `-flavor gnu` (para mais opções de linker, veja [a documentação do rustc](https://doc.rust-lang.org/rustc/codegen-options/index.html#linker-flavor)). Isso significa que precisamos de um ponto de entrada chamado `_start` como descrito no [post anterior]:

[post anterior]: @/edition-2/posts/01-freestanding-rust-binary/index.pt-BR.md

```rust
// src/main.rs

#![no_std] // não vincule a biblioteca padrão do Rust
#![no_main] // desativar todos os pontos de entrada no nível Rust

use core::panic::PanicInfo;

/// Esta função é chamada em caso de pânico.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)] // não altere (mangle) o nome desta função
pub extern "C" fn _start() -> ! {
    // essa função é o ponto de entrada, já que o vinculador procura uma função
    // denominado `_start` por padrão
    loop {}
}
```

Note que o ponto de entrada precisa ser chamado `_start` independentemente do seu SO host.

Agora podemos construir o kernel para nosso novo alvo passando o nome do arquivo JSON como `--target`:

```
> cargo build --target x86_64-blog_os.json

error: `.json` target specs require -Zjson-target-spec
```

Falha! O erro nos diz que especificações de alvo JSON personalizadas são um recurso instável que requer habilitação explícita. Isso ocorre porque o formato dos arquivos JSON de alvo ainda não é considerado estável, então mudanças podem ocorrer em futuras versões do Rust. Consulte a [issue de rastreamento para especificações de alvo JSON personalizadas][json-target-spec-issue] para mais informações.

[json-target-spec-issue]: https://github.com/rust-lang/rust/issues/151528

#### A Opção `json-target-spec`

Para habilitar o suporte para especificações de alvo JSON personalizadas, precisamos criar um arquivo de [configuração cargo] local em `.cargo/config.toml` (a pasta `.cargo` deve estar ao lado da sua pasta `src`) com o seguinte conteúdo:

[configuração cargo]: https://doc.rust-lang.org/cargo/reference/config.html

```toml
# em .cargo/config.toml

[unstable]
json-target-spec = true
```

Isso habilita o recurso instável `json-target-spec`, permitindo-nos usar arquivos JSON de alvo personalizados.

Com esta configuração em vigor, vamos tentar construir novamente:

```
> cargo build --target x86_64-blog_os.json

error[E0463]: can't find crate for `core`
```

Agora vemos um erro diferente! O erro nos diz que o compilador Rust não consegue mais encontrar a [biblioteca `core`]. Esta biblioteca contém tipos básicos do Rust como `Result`, `Option` e iteradores, e é implicitamente vinculada a todas as crates `no_std`.

[biblioteca `core`]: https://doc.rust-lang.org/nightly/core/index.html

O problema é que a biblioteca core é distribuída junto com o compilador Rust como uma biblioteca _pré-compilada_. Então ela é válida apenas para target triples host suportados (por exemplo, `x86_64-unknown-linux-gnu`) mas não para nosso alvo customizado. Se quisermos compilar código para outros alvos, precisamos recompilar `core` para esses alvos primeiro.

#### A Opção `build-std`

É aí que entra o [recurso `build-std`] do cargo. Ele permite recompilar `core` e outras crates da biblioteca padrão sob demanda, em vez de usar as versões pré-compiladas enviadas com a instalação do Rust. Este recurso é muito novo e ainda não está finalizado, então é marcado como "unstable" e disponível apenas em [compiladores Rust nightly].

[recurso `build-std`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std
[compiladores Rust nightly]: #instalando-o-rust-nightly

Para usar o recurso, precisamos adicionar o seguinte ao nosso arquivo de [configuração cargo] em `.cargo/config.toml`:

```toml
# em .cargo/config.toml

[unstable]
json-target-spec = true
build-std = ["core", "compiler_builtins"]
```

Isso diz ao cargo que ele deve recompilar as bibliotecas `core` e `compiler_builtins`. Esta última é necessária porque é uma dependência de `core`. Para recompilar essas bibliotecas, o cargo precisa de acesso ao código-fonte do rust, que podemos instalar com `rustup component add rust-src`.

<div class="note">

**Nota:** A chave de configuração `unstable.build-std` requer pelo menos o Rust nightly de 15-07-2020.

</div>

Depois de definir a chave de configuração `unstable.build-std` e instalar o componente `rust-src`, podemos executar novamente nosso comando de compilação:

```
> cargo build --target x86_64-blog_os.json
   Compiling core v0.0.0 (/…/rust/src/libcore)
   Compiling rustc-std-workspace-core v1.99.0 (/…/rust/src/tools/rustc-std-workspace-core)
   Compiling compiler_builtins v0.1.32
   Compiling blog_os v0.1.0 (/…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

Vemos que `cargo build` agora recompila as bibliotecas `core`, `rustc-std-workspace-core` (uma dependência de `compiler_builtins`) e `compiler_builtins` para nosso alvo customizado.

#### Intrínsecos Relacionados a Memória

O compilador Rust assume que um certo conjunto de funções embutidas está disponível para todos os sistemas. A maioria dessas funções é fornecida pela crate `compiler_builtins` que acabamos de recompilar. No entanto, existem algumas funções relacionadas a memória nessa crate que não são habilitadas por padrão porque normalmente são fornecidas pela biblioteca C no sistema. Essas funções incluem `memset`, que define todos os bytes em um bloco de memória para um valor dado, `memcpy`, que copia um bloco de memória para outro, e `memcmp`, que compara dois blocos de memória. Embora não precisássemos de nenhuma dessas funções para compilar nosso kernel agora, elas serão necessárias assim que adicionarmos mais código a ele (por exemplo, ao copiar structs).

Como não podemos vincular à biblioteca C do sistema operacional, precisamos de uma maneira alternativa de fornecer essas funções ao compilador. Uma possível abordagem para isso poderia ser implementar nossas próprias funções `memset` etc. e aplicar o atributo `#[unsafe(no_mangle)]` a elas (para evitar a renomeação automática durante a compilação). No entanto, isso é perigoso, pois o menor erro na implementação dessas funções pode levar a undefined behavior. Por exemplo, implementar `memcpy` com um loop `for` pode resultar em recursão infinita porque loops `for` implicitamente chamam o método da trait [`IntoIterator::into_iter`], que pode chamar `memcpy` novamente. Então é uma boa ideia reutilizar implementações existentes e bem testadas em vez disso.

[`IntoIterator::into_iter`]: https://doc.rust-lang.org/stable/core/iter/trait.IntoIterator.html#tymethod.into_iter

Felizmente, a crate `compiler_builtins` já contém implementações para todas as funções necessárias, elas estão apenas desabilitadas por padrão para não colidir com as implementações da biblioteca C. Podemos habilitá-las definindo a flag [`build-std-features`] do cargo para `["compiler-builtins-mem"]`. Como a flag `build-std`, esta flag pode ser passada na linha de comando como uma flag `-Z` ou configurada na tabela `unstable` no arquivo `.cargo/config.toml`. Como queremos sempre compilar com esta flag, a opção do arquivo de configuração faz mais sentido para nós:

[`build-std-features`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features

```toml
# em .cargo/config.toml

[unstable]
json-target-spec = true
build-std-features = ["compiler-builtins-mem"]
build-std = ["core", "compiler_builtins"]
```

(O suporte para o recurso `compiler-builtins-mem` foi [adicionado muito recentemente](https://github.com/rust-lang/rust/pull/77284), então você precisa pelo menos do Rust nightly `2020-09-30` para ele.)

Nos bastidores, esta flag habilita o [recurso `mem`] da crate `compiler_builtins`. O efeito disso é que o atributo `#[unsafe(no_mangle)]` é aplicado às [implementações `memcpy` etc.] da crate, o que as torna disponíveis ao linker.

[recurso `mem`]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/Cargo.toml#L54-L55
[implementações `memcpy` etc.]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/src/mem.rs#L12-L69

Com esta mudança, nosso kernel tem implementações válidas para todas as funções exigidas pelo compilador, então ele continuará a compilar mesmo se nosso código ficar mais complexo.

#### Definir um Alvo Padrão

Para evitar passar o parâmetro `--target` em cada invocação de `cargo build`, podemos sobrescrever o alvo padrão. Para fazer isso, adicionamos o seguinte ao nosso arquivo de [configuração cargo] em `.cargo/config.toml`:

[configuração cargo]: https://doc.rust-lang.org/cargo/reference/config.html

```toml
# em .cargo/config.toml

[build]
target = "x86_64-blog_os.json"
```

Isso diz ao `cargo` para usar nosso alvo `x86_64-blog_os.json` quando nenhum argumento `--target` explícito é passado. Isso significa que agora podemos construir nosso kernel com um simples `cargo build`. Para mais informações sobre opções de configuração do cargo, confira a [documentação oficial][configuração cargo].

Agora podemos construir nosso kernel para um alvo bare metal com um simples `cargo build`. No entanto, nosso ponto de entrada `_start`, que será chamado pelo bootloader, ainda está vazio. É hora de mostrar algo na tela a partir dele.

### Imprimindo na Tela
A maneira mais fácil de imprimir texto na tela neste estágio é o [buffer de texto VGA]. É uma área de memória especial mapeada para o hardware VGA que contém o conteúdo exibido na tela. Normalmente consiste em 25 linhas que cada uma contém 80 células de caractere. Cada célula de caractere exibe um caractere ASCII com algumas cores de primeiro plano e fundo. A saída da tela se parece com isto:

[buffer de texto VGA]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode

![saída de tela para caracteres ASCII comuns](https://upload.wikimedia.org/wikipedia/commons/f/f8/Codepage-437.png)

Discutiremos o layout exato do buffer VGA no próximo post, onde escreveremos um primeiro pequeno driver para ele. Para imprimir "Hello World!", só precisamos saber que o buffer está localizado no endereço `0xb8000` e que cada célula de caractere consiste em um byte ASCII e um byte de cor.

A implementação se parece com isto:

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

Primeiro, convertemos o inteiro `0xb8000` em um [ponteiro bruto]. Então [iteramos] sobre os bytes da [byte string] [static] `HELLO`. Usamos o método [`enumerate`] para obter adicionalmente uma variável em execução `i`. No corpo do loop for, usamos o método [`offset`] para escrever o byte da string e o byte de cor correspondente (`0xb` é um ciano claro).

[iterar]: https://doc.rust-lang.org/stable/book/ch13-02-iterators.html
[static]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate
[byte string]: https://doc.rust-lang.org/reference/tokens.html#byte-string-literals
[ponteiro bruto]: https://doc.rust-lang.org/stable/book/ch20-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

Note que há um bloco [`unsafe`] em torno de todas as escritas de memória. A razão é que o compilador Rust não pode provar que os ponteiros brutos que criamos são válidos. Eles poderiam apontar para qualquer lugar e levar à corrupção de dados. Ao colocá-los em um bloco `unsafe`, estamos basicamente dizendo ao compilador que temos absoluta certeza de que as operações são válidas. Note que um bloco `unsafe` não desativa as verificações de segurança do Rust. Ele apenas permite que você faça [cinco coisas adicionais].

[`unsafe`]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html
[cinco coisas adicionais]: https://doc.rust-lang.org/stable/book/ch20-01-unsafe-rust.html#unsafe-superpowers

Quero enfatizar que **esta não é a maneira como queremos fazer as coisas em Rust!** É muito fácil bagunçar ao trabalhar com ponteiros brutos dentro de blocos unsafe. Por exemplo, poderíamos facilmente escrever além do fim do buffer se não tivermos cuidado.

Então queremos minimizar o uso de `unsafe` o máximo possível. O Rust nos dá a capacidade de fazer isso criando abstrações seguras. Por exemplo, poderíamos criar um tipo de buffer VGA que encapsula toda a unsafety e garante que seja _impossível_ fazer algo errado de fora. Desta forma, precisaríamos apenas de quantidades mínimas de código `unsafe` e poderíamos ter certeza de que não violamos [memory safety]. Criaremos tal abstração de buffer VGA segura no próximo post.

[memory safety]: https://en.wikipedia.org/wiki/Memory_safety

## Executando nosso Kernel

Agora que temos um executável que faz algo perceptível, é hora de executá-lo. Primeiro, precisamos transformar nosso kernel compilado em uma imagem de disco inicializável vinculando-o com um bootloader. Então podemos executar a imagem de disco na máquina virtual [QEMU] ou inicializá-la em hardware real usando um pendrive USB.

### Criando uma Bootimage

Para transformar nosso kernel compilado em uma imagem de disco inicializável, precisamos vinculá-lo com um bootloader. Como aprendemos na [seção sobre boot], o bootloader é responsável por inicializar a CPU e carregar nosso kernel.

[seção sobre boot]: #o-processo-de-boot

Em vez de escrever nosso próprio bootloader, que é um projeto por si só, usamos a crate [`bootloader`]. Esta crate implementa um bootloader BIOS básico sem nenhuma dependência C, apenas Rust e assembly inline. Para usá-lo para inicializar nosso kernel, precisamos adicionar uma dependência nele:

[`bootloader`]: https://crates.io/crates/bootloader

```toml
# em Cargo.toml

[dependencies]
bootloader = "0.9"
```

**Nota:** Este post é compatível apenas com `bootloader v0.9`. Versões mais novas usam um sistema de compilação diferente e resultarão em erros de compilação ao seguir este post.

Adicionar o bootloader como uma dependência não é suficiente para realmente criar uma imagem de disco inicializável. O problema é que precisamos vincular nosso kernel com o bootloader após a compilação, mas o cargo não tem suporte para [scripts pós-compilação].

[scripts pós-compilação]: https://github.com/rust-lang/cargo/issues/545

Para resolver este problema, criamos uma ferramenta chamada `bootimage` que primeiro compila o kernel e o bootloader, e então os vincula juntos para criar uma imagem de disco inicializável. Para instalar a ferramenta, vá para seu diretório home (ou qualquer diretório fora do seu projeto cargo) e execute o seguinte comando no seu terminal:

```
cargo install bootimage
```

Para executar `bootimage` e construir o bootloader, você precisa ter o componente rustup `llvm-tools-preview` instalado. Você pode fazer isso executando `rustup component add llvm-tools-preview`.

Depois de instalar `bootimage` e adicionar o componente `llvm-tools-preview`, você pode criar uma imagem de disco inicializável voltando para o diretório do seu projeto cargo e executando:

```
> cargo bootimage
```

Vemos que a ferramenta recompila nosso kernel usando `cargo build`, então automaticamente pegará quaisquer mudanças que você fizer. Depois, ela compila o bootloader, o que pode demorar um pouco. Como todas as dependências de crate, ele é compilado apenas uma vez e então armazenado em cache, então compilações subsequentes serão muito mais rápidas. Finalmente, `bootimage` combina o bootloader e seu kernel em uma imagem de disco inicializável.

Após executar o comando, você deve ver uma imagem de disco inicializável chamada `bootimage-blog_os.bin` no seu diretório `target/x86_64-blog_os/debug`. Você pode inicializá-la em uma máquina virtual ou copiá-la para um pendrive USB para inicializá-la em hardware real. (Note que este não é uma imagem de CD, que tem um formato diferente, então gravá-la em um CD não funciona).

#### Como funciona?
A ferramenta `bootimage` executa os seguintes passos nos bastidores:

- Ela compila nosso kernel para um arquivo [ELF].
- Ela compila a dependência do bootloader como um executável autônomo.
- Ela vincula os bytes do arquivo ELF do kernel ao bootloader.

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
[rust-osdev/bootloader]: https://github.com/rust-osdev/bootloader

Quando inicializado, o bootloader lê e analisa o arquivo ELF anexado. Ele então mapeia os segmentos do programa para endereços virtuais nas tabelas de página, zera a seção `.bss` e configura um stack. Finalmente, ele lê o endereço do ponto de entrada (nossa função `_start`) e salta para ele.

### Inicializando no QEMU

Agora podemos inicializar a imagem de disco em uma máquina virtual. Para inicializá-la no [QEMU], execute o seguinte comando:

[QEMU]: https://www.qemu.org/

```
> qemu-system-x86_64 -drive format=raw,file=target/x86_64-blog_os/debug/bootimage-blog_os.bin
```

Isso abre uma janela separada que deve se parecer com isto:

![QEMU mostrando "Hello World!"](qemu.png)

Vemos que nosso "Hello World!" está visível na tela.

### Máquina Real

Também é possível escrevê-lo em um pendrive USB e inicializá-lo em uma máquina real, **mas tenha cuidado** para escolher o nome correto do dispositivo, porque **tudo naquele dispositivo será sobrescrito**:

```
> dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

Onde `sdX` é o nome do dispositivo do seu pendrive USB.

Depois de escrever a imagem no pendrive USB, você pode executá-la em hardware real inicializando a partir dele. Você provavelmente precisará usar um menu de boot especial ou alterar a ordem de boot na configuração do BIOS para inicializar a partir do pendrive USB. Note que atualmente não funciona para máquinas UEFI, já que a crate `bootloader` ainda não tem suporte UEFI.

### Usando `cargo run`

Para facilitar a execução do nosso kernel no QEMU, podemos definir a chave de configuração `runner` para o cargo:

```toml
# em .cargo/config.toml

[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```

A tabela `target.'cfg(target_os = "none")'` se aplica a todos os alvos cujo campo `"os"` do arquivo de configuração de alvo está definido como `"none"`. Isso inclui nosso alvo `x86_64-blog_os.json`. A chave `runner` especifica o comando que deve ser invocado para `cargo run`. O comando é executado após uma compilação bem-sucedida com o caminho do executável passado como o primeiro argumento. Veja a [documentação do cargo][configuração cargo] para mais detalhes.

O comando `bootimage runner` é especificamente projetado para ser utilizável como um executável `runner`. Ele vincula o executável dado com a dependência do bootloader do projeto e então lança o QEMU. Veja o [Readme do `bootimage`] para mais detalhes e opções de configuração possíveis.

[Readme do `bootimage`]: https://github.com/rust-osdev/bootimage

Agora podemos usar `cargo run` para compilar nosso kernel e inicializá-lo no QEMU.

## O que vem a seguir?

No próximo post, exploraremos o buffer de texto VGA em mais detalhes e escreveremos uma interface segura para ele. Também adicionaremos suporte para a macro `println`.
