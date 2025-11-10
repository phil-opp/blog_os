+++
title = "Um Binário Rust Independente"
weight = 1
path = "pt-BR/freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "O Básico"

# Please update this when updating the translation
translation_based_on_commit = "624f0b7663daca1ce67f297f1c450420fbb4d040"

# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

O primeiro passo para criar nosso próprio kernel de sistema operacional é criar um executável Rust que não vincule a biblioteca padrão. Isso torna possível executar o código Rust no [bare metal] sem um sistema operacional subjacente.

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

Este blog é desenvolvido abertamente no [GitHub]. Se você tiver algum problema ou dúvida, abra um issue lá. Você também pode deixar comentários [na parte inferior]. O código-fonte completo desta publicação pode ser encontrado na banch [`post-01`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[na parte inferior]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## Introdução
Para escrever um kernel de sistema operacional, precisamos de código que não dependa de nenhum recurso do sistema operacional. Isso significa que não podemos usar threads, arquivos, memória heap, rede, números aleatórios, saída padrão ou qualquer outro recurso que exija abstrações do sistema operacional ou hardware específico. O que faz sentido, já que estamos tentando escrever nosso próprio sistema operacional e nossos próprios drivers.

Isso significa que não podemos usar a maior parte da [biblioteca padrão do Rust], mas há muitos recursos do Rust que _podemos_ usar. Por exemplo, podemos usar [iteradores], [closures], [pattern matching], [option] e [result], [formatação de string] e, claro, o [sistema de ownership]. Esses recursos tornam possível escrever um kernel de uma maneira muito expressiva e de alto nível, sem nos preocuparmos com [undefined behavior] ou [memory safety].

[option]: https://doc.rust-lang.org/core/option/
[result]:https://doc.rust-lang.org/core/result/
[Rust standard library]: https://doc.rust-lang.org/std/
[iteradores]: https://doc.rust-lang.org/book/ch13-02-iterators.html
[closures]: https://doc.rust-lang.org/book/ch13-01-closures.html
[pattern matching]: https://doc.rust-lang.org/book/ch06-00-enums.html
[formatação de string]: https://doc.rust-lang.org/core/macro.write.html
[sistema de ownership]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
[undefined behavior]: https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs
[memory safety]: https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention

Para criar um kernel de sistema operacional em Rust, precisamos criar um executável que possa ser executado sem um sistema operacional subjacente. Esse executável é frequentemente chamado de executável “autônomo” ou “bare-metal”.

Este post descreve as etapas necessárias para criar um binário Rust independente e explica por que essas etapas são necessárias. Se você estiver interessado apenas em um exemplo mínimo, pode **[ir para o resumo](#resumo)**.

## Desativando a biblioteca padrão
Por padrão, todos as crates Rust vinculam a [biblioteca padrão], que depende do sistema operacional para recursos como threads, arquivos ou rede. Ela também depende da biblioteca padrão C `libc`, que interage intimamente com os serviços do sistema operacional. Como nosso plano é escrever um sistema operacional, não podemos usar nenhuma biblioteca dependente de um sistema operacional. 
Portanto, temos que desativar a inclusão automática da biblioteca padrão por meio do [atributo `no_std`].

[biblioteca padrão]: https://doc.rust-lang.org/std/
[atributo `no_std`]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

Começamos criando um novo projeto de binário cargo. A maneira mais fácil de fazer isso é através da linha de comando:

```
cargo new blog_os --bin --edition 2024
```

Eu nommei o projeto `blog_os`, mas claro que você pode escolher o seu próprio nome. A flag `--bin` especifica que queremos criar um executável binário (em contraste com uma biblioteca) e a flag `--edition 2024` especifica que queremos usar a [edição 2024] de Rust para nossa crate. Quando executamos o comando, o cargo cria a seguinte estrutura de diretório para nós:

[edição 2024]: https://doc.rust-lang.org/nightly/edition-guide/rust-2024/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

O `Cargo.toml` contém a configuração da crate, por exemplo o nome da crate, o autor, o número da [versão semântica] e dependências. O arquivo `src/main.rs` contém o módulo raiz da nossa crate e nossa função `main`. Você pode compilar sua crate através de `cargo build` e então executar o binário compilado `blog_os` na subpasta `target/debug`.

[versão semântica]: https://semver.org/

### O Atributo `no_std`

Agora nossa crate implicitamente vincula a biblioteca padrão. Vamos tentar desativar isso adicionando o [atributo `no_std`]:

```rust
// main.rs

#![no_std]

fn main() {
    println!("Olá, mundo!");
}
```

Quando tentamos compilá-lo agora (executando `cargo build`), o seguinte erro ocorre:

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Olá, mundo!");
  |     ^^^^^^^
```

A razão deste erro é que a [macro `println`] é parte da biblioteca padrão, que não incluímos mais. Então não conseguimos mais imprimir coisas. Isso faz sentido, já que `println` escreve no [standard output], que é um descritor de arquivo especial fornecido pelo sistema operacional.

[macro `println`]: https://doc.rust-lang.org/std/macro.println.html
[standard output]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

Então vamos remover o println!() e tentar novamente com uma função main vazia:

```rust
// main.rs

#![no_std]

fn main() {}
```

```
> cargo build
error: `#[panic_handler]` function required, but not found
error: language item required, but not found: `eh_personality`
```

Agora o compilador está pedindo uma função `#[panic_handler]` e um _item de linguagem_.

##  Implementação de Panic

O atributo `panic_handler` define a função que o compilador deve invocar quando ocorre um [panic]. A biblioteca padrão fornece sua própria função de tratamento de panic, mas em um ambiente `no_std` precisamos defini-la nós mesmos:

[panic]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// in main.rs

use core::panic::PanicInfo;

/// Esta função é chamada em caso de pânico.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

O parâmetro [`PanicInfo`][PanicInfo] contém o arquivo e a linha onde o panic aconteceu e a mensagem de panic opcional. A função nunca deve retornar, então é marcada como uma [função divergente] ao retornar o [tipo “never”] `!`. Não há muito que possamos fazer nesta função por enquanto, então apenas fazemos um loop infinito.

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[função divergente]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[tipo “never”]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## O Item de Linguagem `eh_personality`

Items de linguagem são funções e tipos especiais necessários internamente pelo compilador. Por exemplo, a trait [`Copy`] é um item de linguagem que diz ao compilador quais tipos têm [_semântica de cópia_][`Copy`]. Quando olhamos para a [implementação][copy code], vemos que tem o atributo especial `#[lang = "copy"]` que o define como um item de linguagem (_Language Item_ em inglês).

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

Enquanto é possível fornecer implementações customizadas de items de linguagem, isso deve ser feito apenas como último recurso. A razão é que items de linguagem são detalhes de implementação altamente instáveis e nem mesmo verificados de tipo (então o compilador não verifica se uma função tem os tipos de argumento corretos). Felizmente, há uma forma mais estável de corrigir o erro de item de linguagem acima.

O [item de linguagem `eh_personality`] marca uma função que é usada para implementar [stack unwinding]. Por padrão, Rust usa unwinding para executar os destructores de todas as variáveis da stack vivas em caso de [panic]. Isso garante que toda memória usada seja liberada e permite que a thread pai capture o panic e continue a execução. Unwinding, no entanto, é um processo complicado e requer algumas bibliotecas específicas do SO (por exemplo, [libunwind] no Linux ou [tratamento estruturado de exceção] no Windows), então não queremos usá-lo para nosso sistema operacional.

[item de linguagem `eh_personality`]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: https://www.nongnu.org/libunwind/
[tratamento estruturado de exceção]: https://docs.microsoft.com/en-us/windows/win32/debug/structured-exception-handling

### Desativando o Unwinding

Existem outros casos de uso também para os quais unwinding é indesejável, então Rust fornece uma opção para [abortar no panic] em vez disso. Isso desativa a geração de informações de símbolo de desenrolar e reduz consideravelmente o tamanho do binário. Há múltiplos locais onde podemos desativar o unwinding. A forma mais fácil é adicionar as seguintes linhas ao nosso `Cargo.toml`:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

Isso define a estratégia de panic para `abort` tanto para o perfil `dev` (usado para `cargo build`) quanto para o perfil `release` (usado para `cargo build --release`). Agora o item de linguagem `eh_personality` não deve mais ser necessário.

[abortar no panic]: https://github.com/rust-lang/rust/pull/32900

Agora corrigimos ambos os erros acima. No entanto, se tentarmos compilar agora, outro erro ocorre:

```
> cargo build
error: requires `start` lang_item
```

Está faltando o item de linguagem `start` no nosso programa, que define o ponto de entrada.

## O Atributo `start`

Alguém pode pensar que a função `main` é a primeira função chamada quando você executa um programa. No entanto, a maioria das linguagens tem um [sistema de runtime], que é responsável por coisas como coleta de lixo (por exemplo, em Java) ou threads de software (por exemplo, goroutines em Go). Este runtime precisa ser chamado antes de `main`, já que ele precisa se inicializar a si mesmo.

[sistema de runtime]: https://en.wikipedia.org/wiki/Runtime_system

Em um binário Rust típico que vincula a biblioteca padrão, a execução começa em uma biblioteca de runtime C chamada `crt0` ("C runtime zero"), que configura o ambiente para uma aplicação C. Isso inclui criar um stack e colocar os argumentos nos registradores certos. O runtime C então invoca o [ponto de entrada do runtime Rust][rt::lang_start], que é marcado pelo item de linguagem `start`. Rust tem apenas um runtime muito mínimo, que cuida de algumas poucas coisas como configurar guardas de estouro do stack ou imprimir um backtrace ao fazer panic. O runtime então finalmente chama a função `main`.

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

Nosso executável independente não tem acesso ao runtime Rust e ao `crt0`, então precisamos definir nosso próprio ponto de entrada. Implementar o item de linguagem `start` não ajudaria, já que ainda exigiria `crt0`. Em vez disso, precisamos sobrescrever diretamente o ponto de entrada `crt0`.

### Sobrescrevendo o Ponto de Entrada (Entry Point)
Para dizer ao compilador Rust que não queremos usar a cadeia normal de ponto de entrada, adicionamos o atributo `#![no_main]`.

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// Esta função é chamada em caso de pânico.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

Você pode notar que removemos a função `main`. A razão é que um `main` não faz sentido sem um runtime subjacente que o chame. Em vez disso, estamos agora sobrescrevendo o ponto de entrada do sistema operacional com nossa própria função `_start`:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

Ao usar o atributo `#[unsafe(no_mangle)]`, desativamos [mangling de nomes] para garantir que o compilador Rust realmente produza uma função com o nome `_start`. Sem o atributo, o compilador geraria algum símbolo criptografado como `_ZN3blog_os4_start7hb173fedf945531caE` para dar a cada função um nome único. O atributo é necessário porque precisamos dizer o nome da função do ponto de entrada ao linker no próximo passo.

Também temos que marcar a função como `extern "C"` para dizer ao compilador que ele deve usar a [convenção de chamada C] para esta função (em vez da convenção de chamada Rust não especificada). A razão de nomear a função `_start` é que este é o nome do ponto de entrada padrão para a maioria dos sistemas.

[mangling de nomes]: https://en.wikipedia.org/wiki/Name_mangling
[convenção de chamada C]: https://en.wikipedia.org/wiki/Calling_convention

O tipo de retorno `!` significa que a função é divergente, ou seja, não é permitida retornar nunca. Isso é necessário porque o ponto de entrada não é chamado por nenhuma função, mas invocado diretamente pelo sistema operacional ou bootloader. Então em vez de retornar, o ponto de entrada deve por exemplo invocar a [chamada de sistema `exit`] do sistema operacional. No nosso caso, desligar a máquina poderia ser uma ação razoável, já que não há nada mais a fazer se um binário independente retorna. Por enquanto, cumprimos o requisito fazendo um loop infinito.

[chamada de sistema `exit`]: https://en.wikipedia.org/wiki/Exit_(system_call)

Quando executamos `cargo build` agora, recebemos um feio erro de _linker_.

## Erros do Linker

O linker é um programa que combina o código gerado em um executável. Como o formato executável difere entre Linux, Windows e macOS, cada sistema tem seu próprio linker que lança um erro diferente. A causa fundamental dos erros é a mesma: a configuração padrão do linker assume que nosso programa depende do runtime C, o que não é o caso.

Para resolver os erros, precisamos dizer ao linker que ele não deve incluir o runtime C. Podemos fazer isso passando um certo conjunto de argumentos ao linker ou compilando para um alvo bare metal.

### Compilando para um Alvo Bare Metal

Por padrão, Rust tenta construir um executável que seja capaz de executar no seu ambiente de sistema atual. Por exemplo, se você estiver usando Windows em `x86_64`, Rust tenta construir um executável `.exe` Windows que usa instruções `x86_64`. Este ambiente é chamado seu sistema "host".

Para descrever ambientes diferentes, Rust usa uma string chamada [_target triple_]. Você pode ver o target triple do seu sistema host executando `rustc --version --verbose`:

[_target triple_]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple

```
rustc 1.91.0 (f8297e351 2025-10-28)
binary: rustc
commit-hash: f8297e351a40c1439a467bbbb6879088047f50b3
commit-date: 2025-10-28
host: x86_64-unknown-linux-gnu
release: 1.91.0
LLVM version: 21.1.2
```

A saída acima é de um sistema Linux `x86_64`. Vemos que o triple `host` é `x86_64-unknown-linux-gnu`, que inclui a arquitetura de CPU (`x86_64`), o vendor (`unknown`), o sistema operacional (`linux`), e a [ABI] (`gnu`).

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

Ao compilar para nosso triple host, o compilador Rust e o linker assumem que há um sistema operacional subjacente como Linux ou Windows que usa o runtime C por padrão, o que causa os erros do linker. Então, para evitar os erros do linker, podemos compilar para um ambiente diferente sem nenhum sistema operacional subjacente.

Um exemplo de tal ambiente bare metal é o target triple `thumbv7em-none-eabihf`, que descreve um sistema [embarcado] [ARM]. Os detalhes não são importantes, tudo o que importa é que o target triple não tem nenhum sistema operacional subjacente, o que é indicado pelo `none` no target triple. Para ser capaz de compilar para este alvo, precisamos adicioná-lo em rustup:

[embarcado]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture

```
rustup target add thumbv7em-none-eabihf
```

Isso baixa uma cópia da biblioteca padrão std (e core) para o sistema. Agora podemos compilar nosso executável independente para este alvo:

```
cargo build --target thumbv7em-none-eabihf
```

Ao passar um argumento `--target`, nós fazemos uma compilação [cross compile] nosso executável para um sistema alvo bare metal. Como o sistema alvo não tem sistema operacional, o linker não tenta vincular o runtime C e nossa compilação é bem-sucedida sem nenhum erro de linker.

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

Esta é a abordagem que usaremos para construir nosso kernel de SO. Em vez de `thumbv7em-none-eabihf`, usaremos um [alvo customizado] que descreve um ambiente bare metal `x86_64`. Os detalhes serão explicados no próximo post.

[alvo customizado]: https://doc.rust-lang.org/rustc/targets/custom.html

### Argumentos do Linker

Em vez de compilar para um sistema bare metal, também é possível resolver os erros do linker passando um certo conjunto de argumentos ao linker. Esta não é a abordagem que usaremos para nosso kernel, portanto esta seção é opcional e fornecida apenas para completude. Clique em _"Argumentos do Linker"_ abaixo para mostrar o conteúdo opcional.

<details>

<summary>Argumentos do Linker</summary>

Nesta seção discutimos os erros do linker que ocorrem no Linux, Windows e macOS, e explicamos como resolvê-los passando argumentos adicionais ao linker. Note que o formato executável e o linker diferem entre sistemas operacionais, então que um conjunto diferente de argumentos é necessário para cada sistema.

#### Linux

No Linux, o seguinte erro de linker ocorre (encurtado):

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

O problema é que o linker inclui a rotina de inicialização do runtime C por padrão, que também é chamada `_start`. Ela requer alguns símbolos da biblioteca padrão C `libc` que não incluímos devido ao atributo `no_std`, portanto o linker não consegue resolver estas referências. Para resolver isso, podemos dizer ao linker que ele não deve vincular a rotina de inicialização C passando a flag `-nostartfiles`.

Uma forma de passar atributos de linker via cargo é o comando `cargo rustc`. O comando se comporta exatamente como `cargo build`, mas permite passar opções para `rustc`, o compilador Rust subjacente. `rustc` tem a flag `-C link-arg`, que passa um argumento ao linker. Combinados, nosso novo comando de compilação se parece com isso:

```
cargo rustc -- -C link-arg=-nostartfiles
```

Agora nossa crate compilada como um executável independente no Linux!

Não precisávamos especificar o nome da nossa função de ponto de entrada explicitamente, já que o linker procura por uma função com o nome `_start` por padrão.

#### Windows

No Windows, um erro de linker diferente ocorre (encurtado):

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

O erro "entry point must be defined" (ponto de entrada deve ser definido) significa que o linker não consegue encontrar o ponto de entrada. No Windows, o nome do ponto de entrada padrão [depende do subsistema usado][windows-subsystems]. Para o subsistema `CONSOLE`, o linker procura por uma função nomeada `mainCRTStartup` e para o subsistema `WINDOWS`, ele procura por uma função nomeada `WinMainCRTStartup`. Para sobrescrever o padrão e dizer ao linker para procurar por nossa função `_start` em vez disso, podemos passar um argumento `/ENTRY` ao linker:

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

Do formato de argumento diferente, vemos claramente que o linker Windows é um programa completamente diferente do linker Linux.

Agora um erro de linker diferente ocorre:

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

Este erro ocorre porque os executáveis Windows podem usar [subsistemas] diferentes[windows-subsystems]. Para programas normais, eles são inferidos dependendo do nome do ponto de entrada: Se o ponto de entrada é nomeado `main`, o subsistema `CONSOLE` é usado, e se o ponto de entrada é nomeado `WinMain`, o subsistema `WINDOWS` é usado. Como nossa função `_start` tem um nome diferente, precisamos especificar o subsistema explicitamente:

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

Usamos o subsistema `CONSOLE` aqui, mas o subsistema `WINDOWS` funcionaria também. Em vez de passar `-C link-arg` múltiplas vezes, usamos `-C link-args` que leva uma lista de argumentos separados por espaço.

Com este comando, nosso executável deve compilar com sucesso no Windows.

#### macOS

No macOS, o seguinte erro de linker ocorre (encurtado):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

Esta mensagem de erro nos diz que o linker não consegue encontrar uma função de ponto de entrada com o nome padrão `main` (por alguma razão, todas as funções são prefixadas com um `_` no macOS). Para definir o ponto de entrada para nossa função `_start`, passamos o argumento de linker `-e`:

```
cargo rustc -- -C link-args="-e __start"
```

A flag `-e` especifica o nome da função de ponto de entrada. Como todas as funções têm um `_` adicional prefixado no macOS, precisamos definir o ponto de entrada para `__start` em vez de `_start`.

Agora o seguinte erro de linker ocorre:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS [não oferece suporte oficial a binários vinculados estaticamente] e requer que programas vinculem a biblioteca `libSystem` por padrão. Para sobrescrever isto e vincular um binário estático, passamos a flag `-static` ao linker:

[não oferece suporte oficial a binários vinculados estaticamente]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

Isso ainda não é suficiente, pois um terceiro erro de linker ocorre:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

Este erro ocorre porque programas no macOS vinculam a `crt0` ("C runtime zero") por padrão. Isto é similar ao erro que tivemos no Linux e também pode ser resolvido adicionando o argumento de linker `-nostartfiles`:

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Agora nosso programa deve compilar com sucesso no macOS.

#### Unificando os Comandos de Compilação

Agora temos diferentes comandos de compilação dependendo da plataforma host, o que não é o ideal. Para evitar isto, podemos criar um arquivo nomeado `.cargo/config.toml` que contém os argumentos específicos de plataforma:

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

A key `rustflags` contém argumentos que são automaticamente adicionados a cada invocação de `rustc`. Para mais informações sobre o arquivo `.cargo/config.toml`, veja a [documentação oficial](https://doc.rust-lang.org/cargo/reference/config.html).

Agora nosso programa deve ser compilável em todas as três plataformas com um simples `cargo build`.

#### Você Deveria Fazer Isto?

Enquanto é possível construir um executável independente para Linux, Windows e macOS, provavelmente não é uma boa ideia. A razão é que nosso executável ainda espera por várias coisas, por exemplo que uma pilha seja inicializada quando a função `_start` é chamada. Sem o runtime C, alguns desses requisitos podem não ser atendidos, o que pode causar nosso programa falhar, por exemplo através de um segmentation fault.

Se você quiser criar um binário mínimo que execute em cima de um sistema operacional existente, incluindo `libc` e definindo o atributo `#[start]` conforme descrito [aqui](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html) é provavelmente uma melhor ideia.

</details>

## Resumo

Um binário Rust independente mínimo se parece com isto:

`src/main.rs`:

```rust
#![no_std] // Não vincule a biblioteca padrão do Rust
#![no_main] // desativar todos os pontos de entrada no nível Rust

use core::panic::PanicInfo;

#[unsafe(no_mangle)] // não altere (mangle) o nome desta função
pub extern "C" fn _start() -> ! {
    // essa função é o ponto de entrada, já que o vinculador procura uma função
    // denominado `_start` por padrão
    loop {}
}

/// Esta função é chamada em caso de pânico.
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

# o perfil usado para `cargo build`
[profile.dev]
panic = "abort" # desativar o unwinding do stack em caso de pânico

# o perfil usado para `cargo build --release`
[profile.release]
panic = "abort" # desativar o unwinding do stack em caso de pânico
```

Para construir este binário, precisamos compilar para um alvo bare metal como `thumbv7em-none-eabihf`:

```
cargo build --target thumbv7em-none-eabihf
```

Alternativamente, podemos compilá-lo para o sistema host passando argumentos adicionais de linker:

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Note que isto é apenas um exemplo mínimo de um binário Rust independente. Este binário espera por várias coisas, por exemplo, que um stack seja inicializado quando a função `_start` é chamada. **Portanto para qualquer uso real de tal binário, mais passos são necessários**.

## Deixando `rust-analyzer` Feliz

O projeto [`rust-analyzer`](https://rust-analyzer.github.io/) é uma ótima forma de obter autocompletar e suporte "ir para definição" (e muitos outros recursos) para código Rust no seu editor.
Funciona muito bem para projetos `#![no_std]` também, então recomendo usá-lo para desenvolvimento de kernel!

Se você estiver usando a funcionalidade [`checkOnSave`](https://rust-analyzer.github.io/book/configuration.html#checkOnSave) de `rust-analyzer` (habilitada por padrão), ela pode relatar um erro para a função panic do nosso kernel:

```
found duplicate lang item `panic_impl`
```

A razão para este erro é que `rust-analyzer` invoca `cargo check --all-targets` por padrão, que também tenta construir o binário em modo [teste](https://doc.rust-lang.org/book/ch11-01-writing-tests.html) e [benchmark](https://doc.rust-lang.org/rustc/tests/index.html#benchmarks).

<div class="note">

### Os dois significados de "target"

A flag `--all-targets` é completamente não relacionada ao argumento `--target`.
Há dois significados diferentes do termo "target" no `cargo`:

- A flag `--target` especifica o [_alvo de compilação_] que deve ser passado ao compilador `rustc`. Isso deve ser definido como o [target triple] da máquina que deve executar nosso código.
- A flag `--all-targets` referencia o [_alvo do package] do Cargo. Pacotes Cargo podem ser uma biblioteca e binário ao mesmo tempo, então você pode especificar de qual forma você gostaria de construir sua crate. Além disso, Cargo também tem alvos de package para [exemplos](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#examples), [testes](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#tests), e [benchmarks](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#benchmarks). Esses alvos de pacote podem coexistir, então você pode construir/verificar a mesma crate por exemplo em modo biblioteca ou modo teste.

[_alvo de compilação_]: https://doc.rust-lang.org/rustc/targets/index.html
[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[_alvo do package]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html

</div>

Por padrão, `cargo check` apenas constrói o _biblioteca_ e os alvos de pacote _binário_.
No entanto, `rust-analyzer` escolhe verificar todos os alvos de pacote por padrão quando [`checkOnSave`](https://rust-analyzer.github.io/book/configuration.html#checkOnSave) é habilitado.
Esta é a razão pela qual `rust-analyzer` relata o erro de `lang item` acima que não vemos em `cargo check`.
Se executarmos `cargo check --all-targets`, vemos o erro também:

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

A primeira `note` nos diz que o item de linguagem panic já está definido na crate `std`, que é uma dependência da crate `test`.
A crate `test` é automaticamente incluída ao construir uma crate em [modo teste](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#tests).
Isso não faz sentido para nosso kernel `#![no_std]` já que não há forma de suportar a biblioteca padrão em bare metal.
Então este erro não é relevante para nosso projeto e podemos seguramente ignorá-lo.

A forma apropriada de evitar este erro é especificar em nosso `Cargo.toml` que nosso binário não suporta construção em modos `test` e `bench`.
Podemos fazer isso adicionando uma seção `[[bin]]` em nosso `Cargo.toml` para [configurar a construção](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#configuring-a-target) do nosso binário:

```toml
# no Cargo.toml

[[bin]]
name = "blog_os"
test = false
bench = false
```

Os colchetes duplos ao redor de `bin` não é um erro, isto é como o formato TOML define chaves que podem aparecer múltiplas vezes.
Como uma crate pode ter múltiplos binários, a seção `[[bin]]` pode aparecer múltiplas vezes em `Cargo.toml` também.
Esta é também a razão para o campo `name` obrigatório, que precisa corresponder ao nome do binário (para que `cargo` saiba quais configurações devem ser aplicadas a qual binário).

Ao definir os campos [`test`](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-test-field) e [`bench` ](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-bench-field) para `false`, instruímos `cargo` a não construir nosso binário em modo teste ou benchmark.
Agora `cargo check --all-targets` não deve lançar mais erros, e a implementação de `checkOnSave` de `rust-analyzer` também deve estar feliz.

## O que vem a seguir?

O [próximo post] explica os passos necessários para transformar nosso binário independente em um kernel mínimo do sistema operacional. Isso inclui criar um alvo customizado, combinar nosso executável com um bootloader, e aprender como imprimir algo na tela.

[próximo post]: @/edition-2/posts/02-minimal-rust-kernel/index.md
