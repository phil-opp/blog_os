+++
title = "Alocação no Heap"
weight = 10
path = "pt-BR/heap-allocation"
date = 2019-06-26

[extra]
chapter = "Gerenciamento de Memória"
# Please update this when updating the translation
translation_based_on_commit = "1ba06fe61c39c1379bd768060c21040b62ff3f0b"
# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

Este post adiciona suporte para alocação no heap ao nosso kernel. Primeiro, ele fornece uma introdução à memória dinâmica e mostra como o verificador de empréstimos previne erros comuns de alocação. Em seguida, implementa a interface básica de alocação do Rust, cria uma região de memória heap e configura uma crate de alocador. Ao final deste post, todos os tipos de alocação e coleção da crate embutida `alloc` estarão disponíveis para o nosso kernel.

<!-- more -->

Este blog é desenvolvido abertamente no [GitHub]. Se você tiver algum problema ou pergunta, por favor abra uma issue lá. Você também pode deixar comentários [no final]. O código-fonte completo para este post pode ser encontrado no branch [`post-10`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-10

<!-- toc -->

## Variáveis Locais e Estáticas

Atualmente usamos dois tipos de variáveis em nosso kernel: variáveis locais e variáveis `static`. Variáveis locais são armazenadas na [pilha de chamadas] e são válidas apenas até que a função circundante retorne. Variáveis estáticas são armazenadas em um local de memória fixo e sempre vivem pela duração completa do programa.

### Variáveis Locais

Variáveis locais são armazenadas na [pilha de chamadas], que é uma [estrutura de dados de pilha] que suporta operações de `push` e `pop`. Em cada entrada de função, os parâmetros, o endereço de retorno e as variáveis locais da função chamada são colocados na pilha pelo compilador:

[pilha de chamadas]: https://en.wikipedia.org/wiki/Call_stack
[estrutura de dados de pilha]: https://en.wikipedia.org/wiki/Stack_(abstract_data_type)

![Uma função `outer()` e uma função `inner(i: usize)`, onde `outer` chama `inner(1)`. Ambas têm algumas variáveis locais. A pilha de chamadas contém os seguintes slots: as variáveis locais de outer, então o argumento `i = 1`, então o endereço de retorno, então as variáveis locais de inner.](call-stack.svg)

O exemplo acima mostra a pilha de chamadas depois que a função `outer` chamou a função `inner`. Vemos que a pilha de chamadas contém as variáveis locais de `outer` primeiro. Na chamada de `inner`, o parâmetro `1` e o endereço de retorno da função foram colocados na pilha. Então o controle foi transferido para `inner`, que colocou suas variáveis locais na pilha.

Depois que a função `inner` retorna, sua parte da pilha de chamadas é removida novamente e apenas as variáveis locais de `outer` permanecem:

![A pilha de chamadas contendo apenas as variáveis locais de `outer`](call-stack-return.svg)

Vemos que as variáveis locais de `inner` vivem apenas até a função retornar. O compilador Rust impõe esses tempos de vida e gera um erro quando usamos um valor por muito tempo, por exemplo, quando tentamos retornar uma referência a uma variável local:

```rust
fn inner(i: usize) -> &'static u32 {
    let z = [1, 2, 3];
    &z[i]
}
```

([execute o exemplo no playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=6186a0f3a54f468e1de8894996d12819))

Embora retornar uma referência não faça sentido neste exemplo, há casos em que queremos que uma variável viva mais tempo do que a função. Já vimos tal caso em nosso kernel quando tentamos [carregar uma tabela de descritores de interrupção] e tivemos que usar uma variável `static` para estender o tempo de vida.

[carregar uma tabela de descritores de interrupção]: @/edition-2/posts/05-cpu-exceptions/index.md#loading-the-idt

### Variáveis Estáticas

Variáveis estáticas são armazenadas em um local de memória fixo separado da pilha. Este local de memória é atribuído em tempo de compilação pelo linker e codificado no executável. Variáveis estáticas vivem pela duração completa de execução do programa, então têm o tempo de vida `'static` e sempre podem ser referenciadas de variáveis locais:

![O mesmo exemplo outer/inner, exceto que inner tem um `static Z: [u32; 3] = [1,2,3];` e retorna uma referência `&Z[i]`](call-stack-static.svg)

Quando a função `inner` retorna no exemplo acima, sua parte da pilha de chamadas é destruída. As variáveis estáticas vivem em um intervalo de memória separado que nunca é destruído, então a referência `&Z[1]` ainda é válida após o retorno.

Além do tempo de vida `'static`, variáveis estáticas também têm a propriedade útil de que sua localização é conhecida em tempo de compilação, de modo que nenhuma referência é necessária para acessá-las. Utilizamos essa propriedade para nossa macro `println`: Ao usar um [`Writer` estático] internamente, nenhuma referência `&mut Writer` é necessária para invocar a macro, o que é muito útil em [manipuladores de exceção], onde não temos acesso a variáveis adicionais.

[`Writer` estático]: @/edition-2/posts/03-vga-text-buffer/index.md#a-global-interface
[manipuladores de exceção]: @/edition-2/posts/05-cpu-exceptions/index.md#implementation

No entanto, essa propriedade de variáveis estáticas traz uma desvantagem crucial: elas são somente leitura por padrão. Rust impõe isso porque uma [corrida de dados] ocorreria se, por exemplo, duas threads modificassem uma variável estática ao mesmo tempo. A única maneira de modificar uma variável estática é encapsulá-la em um tipo [`Mutex`], que garante que apenas uma referência `&mut` exista em qualquer momento. Já usamos um `Mutex` para nosso [`Writer` estático do buffer VGA][vga mutex].

[corrida de dados]: https://doc.rust-lang.org/nomicon/races.html
[`Mutex`]: https://docs.rs/spin/0.5.2/spin/struct.Mutex.html
[vga mutex]: @/edition-2/posts/03-vga-text-buffer/index.md#spinlocks

## Memória Dinâmica

Variáveis locais e estáticas já são muito poderosas juntas e permitem a maioria dos casos de uso. No entanto, vimos que ambas têm suas limitações:

- Variáveis locais vivem apenas até o final da função ou bloco circundante. Isso ocorre porque elas vivem na pilha de chamadas e são destruídas depois que a função circundante retorna.
- Variáveis estáticas sempre vivem pela duração completa de execução do programa, então não há maneira de recuperar e reutilizar sua memória quando não são mais necessárias. Além disso, elas têm semântica de propriedade pouco clara e são acessíveis de todas as funções, então precisam ser protegidas por um [`Mutex`] quando queremos modificá-las.

Outra limitação de variáveis locais e estáticas é que elas têm um tamanho fixo. Então elas não podem armazenar uma coleção que cresce dinamicamente quando mais elementos são adicionados. (Existem propostas para [rvalues não dimensionados] em Rust que permitiriam variáveis locais de tamanho dinâmico, mas eles só funcionam em alguns casos específicos.)

[rvalues não dimensionados]: https://github.com/rust-lang/rust/issues/48055

Para contornar essas desvantagens, linguagens de programação frequentemente suportam uma terceira região de memória para armazenar variáveis chamada **heap**. O heap suporta _alocação de memória dinâmica_ em tempo de execução através de duas funções chamadas `allocate` e `deallocate`. Funciona da seguinte maneira: A função `allocate` retorna um pedaço livre de memória do tamanho especificado que pode ser usado para armazenar uma variável. Esta variável então vive até ser liberada chamando a função `deallocate` com uma referência à variável.

Vamos passar por um exemplo:

![A função inner chama `allocate(size_of([u32; 3]))`, escreve `z.write([1,2,3]);`, e retorna `(z as *mut u32).offset(i)`. No valor retornado `y`, a função outer realiza `deallocate(y, size_of(u32))`.](call-stack-heap.svg)

Aqui a função `inner` usa memória heap em vez de variáveis estáticas para armazenar `z`. Primeiro ela aloca um bloco de memória do tamanho necessário, que retorna um [ponteiro bruto] `*mut u32`. Em seguida, usa o método [`ptr::write`] para escrever o array `[1,2,3]` nele. No último passo, usa a função [`offset`] para calcular um ponteiro para o `i`-ésimo elemento e então o retorna. (Note que omitimos alguns casts e blocos unsafe necessários nesta função de exemplo por brevidade.)

[ponteiro bruto]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`ptr::write`]: https://doc.rust-lang.org/core/ptr/fn.write.html
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

A memória alocada vive até ser explicitamente liberada através de uma chamada para `deallocate`. Assim, o ponteiro retornado ainda é válido mesmo depois que `inner` retornou e sua parte da pilha de chamadas foi destruída. A vantagem de usar memória heap comparada à memória estática é que a memória pode ser reutilizada depois de ser liberada, o que fazemos através da chamada `deallocate` em `outer`. Depois dessa chamada, a situação se parece com isso:

![A pilha de chamadas contém as variáveis locais de `outer`, o heap contém `z[0]` e `z[2]`, mas não mais `z[1]`.](call-stack-heap-freed.svg)

Vemos que o slot `z[1]` está livre novamente e pode ser reutilizado para a próxima chamada `allocate`. No entanto, também vemos que `z[0]` e `z[2]` nunca são liberados porque nunca os desalocamos. Tal bug é chamado de _vazamento de memória_ e é frequentemente a causa do consumo excessivo de memória de programas (imagine apenas o que acontece quando chamamos `inner` repetidamente em um loop). Isso pode parecer ruim, mas existem tipos muito mais perigosos de bugs que podem acontecer com alocação dinâmica.

### Erros Comuns

Além de vazamentos de memória, que são lamentáveis mas não tornam o programa vulnerável a atacantes, existem dois tipos comuns de bugs com consequências mais graves:

- Quando acidentalmente continuamos a usar uma variável depois de chamar `deallocate` nela, temos uma chamada vulnerabilidade **use-after-free**. Tal bug causa comportamento indefinido e pode frequentemente ser explorado por atacantes para executar código arbitrário.
- Quando acidentalmente liberamos uma variável duas vezes, temos uma vulnerabilidade **double-free**. Isso é problemático porque pode liberar uma alocação diferente que foi alocada no mesmo local após a primeira chamada `deallocate`. Assim, pode levar a uma vulnerabilidade use-after-free novamente.

Esses tipos de vulnerabilidades são comumente conhecidos, então pode-se esperar que as pessoas tenham aprendido como evitá-los até agora. Mas não, tais vulnerabilidades ainda são encontradas regularmente, por exemplo esta [vulnerabilidade use-after-free no Linux][linux vulnerability] (2019), que permitiu execução de código arbitrário. Uma busca na web como `use-after-free linux {ano atual}` provavelmente sempre produzirá resultados. Isso mostra que mesmo os melhores programadores nem sempre são capazes de lidar corretamente com memória dinâmica em projetos complexos.

[linux vulnerability]: https://securityboulevard.com/2019/02/linux-use-after-free-vulnerability-found-in-linux-2-6-through-4-20-11/

Para evitar esses problemas, muitas linguagens, como Java ou Python, gerenciam memória dinâmica automaticamente usando uma técnica chamada [_coleta de lixo_]. A ideia é que o programador nunca invoca `deallocate` manualmente. Em vez disso, o programa é regularmente pausado e escaneado em busca de variáveis heap não utilizadas, que são então automaticamente desalocadas. Assim, as vulnerabilidades acima nunca podem ocorrer. As desvantagens são a sobrecarga de desempenho do escaneamento regular e os tempos de pausa provavelmente longos.

[_coleta de lixo_]: https://en.wikipedia.org/wiki/Garbage_collection_(computer_science)

Rust adota uma abordagem diferente para o problema: Ele usa um conceito chamado [_propriedade_] que é capaz de verificar a correção das operações de memória dinâmica em tempo de compilação. Assim, nenhuma coleta de lixo é necessária para evitar as vulnerabilidades mencionadas, o que significa que não há sobrecarga de desempenho. Outra vantagem dessa abordagem é que o programador ainda tem controle refinado sobre o uso de memória dinâmica, assim como com C ou C++.

[_propriedade_]: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html

### Alocações em Rust

Em vez de deixar o programador chamar `allocate` e `deallocate` manualmente, a biblioteca padrão do Rust fornece tipos de abstração que chamam essas funções implicitamente. O tipo mais importante é [**`Box`**], que é uma abstração para um valor alocado no heap. Ele fornece uma função construtora [`Box::new`] que recebe um valor, chama `allocate` com o tamanho do valor e então move o valor para o slot recém-alocado no heap. Para liberar a memória heap novamente, o tipo `Box` implementa a [trait `Drop`] para chamar `deallocate` quando sai do escopo:

[**`Box`**]: https://doc.rust-lang.org/std/boxed/index.html
[`Box::new`]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html#method.new
[trait `Drop`]: https://doc.rust-lang.org/book/ch15-03-drop.html

```rust
{
    let z = Box::new([1,2,3]);
    […]
} // z sai do escopo e `deallocate` é chamado
```

Esse padrão tem o nome estranho [_aquisição de recurso é inicialização_] (ou _RAII_ abreviado). Ele se originou em C++, onde é usado para implementar um tipo de abstração similar chamado [`std::unique_ptr`].

[_aquisição de recurso é inicialização_]: https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization
[`std::unique_ptr`]: https://en.cppreference.com/w/cpp/memory/unique_ptr

Tal tipo sozinho não é suficiente para prevenir todos os bugs use-after-free, já que programadores ainda podem manter referências depois que o `Box` sai do escopo e o slot de memória heap correspondente é desalocado:

```rust
let x = {
    let z = Box::new([1,2,3]);
    &z[1]
}; // z sai do escopo e `deallocate` é chamado
println!("{}", x);
```

É aqui que a propriedade do Rust entra. Ela atribui um [tempo de vida] abstrato a cada referência, que é o escopo no qual a referência é válida. No exemplo acima, a referência `x` é retirada do array `z`, então ela se torna inválida depois que `z` sai do escopo. Quando você [executa o exemplo acima no playground][playground-2], você vê que o compilador Rust de fato gera um erro:

[tempo de vida]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
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
5 |     }; // z sai do escopo e `deallocate` é chamado
  |     - `z[_]` dropped here while still borrowed
```

A terminologia pode ser um pouco confusa no início. Pegar uma referência a um valor é chamado de _emprestar_ o valor, já que é similar a um empréstimo na vida real: Você tem acesso temporário a um objeto mas precisa devolvê-lo em algum momento, e você não deve destruí-lo. Ao verificar que todos os empréstimos terminam antes que um objeto seja destruído, o compilador Rust pode garantir que nenhuma situação use-after-free pode ocorrer.

O sistema de propriedade do Rust vai ainda mais longe, prevenindo não apenas bugs use-after-free mas também fornecendo [_segurança de memória_] completa, como linguagens com coleta de lixo como Java ou Python fazem. Adicionalmente, ele garante [_segurança de thread_] e assim é ainda mais seguro que essas linguagens em código multi-thread. E mais importante, todas essas verificações acontecem em tempo de compilação, então não há sobrecarga em tempo de execução comparado ao gerenciamento de memória manual em C.

[_segurança de memória_]: https://en.wikipedia.org/wiki/Memory_safety
[_segurança de thread_]: https://en.wikipedia.org/wiki/Thread_safety

### Casos de Uso

Agora sabemos o básico de alocação de memória dinâmica em Rust, mas quando devemos usá-la? Chegamos muito longe com nosso kernel sem alocação de memória dinâmica, então por que precisamos dela agora?

Primeiro, alocação de memória dinâmica sempre vem com um pouco de sobrecarga de desempenho, já que precisamos encontrar um slot livre no heap para cada alocação. Por essa razão, variáveis locais geralmente são preferíveis, especialmente em código kernel sensível ao desempenho. No entanto, existem casos em que alocação de memória dinâmica é a melhor escolha.

Como regra básica, memória dinâmica é necessária para variáveis que têm um tempo de vida dinâmico ou um tamanho variável. O tipo mais importante com tempo de vida dinâmico é [**`Rc`**], que conta as referências ao seu valor encapsulado e o desaloca depois que todas as referências saíram do escopo. Exemplos de tipos com tamanho variável são [**`Vec`**], [**`String`**] e outros [tipos de coleção] que crescem dinamicamente quando mais elementos são adicionados. Esses tipos funcionam alocando uma quantidade maior de memória quando ficam cheios, copiando todos os elementos e então desalocando a alocação antiga.

[**`Rc`**]: https://doc.rust-lang.org/alloc/rc/index.html
[**`Vec`**]: https://doc.rust-lang.org/alloc/vec/index.html
[**`String`**]: https://doc.rust-lang.org/alloc/string/index.html
[tipos de coleção]: https://doc.rust-lang.org/alloc/collections/index.html

Para o nosso kernel, precisaremos principalmente dos tipos de coleção, por exemplo, para armazenar uma lista de tarefas ativas ao implementar multitarefa em posts futuros.

## A Interface do Alocador

O primeiro passo na implementação de um alocador heap é adicionar uma dependência na crate embutida [`alloc`]. Como a crate [`core`], ela é um subconjunto da biblioteca padrão que adicionalmente contém os tipos de alocação e coleção. Para adicionar a dependência em `alloc`, adicionamos o seguinte ao nosso `lib.rs`:

[`alloc`]: https://doc.rust-lang.org/alloc/
[`core`]: https://doc.rust-lang.org/core/

```rust
// em src/lib.rs

extern crate alloc;
```

Ao contrário de dependências normais, não precisamos modificar o `Cargo.toml`. A razão é que a crate `alloc` vem com o compilador Rust como parte da biblioteca padrão, então o compilador já conhece a crate. Ao adicionar esta declaração `extern crate`, especificamos que o compilador deve tentar incluí-la. (Historicamente, todas as dependências precisavam de uma declaração `extern crate`, que agora é opcional).

Como estamos compilando para um alvo personalizado, não podemos usar a versão pré-compilada de `alloc` que vem com a instalação do Rust. Em vez disso, temos que dizer ao cargo para recompilar a crate a partir do código-fonte. Podemos fazer isso adicionando-a ao array `unstable.build-std` em nosso arquivo `.cargo/config.toml`:

```toml
# em .cargo/config.toml

[unstable]
build-std = ["core", "compiler_builtins", "alloc"]
```

Agora o compilador irá recompilar e incluir a crate `alloc` em nosso kernel.

A razão pela qual a crate `alloc` é desabilitada por padrão em crates `#[no_std]` é que ela tem requisitos adicionais. Quando tentamos compilar nosso projeto agora, veremos esses requisitos como erros:

```
error: no global memory allocator found but one is required; link to std or add
       #[global_allocator] to a static item that implements the GlobalAlloc trait.
```

O erro ocorre porque a crate `alloc` requer um alocador heap, que é um objeto que fornece as funções `allocate` e `deallocate`. Em Rust, alocadores heap são descritos pela trait [`GlobalAlloc`], que é mencionada na mensagem de erro. Para definir o alocador heap para a crate, o atributo `#[global_allocator]` deve ser aplicado a uma variável `static` que implementa a trait `GlobalAlloc`.

[`GlobalAlloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html

### A Trait `GlobalAlloc`

A trait [`GlobalAlloc`] define as funções que um alocador heap deve fornecer. A trait é especial porque quase nunca é usada diretamente pelo programador. Em vez disso, o compilador irá automaticamente inserir as chamadas apropriadas aos métodos da trait ao usar os tipos de alocação e coleção de `alloc`.

Como precisaremos implementar a trait para todos os nossos tipos de alocador, vale a pena dar uma olhada mais de perto em sua declaração:

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

Ela define os dois métodos obrigatórios [`alloc`] e [`dealloc`], que correspondem às funções `allocate` e `deallocate` que usamos em nossos exemplos:
- O método [`alloc`] recebe uma instância [`Layout`] como argumento, que descreve o tamanho e alinhamento desejados que a memória alocada deve ter. Ele retorna um [ponteiro bruto] para o primeiro byte do bloco de memória alocado. Em vez de um valor de erro explícito, o método `alloc` retorna um ponteiro nulo para sinalizar um erro de alocação. Isso é um pouco não idiomático, mas tem a vantagem de que envolver alocadores de sistema existentes é fácil, já que eles usam a mesma convenção.
- O método [`dealloc`] é a contraparte e é responsável por liberar um bloco de memória novamente. Ele recebe dois argumentos: o ponteiro retornado por `alloc` e o `Layout` que foi usado para a alocação.

[`alloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.alloc
[`dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc
[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html

A trait adicionalmente define os dois métodos [`alloc_zeroed`] e [`realloc`] com implementações padrão:

- O método [`alloc_zeroed`] é equivalente a chamar `alloc` e então definir o bloco de memória alocado para zero, que é exatamente o que a implementação padrão fornecida faz. Uma implementação de alocador pode substituir as implementações padrão com uma implementação personalizada mais eficiente se possível.
- O método [`realloc`] permite aumentar ou diminuir uma alocação. A implementação padrão aloca um novo bloco de memória com o tamanho desejado e copia todo o conteúdo da alocação anterior. Novamente, uma implementação de alocador pode provavelmente fornecer uma implementação mais eficiente deste método, por exemplo, aumentando/diminuindo a alocação no lugar, se possível.

[`alloc_zeroed`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#method.alloc_zeroed
[`realloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#method.realloc

#### Insegurança

Uma coisa a notar é que tanto a trait em si quanto todos os métodos da trait são declarados como `unsafe`:

- A razão para declarar a trait como `unsafe` é que o programador deve garantir que a implementação da trait para um tipo de alocador esteja correta. Por exemplo, o método `alloc` nunca deve retornar um bloco de memória que já está sendo usado em outro lugar porque isso causaria comportamento indefinido.
- Similarmente, a razão pela qual os métodos são `unsafe` é que o chamador deve garantir várias invariantes ao chamar os métodos, por exemplo, que o `Layout` passado para `alloc` especifica um tamanho diferente de zero. Isso não é realmente relevante na prática, já que os métodos normalmente são chamados diretamente pelo compilador, que garante que os requisitos sejam atendidos.

### Um `DummyAllocator`

Agora que sabemos o que um tipo de alocador deve fornecer, podemos criar um alocador dummy simples. Para isso, criamos um novo módulo `allocator`:

```rust
// em src/lib.rs

pub mod allocator;
```

Nosso alocador dummy faz o mínimo absoluto para implementar a trait e sempre retorna um erro quando `alloc` é chamado. Ele se parece com isso:

```rust
// em src/allocator.rs

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

A struct não precisa de nenhum campo, então a criamos como um [tipo de tamanho zero]. Como mencionado acima, sempre retornamos o ponteiro nulo de `alloc`, que corresponde a um erro de alocação. Como o alocador nunca retorna nenhuma memória, uma chamada para `dealloc` nunca deve ocorrer. Por essa razão, simplesmente entramos em pânico no método `dealloc`. Os métodos `alloc_zeroed` e `realloc` têm implementações padrão, então não precisamos fornecer implementações para eles.

[tipo de tamanho zero]: https://doc.rust-lang.org/nomicon/exotic-sizes.html#zero-sized-types-zsts

Agora temos um alocador simples, mas ainda temos que dizer ao compilador Rust que ele deve usar este alocador. É aqui que o atributo `#[global_allocator]` entra.

### O Atributo `#[global_allocator]`

O atributo `#[global_allocator]` diz ao compilador Rust qual instância de alocador ele deve usar como alocador heap global. O atributo só é aplicável a um `static` que implementa a trait `GlobalAlloc`. Vamos registrar uma instância de nosso alocador `Dummy` como o alocador global:

```rust
// em src/allocator.rs

#[global_allocator]
static ALLOCATOR: Dummy = Dummy;
```

Como o alocador `Dummy` é um [tipo de tamanho zero], não precisamos especificar nenhum campo na expressão de inicialização.

Com este static, os erros de compilação devem ser corrigidos. Agora podemos usar os tipos de alocação e coleção de `alloc`. Por exemplo, podemos usar um [`Box`] para alocar um valor no heap:

[`Box`]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html

```rust
// em src/main.rs

extern crate alloc;

use alloc::boxed::Box;

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] imprimir "Hello World!", chamar `init`, criar `mapper` e `frame_allocator`

    let x = Box::new(41);

    // […] chamar `test_main` no modo de teste

    println!("It did not crash!");
    blog_os::hlt_loop();
}

```

Note que precisamos especificar a declaração `extern crate alloc` em nosso `main.rs` também. Isso é necessário porque as partes `lib.rs` e `main.rs` são tratadas como crates separadas. No entanto, não precisamos criar outro `#[global_allocator]` static porque o alocador global se aplica a todas as crates do projeto. Na verdade, especificar um alocador adicional em outra crate seria um erro.

Quando executamos o código acima, vemos que um pânico ocorre:

![QEMU imprimindo "panicked at `allocation error: Layout { size_: 4, align_: 4 }, src/lib.rs:89:5"](qemu-dummy-output.png)

O pânico ocorre porque a função `Box::new` chama implicitamente a função `alloc` do alocador global. Nosso alocador dummy sempre retorna um ponteiro nulo, então toda alocação falha. Para corrigir isso, precisamos criar um alocador que realmente retorna memória utilizável.

## Criando um Heap do Kernel

Antes de podermos criar um alocador apropriado, primeiro precisamos criar uma região de memória heap da qual o alocador pode alocar memória. Para fazer isso, precisamos definir um intervalo de memória virtual para a região heap e então mapear esta região para frames físicos. Veja o post [_"Introdução ao Paging"_] para uma visão geral de memória virtual e tabelas de página.

[_"Introdução ao Paging"_]: @/edition-2/posts/08-paging-introduction/index.md

O primeiro passo é definir uma região de memória virtual para o heap. Podemos escolher qualquer intervalo de endereço virtual que quisermos, desde que não esteja já sendo usado para uma região de memória diferente. Vamos defini-la como a memória começando no endereço `0x_4444_4444_0000` para que possamos facilmente reconhecer um ponteiro heap mais tarde:

```rust
// em src/allocator.rs

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB
```

Definimos o tamanho do heap para 100&nbsp;KiB por enquanto. Se precisarmos de mais espaço no futuro, podemos simplesmente aumentá-lo.

Se tentássemos usar esta região heap agora, uma falha de página ocorreria, já que a região de memória virtual ainda não está mapeada para memória física. Para resolver isso, criamos uma função `init_heap` que mapeia as páginas heap usando a [API `Mapper`] que introduzimos no post [_"Implementação de Paging"_]:

[API `Mapper`]: @/edition-2/posts/09-paging-implementation/index.md#using-offsetpagetable
[_"Implementação de Paging"_]: @/edition-2/posts/09-paging-implementation/index.md

```rust
// em src/allocator.rs

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

A função recebe referências mutáveis para uma instância [`Mapper`] e uma instância [`FrameAllocator`], ambas limitadas a páginas de 4&nbsp;KiB usando [`Size4KiB`] como o parâmetro genérico. O valor de retorno da função é um [`Result`] com o tipo unitário `()` como a variante de sucesso e um [`MapToError`] como a variante de erro, que é o tipo de erro retornado pelo método [`Mapper::map_to`]. Reutilizar o tipo de erro faz sentido aqui porque o método `map_to` é a principal fonte de erros nesta função.

[`Mapper`]:https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html
[`FrameAllocator`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.FrameAllocator.html
[`Size4KiB`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/enum.Size4KiB.html
[`Result`]: https://doc.rust-lang.org/core/result/enum.Result.html
[`MapToError`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/enum.MapToError.html
[`Mapper::map_to`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html#method.map_to

A implementação pode ser dividida em duas partes:

- **Criando o intervalo de páginas:**: Para criar um intervalo das páginas que queremos mapear, convertemos o ponteiro `HEAP_START` para um tipo [`VirtAddr`]. Então calculamos o endereço final do heap a partir dele adicionando o `HEAP_SIZE`. Queremos um limite inclusivo (o endereço do último byte do heap), então subtraímos 1. Em seguida, convertemos os endereços em tipos [`Page`] usando a função [`containing_address`]. Finalmente, criamos um intervalo de páginas das páginas inicial e final usando a função [`Page::range_inclusive`].

- **Mapeando as páginas:** O segundo passo é mapear todas as páginas do intervalo de páginas que acabamos de criar. Para isso, iteramos sobre essas páginas usando um loop `for`. Para cada página, fazemos o seguinte:

    - Alocamos um frame físico para o qual a página deve ser mapeada usando o método [`FrameAllocator::allocate_frame`]. Este método retorna [`None`] quando não há mais frames disponíveis. Lidamos com esse caso mapeando-o para um erro [`MapToError::FrameAllocationFailed`] através do método [`Option::ok_or`] e então aplicando o [operador de ponto de interrogação] para retornar cedo em caso de erro.

    - Definimos a flag `PRESENT` obrigatória e a flag `WRITABLE` para a página. Com essas flags, tanto acessos de leitura quanto de escrita são permitidos, o que faz sentido para memória heap.

    - Usamos o método [`Mapper::map_to`] para criar o mapeamento na tabela de páginas ativa. O método pode falhar, então usamos o [operador de ponto de interrogação] novamente para encaminhar o erro ao chamador. Em caso de sucesso, o método retorna uma instância [`MapperFlush`] que podemos usar para atualizar o [_buffer de tradução lookaside_] usando o método [`flush`].

[`VirtAddr`]: https://docs.rs/x86_64/0.14.2/x86_64/addr/struct.VirtAddr.html
[`Page`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/struct.Page.html
[`containing_address`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/struct.Page.html#method.containing_address
[`Page::range_inclusive`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/struct.Page.html#method.range_inclusive
[`FrameAllocator::allocate_frame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.FrameAllocator.html#tymethod.allocate_frame
[`None`]: https://doc.rust-lang.org/core/option/enum.Option.html#variant.None
[`MapToError::FrameAllocationFailed`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/enum.MapToError.html#variant.FrameAllocationFailed
[`Option::ok_or`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.ok_or
[operador de ponto de interrogação]: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html
[`MapperFlush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html
[_buffer de tradução lookaside_]: @/edition-2/posts/08-paging-introduction/index.md#the-translation-lookaside-buffer
[`flush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html#method.flush

O passo final é chamar esta função de nossa `kernel_main`:

```rust
// em src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::allocator; // nova importação
    use blog_os::memory::{self, BootInfoFrameAllocator};

    println!("Hello World{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    // novo
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    let x = Box::new(41);

    // […] chamar `test_main` no modo de teste

    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

Mostramos a função completa para contexto aqui. As únicas linhas novas são a importação `blog_os::allocator` e a chamada para a função `allocator::init_heap`. No caso de a função `init_heap` retornar um erro, entramos em pânico usando o método [`Result::expect`], já que atualmente não há maneira sensata de lidarmos com este erro.

[`Result::expect`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.expect

Agora temos uma região de memória heap mapeada que está pronta para ser usada. A chamada `Box::new` ainda usa nosso alocador `Dummy` antigo, então você ainda verá o erro "out of memory" quando executá-lo. Vamos corrigir isso usando um alocador apropriado.

## Usando uma Crate de Alocador

Como implementar um alocador é um tanto complexo, começamos usando uma crate de alocador externa. Aprenderemos como implementar nosso próprio alocador no próximo post.

Uma crate de alocador simples para aplicações `no_std` é a crate [`linked_list_allocator`]. Seu nome vem do fato de que ela usa uma estrutura de dados de lista encadeada para acompanhar as regiões de memória desalocadas. Veja o próximo post para uma explicação mais detalhada dessa abordagem.

Para usar a crate, primeiro precisamos adicionar uma dependência nela em nosso `Cargo.toml`:

[`linked_list_allocator`]: https://github.com/phil-opp/linked-list-allocator/

```toml
# em Cargo.toml

[dependencies]
linked_list_allocator = "0.9.0"
```

Então podemos substituir nosso alocador dummy pelo alocador fornecido pela crate:

```rust
// em src/allocator.rs

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();
```

A struct é chamada `LockedHeap` porque usa o tipo [`spinning_top::Spinlock`] para sincronização. Isso é necessário porque múltiplas threads podem acessar o static `ALLOCATOR` ao mesmo tempo. Como sempre, ao usar um spinlock ou um mutex, precisamos ter cuidado para não causar acidentalmente um deadlock. Isso significa que não devemos realizar nenhuma alocação em manipuladores de interrupção, já que eles podem executar em um momento arbitrário e podem interromper uma alocação em andamento.

[`spinning_top::Spinlock`]: https://docs.rs/spinning_top/0.1.0/spinning_top/type.Spinlock.html

Definir o `LockedHeap` como alocador global não é suficiente. A razão é que usamos a função construtora [`empty`], que cria um alocador sem nenhuma memória de suporte. Como nosso alocador dummy, ele sempre retorna um erro em `alloc`. Para corrigir isso, precisamos inicializar o alocador após criar o heap:

[`empty`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.LockedHeap.html#method.empty

```rust
// em src/allocator.rs

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // […] mapear todas as páginas heap para frames físicos

    // novo
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
```

Usamos o método [`lock`] no spinlock interno do tipo `LockedHeap` para obter uma referência exclusiva à instância [`Heap`] encapsulada, na qual então chamamos o método [`init`] com os limites do heap como argumentos. Como a função [`init`] já tenta escrever na memória heap, devemos inicializar o heap somente _depois_ de mapear as páginas heap.

[`lock`]: https://docs.rs/lock_api/0.3.3/lock_api/struct.Mutex.html#method.lock
[`Heap`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html
[`init`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.init

Depois de inicializar o heap, agora podemos usar todos os tipos de alocação e coleção da crate embutida [`alloc`] sem erro:

```rust
// em src/main.rs

use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] inicializar interrupções, mapper, frame_allocator, heap

    // alocar um número no heap
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    // criar um vetor de tamanho dinâmico
    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    // criar um vetor com contagem de referências -> será liberado quando a contagem chegar a 0
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!("current reference count is {}", Rc::strong_count(&cloned_reference));
    core::mem::drop(reference_counted);
    println!("reference count is {} now", Rc::strong_count(&cloned_reference));

    // […] chamar `test_main` no contexto de teste
    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

Este exemplo de código mostra alguns usos dos tipos [`Box`], [`Vec`] e [`Rc`]. Para os tipos `Box` e `Vec`, imprimimos os ponteiros heap subjacentes usando o [especificador de formatação `{:p}`]. Para mostrar `Rc`, criamos um valor heap com contagem de referências e usamos a função [`Rc::strong_count`] para imprimir a contagem de referências atual antes e depois de descartar uma instância (usando [`core::mem::drop`]).

[`Vec`]: https://doc.rust-lang.org/alloc/vec/
[`Rc`]: https://doc.rust-lang.org/alloc/rc/
[especificador de formatação `{:p}`]: https://doc.rust-lang.org/core/fmt/trait.Pointer.html
[`Rc::strong_count`]: https://doc.rust-lang.org/alloc/rc/struct.Rc.html#method.strong_count
[`core::mem::drop`]: https://doc.rust-lang.org/core/mem/fn.drop.html

Quando o executamos, vemos o seguinte:

![QEMU imprimindo `
heap_value at 0x444444440000
vec at 0x4444444408000
current reference count is 2
reference count is 1 now
](qemu-alloc-showcase.png)

Como esperado, vemos que os valores `Box` e `Vec` vivem no heap, como indicado pelo ponteiro começando com o prefixo `0x_4444_4444_*`. O valor com contagem de referências também se comporta como esperado, com a contagem de referências sendo 2 após a chamada `clone`, e 1 novamente depois que uma das instâncias foi descartada.

A razão pela qual o vetor começa no offset `0x800` não é que o valor encaixotado seja `0x800` bytes grande, mas as [realocações] que ocorrem quando o vetor precisa aumentar sua capacidade. Por exemplo, quando a capacidade do vetor é 32 e tentamos adicionar o próximo elemento, o vetor aloca um novo array de suporte com capacidade de 64 nos bastidores e copia todos os elementos. Então ele libera a alocação antiga.

[realocações]: https://doc.rust-lang.org/alloc/vec/struct.Vec.html#capacity-and-reallocation

É claro que existem muitos mais tipos de alocação e coleção na crate `alloc` que agora podemos usar todos em nosso kernel, incluindo:

- o ponteiro com contagem de referências thread-safe [`Arc`]
- o tipo de string proprietária [`String`] e a macro [`format!`]
- [`LinkedList`]
- o buffer circular crescente [`VecDeque`]
- a fila de prioridade [`BinaryHeap`]
- [`BTreeMap`] e [`BTreeSet`]

[`Arc`]: https://doc.rust-lang.org/alloc/sync/struct.Arc.html
[`String`]: https://doc.rust-lang.org/alloc/string/struct.String.html
[`format!`]: https://doc.rust-lang.org/alloc/macro.format.html
[`LinkedList`]: https://doc.rust-lang.org/alloc/collections/linked_list/struct.LinkedList.html
[`VecDeque`]: https://doc.rust-lang.org/alloc/collections/vec_deque/struct.VecDeque.html
[`BinaryHeap`]: https://doc.rust-lang.org/alloc/collections/binary_heap/struct.BinaryHeap.html
[`BTreeMap`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html
[`BTreeSet`]: https://doc.rust-lang.org/alloc/collections/btree_set/struct.BTreeSet.html

Esses tipos se tornarão muito úteis quando quisermos implementar listas de threads, filas de escalonamento ou suporte para async/await.

## Adicionando um Teste

Para garantir que não quebremos acidentalmente nosso novo código de alocação, devemos adicionar um teste de integração para ele. Começamos criando um novo arquivo `tests/heap_allocation.rs` com o seguinte conteúdo:

```rust
// em tests/heap_allocation.rs

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

Reutilizamos as funções `test_runner` e `test_panic_handler` de nosso `lib.rs`. Como queremos testar alocações, habilitamos a crate `alloc` através da declaração `extern crate alloc`. Para mais informações sobre o boilerplate de teste, confira o post [_Testing_].

[_Testing_]: @/edition-2/posts/04-testing/index.md

A implementação da função `main` se parece com isso:

```rust
// em tests/heap_allocation.rs

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

Ela é muito similar à função `kernel_main` em nosso `main.rs`, com as diferenças de que não invocamos `println`, não incluímos nenhuma alocação de exemplo, e chamamos `test_main` incondicionalmente.

Agora estamos prontos para adicionar alguns casos de teste. Primeiro, adicionamos um teste que realiza algumas alocações simples usando [`Box`] e verifica os valores alocados para garantir que as alocações básicas funcionam:

```rust
// em tests/heap_allocation.rs
use alloc::boxed::Box;

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}
```

Mais importante, este teste verifica que nenhum erro de alocação ocorre.

Em seguida, construímos iterativamente um vetor grande, para testar tanto alocações grandes quanto múltiplas alocações (devido a realocações):

```rust
// em tests/heap_allocation.rs

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

Verificamos a soma comparando-a com a fórmula para a [soma parcial n-ésima]. Isso nos dá alguma confiança de que os valores alocados estão todos corretos.

[soma parcial n-ésima]: https://en.wikipedia.org/wiki/1_%2B_2_%2B_3_%2B_4_%2B_%E2%8B%AF#Partial_sums

Como terceiro teste, criamos dez mil alocações uma após a outra:

```rust
// em tests/heap_allocation.rs

use blog_os::allocator::HEAP_SIZE;

#[test_case]
fn many_boxes() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}
```

Este teste garante que o alocador reutiliza memória liberada para alocações subsequentes, já que ficaria sem memória caso contrário. Isso pode parecer um requisito óbvio para um alocador, mas existem designs de alocador que não fazem isso. Um exemplo é o design de alocador bump que será explicado no próximo post.

Vamos executar nosso novo teste de integração:

```
> cargo test --test heap_allocation
[…]
Running 3 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
```

Todos os três testes foram bem-sucedidos! Você também pode invocar `cargo test` (sem o argumento `--test`) para executar todos os testes unitários e de integração.

## Resumo

Este post deu uma introdução à memória dinâmica e explicou por que e onde ela é necessária. Vimos como o verificador de empréstimos do Rust previne vulnerabilidades comuns e aprendemos como a API de alocação do Rust funciona.

Depois de criar uma implementação mínima da interface de alocador do Rust usando um alocador dummy, criamos uma região de memória heap apropriada para o nosso kernel. Para isso, definimos um intervalo de endereço virtual para o heap e então mapeamos todas as páginas desse intervalo para frames físicos usando o `Mapper` e `FrameAllocator` do post anterior.

Finalmente, adicionamos uma dependência na crate `linked_list_allocator` para adicionar um alocador apropriado ao nosso kernel. Com este alocador, pudemos usar `Box`, `Vec` e outros tipos de alocação e coleção da crate `alloc`.

## O que vem a seguir?

Embora já tenhamos adicionado suporte para alocação heap neste post, deixamos a maior parte do trabalho para a crate `linked_list_allocator`. O próximo post mostrará em detalhes como um alocador pode ser implementado do zero. Ele apresentará múltiplos designs de alocador possíveis, mostrará como implementar versões simples deles e explicará suas vantagens e desvantagens.
