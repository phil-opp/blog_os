+++
title = "Designs de Alocadores"
weight = 11
path = "pt-BR/allocator-designs"
date = 2020-01-20

[extra]
chapter = "Gerenciamento de Memória"
# Please update this when updating the translation
translation_based_on_commit = "c0fc0bed9e8b8459dde80a71f4f89f578cb5ddfb"
# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

Este post explica como implementar alocadores heap do zero. Ele apresenta e discute diferentes designs de alocadores, incluindo alocação bump, alocação de lista encadeada e alocação de bloco de tamanho fixo. Para cada um dos três designs, criaremos uma implementação básica que pode ser usada para o nosso kernel.

<!-- more -->

Este blog é desenvolvido abertamente no [GitHub]. Se você tiver algum problema ou pergunta, por favor abra uma issue lá. Você também pode deixar comentários [no final]. O código-fonte completo para este post pode ser encontrado no branch [`post-11`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-11

<!-- toc -->

## Introdução

No [post anterior], adicionamos suporte básico para alocações heap ao nosso kernel. Para isso, [criamos uma nova região de memória][map-heap] nas tabelas de página e [usamos a crate `linked_list_allocator`][use-alloc-crate] para gerenciar essa memória. Embora agora tenhamos um heap funcional, deixamos a maior parte do trabalho para a crate do alocador sem tentar entender como ela funciona.

[post anterior]: @/edition-2/posts/10-heap-allocation/index.md
[map-heap]: @/edition-2/posts/10-heap-allocation/index.md#creating-a-kernel-heap
[use-alloc-crate]: @/edition-2/posts/10-heap-allocation/index.md#using-an-allocator-crate

Neste post, mostraremos como criar nosso próprio alocador heap do zero em vez de depender de uma crate de alocador existente. Discutiremos diferentes designs de alocadores, incluindo um _alocador bump_ simplista e um _alocador de bloco de tamanho fixo_ básico, e usaremos esse conhecimento para implementar um alocador com desempenho aprimorado (comparado à crate `linked_list_allocator`).

### Objetivos de Design

A responsabilidade de um alocador é gerenciar a memória heap disponível. Ele precisa retornar memória não utilizada em chamadas `alloc` e acompanhar a memória liberada por `dealloc` para que possa ser reutilizada novamente. Mais importante, ele nunca deve entregar memória que já está em uso em outro lugar porque isso causaria comportamento indefinido.

Além da correção, existem muitos objetivos de design secundários. Por exemplo, o alocador deve utilizar efetivamente a memória disponível e manter a [_fragmentação_] baixa. Além disso, ele deve funcionar bem para aplicações concorrentes e escalar para qualquer número de processadores. Para desempenho máximo, ele poderia até otimizar o layout da memória em relação aos caches da CPU para melhorar a [localidade de cache] e evitar [compartilhamento falso].

[localidade de cache]: https://www.geeksforgeeks.org/locality-of-reference-and-cache-operation-in-cache-memory/
[_fragmentação_]: https://en.wikipedia.org/wiki/Fragmentation_(computing)
[compartilhamento falso]: https://mechanical-sympathy.blogspot.de/2011/07/false-sharing.html

Esses requisitos podem tornar bons alocadores muito complexos. Por exemplo, [jemalloc] tem mais de 30.000 linhas de código. Essa complexidade é frequentemente indesejada no código do kernel, onde um único bug pode levar a vulnerabilidades de segurança graves. Felizmente, os padrões de alocação do código do kernel são frequentemente muito mais simples comparados ao código do espaço do usuário, de modo que designs de alocadores relativamente simples frequentemente são suficientes.

[jemalloc]: http://jemalloc.net/

A seguir, apresentamos três possíveis designs de alocadores de kernel e explicamos suas vantagens e desvantagens.

## Alocador Bump

O design de alocador mais simples é um _alocador bump_ (também conhecido como _alocador de pilha_). Ele aloca memória linearmente e só mantém o controle do número de bytes alocados e do número de alocações. Ele só é útil em casos de uso muito específicos porque tem uma limitação severa: ele só pode liberar toda a memória de uma vez.

### Ideia

A ideia por trás de um alocador bump é alocar memória linearmente aumentando (_"bumping"_) uma variável `next`, que aponta para o início da memória não utilizada. No início, `next` é igual ao endereço inicial do heap. Em cada alocação, `next` é aumentado pelo tamanho da alocação para que sempre aponte para a fronteira entre memória usada e não utilizada:

![A área de memória heap em três pontos no tempo:
 1: Uma única alocação existe no início do heap; o ponteiro `next` aponta para seu final.
 2: Uma segunda alocação foi adicionada logo após a primeira; o ponteiro `next` aponta para o final da segunda alocação.
 3: Uma terceira alocação foi adicionada logo após a segunda; o ponteiro `next` aponta para o final da terceira alocação.](bump-allocation.svg)

O ponteiro `next` só se move em uma única direção e, portanto, nunca entrega a mesma região de memória duas vezes. Quando ele alcança o final do heap, nenhuma memória adicional pode ser alocada, resultando em um erro de falta de memória na próxima alocação.

Um alocador bump é frequentemente implementado com um contador de alocações, que é aumentado em 1 em cada chamada `alloc` e diminuído em 1 em cada chamada `dealloc`. Quando o contador de alocações atinge zero, significa que todas as alocações no heap foram desalocadas. Nesse caso, o ponteiro `next` pode ser redefinido para o endereço inicial do heap, de modo que a memória heap completa esteja disponível para alocações novamente.

### Implementação

Começamos nossa implementação declarando um novo submódulo `allocator::bump`:

```rust
// em src/allocator.rs

pub mod bump;
```

O conteúdo do submódulo vive em um novo arquivo `src/allocator/bump.rs`, que criamos com o seguinte conteúdo:

```rust
// em src/allocator/bump.rs

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    /// Cria um novo alocador bump vazio.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    /// Inicializa o alocador bump com os limites de heap fornecidos.
    ///
    /// Este método é unsafe porque o chamador deve garantir que o intervalo
    /// de memória fornecido esteja não utilizado. Além disso, este método deve ser chamado apenas uma vez.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}
```

Os campos `heap_start` e `heap_end` mantêm o controle dos limites inferior e superior da região de memória heap. O chamador precisa garantir que esses endereços sejam válidos, caso contrário o alocador retornaria memória inválida. Por essa razão, a função `init` precisa ser `unsafe` para chamar.

O propósito do campo `next` é sempre apontar para o primeiro byte não utilizado do heap, ou seja, o endereço inicial da próxima alocação. Ele é definido como `heap_start` na função `init` porque no início, o heap inteiro está não utilizado. Em cada alocação, este campo será aumentado pelo tamanho da alocação (_"bumped"_) para garantir que não retornemos a mesma região de memória duas vezes.

O campo `allocations` é um simples contador para as alocações ativas com o objetivo de redefinir o alocador após a última alocação ter sido liberada. Ele é inicializado com 0.

Escolhemos criar uma função `init` separada em vez de realizar a inicialização diretamente em `new` para manter a interface idêntica ao alocador fornecido pela crate `linked_list_allocator`. Dessa forma, os alocadores podem ser trocados sem mudanças adicionais no código.

### Implementando `GlobalAlloc`

Como [explicado no post anterior][global-alloc], todos os alocadores heap precisam implementar a trait [`GlobalAlloc`], que é definida assim:

[global-alloc]: @/edition-2/posts/10-heap-allocation/index.md#the-allocator-interface
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

Apenas os métodos `alloc` e `dealloc` são obrigatórios; os outros dois métodos têm implementações padrão e podem ser omitidos.

#### Primeira Tentativa de Implementação

Vamos tentar implementar o método `alloc` para nosso `BumpAllocator`:

```rust
// em src/allocator/bump.rs

use alloc::alloc::{GlobalAlloc, Layout};

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // TODO verificação de alinhamento e limites
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

Primeiro, usamos o campo `next` como o endereço inicial para nossa alocação. Então atualizamos o campo `next` para apontar para o endereço final da alocação, que é o próximo endereço não utilizado no heap. Antes de retornar o endereço inicial da alocação como um ponteiro `*mut u8`, aumentamos o contador `allocations` em 1.

Note que não realizamos nenhuma verificação de limites ou ajustes de alinhamento, então esta implementação ainda não é segura. Isso não importa muito porque ela falha ao compilar de qualquer forma com o seguinte erro:

```
error[E0594]: cannot assign to `self.next` which is behind a `&` reference
  --> src/allocator/bump.rs:29:9
   |
29 |         self.next = alloc_start + layout.size();
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `self` is a `&` reference, so the data it refers to cannot be written
```

(O mesmo erro também ocorre para a linha `self.allocations += 1`. Omitimos aqui por brevidade.)

O erro ocorre porque os métodos [`alloc`] e [`dealloc`] da trait `GlobalAlloc` operam apenas em uma referência imutável `&self`, então atualizar os campos `next` e `allocations` não é possível. Isso é problemático porque atualizar `next` em cada alocação é o princípio essencial de um alocador bump.

[`alloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.alloc
[`dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc

#### `GlobalAlloc` e Mutabilidade

Antes de olharmos para uma possível solução para este problema de mutabilidade, vamos tentar entender por que os métodos da trait `GlobalAlloc` são definidos com argumentos `&self`: Como vimos [no post anterior][global-allocator], o alocador heap global é definido adicionando o atributo `#[global_allocator]` a um `static` que implementa a trait `GlobalAlloc`. Variáveis estáticas são imutáveis em Rust, então não há maneira de chamar um método que recebe `&mut self` no alocador estático. Por essa razão, todos os métodos de `GlobalAlloc` recebem apenas uma referência imutável `&self`.

[global-allocator]:  @/edition-2/posts/10-heap-allocation/index.md#the-global-allocator-attribute

Felizmente, há uma maneira de obter uma referência `&mut self` de uma referência `&self`: Podemos usar [mutabilidade interior] sincronizada envolvendo o alocador em um spinlock [`spin::Mutex`]. Este tipo fornece um método `lock` que realiza [exclusão mútua] e, portanto, transforma com segurança uma referência `&self` em uma referência `&mut self`. Já usamos o tipo wrapper várias vezes em nosso kernel, por exemplo, para o [buffer de texto VGA][vga-mutex].

[mutabilidade interior]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
[vga-mutex]: @/edition-2/posts/03-vga-text-buffer/index.md#spinlocks
[`spin::Mutex`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html
[exclusão mútua]: https://en.wikipedia.org/wiki/Mutual_exclusion

#### Um Tipo Wrapper `Locked`

Com a ajuda do tipo wrapper `spin::Mutex`, podemos implementar a trait `GlobalAlloc` para nosso alocador bump. O truque é implementar a trait não para o `BumpAllocator` diretamente, mas para o tipo envolvido `spin::Mutex<BumpAllocator>`:

```rust
unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {…}
```

Infelizmente, isso ainda não funciona porque o compilador Rust não permite implementações de traits para tipos definidos em outras crates:

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

Para corrigir isso, precisamos criar nosso próprio tipo wrapper em torno de `spin::Mutex`:

```rust
// em src/allocator.rs

/// Um wrapper em torno de spin::Mutex para permitir implementações de traits.
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

O tipo é um wrapper genérico em torno de um `spin::Mutex<A>`. Ele não impõe restrições no tipo envolvido `A`, então pode ser usado para envolver todos os tipos, não apenas alocadores. Ele fornece uma simples função construtora `new` que envolve um valor dado. Para conveniência, ele também fornece uma função `lock` que chama `lock` no `Mutex` envolvido. Como o tipo `Locked` é geral o suficiente para ser útil para outras implementações de alocadores também, o colocamos no módulo `allocator` pai.

#### Implementação para `Locked<BumpAllocator>`

O tipo `Locked` é definido em nossa própria crate (em contraste com `spin::Mutex`), então podemos usá-lo para implementar `GlobalAlloc` para nosso alocador bump. A implementação completa se parece com isso:

```rust
// em src/allocator/bump.rs

use super::{align_up, Locked};
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock(); // obter uma referência mutável

        let alloc_start = align_up(bump.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            ptr::null_mut() // fora de memória
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock(); // obter uma referência mutável

        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}
```

O primeiro passo para tanto `alloc` quanto `dealloc` é chamar o método [`Mutex::lock`] através do campo `inner` para obter uma referência mutável ao tipo alocador envolvido. A instância permanece bloqueada até o final do método, para que nenhuma corrida de dados possa ocorrer em contextos multi-thread (adicionaremos suporte a threading em breve).

[`Mutex::lock`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html#method.lock

Comparado ao protótipo anterior, a implementação de `alloc` agora respeita requisitos de alinhamento e realiza uma verificação de limites para garantir que as alocações permaneçam dentro da região de memória heap. O primeiro passo é arredondar o endereço `next` para cima até o alinhamento especificado pelo argumento `Layout`. O código para a função `align_up` é mostrado em um momento. Então adicionamos o tamanho de alocação solicitado a `alloc_start` para obter o endereço final da alocação. Para prevenir overflow de inteiro em alocações grandes, usamos o método [`checked_add`]. Se ocorrer um overflow ou se o endereço final resultante da alocação for maior que o endereço final do heap, retornamos um ponteiro nulo para sinalizar uma situação de falta de memória. Caso contrário, atualizamos o endereço `next` e aumentamos o contador `allocations` em 1 como antes. Finalmente, retornamos o endereço `alloc_start` convertido para um ponteiro `*mut u8`.

[`checked_add`]: https://doc.rust-lang.org/std/primitive.usize.html#method.checked_add
[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html

A função `dealloc` ignora o ponteiro e os argumentos `Layout` fornecidos. Em vez disso, ela apenas diminui o contador `allocations`. Se o contador atingir `0` novamente, significa que todas as alocações foram liberadas novamente. Nesse caso, ela redefine o endereço `next` para o endereço `heap_start` para tornar a memória heap completa disponível novamente.

#### Alinhamento de Endereço

A função `align_up` é geral o suficiente para que possamos colocá-la no módulo `allocator` pai. Uma implementação básica se parece com isso:

```rust
// em src/allocator.rs

/// Alinha o endereço fornecido `addr` para cima até o alinhamento `align`.
fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr // addr já está alinhado
    } else {
        addr - remainder + align
    }
}
```

A função primeiro calcula o [resto] da divisão de `addr` por `align`. Se o resto for `0`, o endereço já está alinhado com o alinhamento fornecido. Caso contrário, alinhamos o endereço subtraindo o resto (para que o novo resto seja 0) e então adicionando o alinhamento (para que o endereço não se torne menor que o endereço original).

[resto]: https://en.wikipedia.org/wiki/Euclidean_division

Note que esta não é a maneira mais eficiente de implementar esta função. Uma implementação muito mais rápida se parece com isso:

```rust
/// Alinha o endereço fornecido `addr` para cima até o alinhamento `align`.
///
/// Requer que `align` seja uma potência de dois.
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
```

Este método requer que `align` seja uma potência de dois, o que pode ser garantido utilizando a trait `GlobalAlloc` (e seu parâmetro [`Layout`]). Isso torna possível criar uma [máscara de bits] para alinhar o endereço de uma maneira muito eficiente. Para entender como funciona, vamos passar por isso passo a passo, começando no lado direito:

[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html
[máscara de bits]: https://en.wikipedia.org/wiki/Mask_(computing)

- Como `align` é uma potência de dois, sua [representação binária] tem apenas um único bit definido (por exemplo, `0b000100000`). Isso significa que `align - 1` tem todos os bits inferiores definidos (por exemplo, `0b00011111`).
- Ao criar o [`NOT` bit a bit] através do operador `!`, obtemos um número que tem todos os bits definidos exceto os bits inferiores a `align` (por exemplo, `0b…111111111100000`).
- Ao realizar um [`AND` bit a bit] em um endereço e `!(align - 1)`, alinhamos o endereço _para baixo_. Isso funciona limpando todos os bits que são inferiores a `align`.
- Como queremos alinhar para cima em vez de para baixo, aumentamos o `addr` por `align - 1` antes de realizar o `AND` bit a bit. Dessa forma, endereços já alinhados permanecem os mesmos enquanto endereços não alinhados são arredondados para o próximo limite de alinhamento.

[representação binária]: https://en.wikipedia.org/wiki/Binary_number#Representation
[`NOT` bit a bit]: https://en.wikipedia.org/wiki/Bitwise_operation#NOT
[`AND` bit a bit]: https://en.wikipedia.org/wiki/Bitwise_operation#AND

Qual variante você escolher fica a seu critério. Ambas calculam o mesmo resultado, apenas usando métodos diferentes.

### Usando-o

Para usar o alocador bump em vez da crate `linked_list_allocator`, precisamos atualizar o static `ALLOCATOR` em `allocator.rs`:

```rust
// em src/allocator.rs

use bump::BumpAllocator;

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());
```

Aqui se torna importante que declaramos `BumpAllocator::new` e `Locked::new` como [funções `const`]. Se fossem funções normais, ocorreria um erro de compilação porque a expressão de inicialização de um `static` deve ser avaliável em tempo de compilação.

[funções `const`]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

Não precisamos modificar a chamada `ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE)` em nossa função `init_heap` porque o alocador bump fornece a mesma interface que o alocador fornecido pela `linked_list_allocator`.

Agora nosso kernel usa nosso alocador bump! Tudo ainda deve funcionar, incluindo os [testes `heap_allocation`] que criamos no post anterior:

[testes `heap_allocation`]: @/edition-2/posts/10-heap-allocation/index.md#adding-a-test

```
> cargo test --test heap_allocation
[…]
Running 3 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
```

Nosso novo alocador parece funcionar!

### Discussão

A grande vantagem da alocação bump é que ela é muito rápida. Comparado a outros designs de alocadores (veja abaixo) que precisam procurar ativamente por um bloco de memória adequado e realizar várias tarefas de contabilidade em `alloc` e `dealloc`, um alocador bump [pode ser otimizado][bump downwards] para apenas algumas instruções assembly. Isso torna os alocadores bump úteis para otimizar o desempenho de alocação, por exemplo, ao criar uma [biblioteca DOM virtual].

[bump downwards]: https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
[biblioteca DOM virtual]: https://hacks.mozilla.org/2019/03/fast-bump-allocated-virtual-doms-with-rust-and-wasm/

Embora um alocador bump raramente seja usado como o alocador global, o princípio de alocação bump é frequentemente aplicado na forma de [alocação arena], que basicamente agrupa alocações individuais juntas para melhorar o desempenho. Um exemplo de um alocador arena para Rust está contido na crate [`toolshed`].

[alocação arena]: https://mgravell.github.io/Pipelines.Sockets.Unofficial/docs/arenas.html
[`toolshed`]: https://docs.rs/toolshed/0.8.1/toolshed/index.html

#### A Desvantagem de um Alocador Bump

A principal limitação de um alocador bump é que ele só pode reutilizar memória desalocada depois que todas as alocações foram liberadas. Isso significa que uma única alocação de longa duração é suficiente para prevenir a reutilização de memória. Podemos ver isso quando adicionamos uma variação do teste `many_boxes`:

```rust
// em tests/heap_allocation.rs

#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1); // novo
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1); // novo
}
```

Como o teste `many_boxes`, este teste cria um grande número de alocações para provocar uma falha de falta de memória se o alocador não reutilizar memória liberada. Adicionalmente, o teste cria uma alocação `long_lived`, que vive pela execução completa do loop.

Quando tentamos executar nosso novo teste, vemos que ele de fato falha:

```
> cargo test --test heap_allocation
Running 4 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [failed]

Error: panicked at 'allocation error: Layout { size_: 8, align_: 8 }', src/lib.rs:86:5
```

Vamos tentar entender por que essa falha ocorre em detalhe: Primeiro, a alocação `long_lived` é criada no início do heap, aumentando assim o contador `allocations` em 1. Para cada iteração do loop, uma alocação de curta duração é criada e diretamente liberada novamente antes da próxima iteração começar. Isso significa que o contador `allocations` é temporariamente aumentado para 2 no início de uma iteração e diminuído para 1 no final dela. O problema agora é que o alocador bump só pode reutilizar memória depois que _todas_ as alocações foram liberadas, ou seja, quando o contador `allocations` cai para 0. Como isso não acontece antes do final do loop, cada iteração do loop aloca uma nova região de memória, levando a um erro de falta de memória após um número de iterações.

#### Corrigindo o Teste?

Existem dois truques potenciais que poderíamos utilizar para corrigir o teste para nosso alocador bump:

- Poderíamos atualizar `dealloc` para verificar se a alocação liberada foi a última alocação retornada por `alloc` comparando seu endereço final com o ponteiro `next`. No caso de serem iguais, podemos com segurança redefinir `next` de volta ao endereço inicial da alocação liberada. Dessa forma, cada iteração do loop reutiliza o mesmo bloco de memória.
- Poderíamos adicionar um método `alloc_back` que aloca memória do _final_ do heap usando um campo `next_back` adicional. Então poderíamos usar manualmente este método de alocação para todas as alocações de longa duração, separando assim alocações de curta e longa duração no heap. Note que esta separação só funciona se estiver claro de antemão quanto tempo cada alocação viverá. Outra desvantagem desta abordagem é que realizar alocações manualmente é trabalhoso e potencialmente inseguro.

Embora ambas essas abordagens funcionem para corrigir o teste, elas não são uma solução geral, já que são capazes apenas de reutilizar memória em casos muito específicos. A questão é: Existe uma solução geral que reutiliza _toda_ memória liberada?

#### Reutilizando Toda Memória Liberada?

Como aprendemos [no post anterior][heap-intro], alocações podem viver arbitrariamente por muito tempo e podem ser liberadas em uma ordem arbitrária. Isso significa que precisamos acompanhar um número potencialmente ilimitado de regiões de memória não contínuas e não utilizadas, conforme ilustrado pelo seguinte exemplo:

[heap-intro]: @/edition-2/posts/10-heap-allocation/index.md#dynamic-memory

![](allocation-fragmentation.svg)

O gráfico mostra o heap ao longo do tempo. No início, o heap completo está não utilizado, e o endereço `next` é igual a `heap_start` (linha 1). Então a primeira alocação ocorre (linha 2). Na linha 3, um segundo bloco de memória é alocado e a primeira alocação é liberada. Muitas mais alocações são adicionadas na linha 4. Metade delas tem vida muito curta e já são liberadas na linha 5, onde outra nova alocação também é adicionada.

A linha 5 mostra o problema fundamental: Temos cinco regiões de memória não utilizadas com tamanhos diferentes, mas o ponteiro `next` só pode apontar para o início da última região. Embora pudéssemos armazenar os endereços iniciais e tamanhos das outras regiões de memória não utilizadas em um array de tamanho 4 para este exemplo, isso não é uma solução geral, já que poderíamos facilmente criar um exemplo com 8, 16 ou 1000 regiões de memória não utilizadas.

Normalmente, quando temos um número potencialmente ilimitado de itens, podemos simplesmente usar uma coleção alocada no heap. Isso não é realmente possível no nosso caso, já que o alocador heap não pode depender de si mesmo (isso causaria recursão infinita ou deadlocks). Então precisamos encontrar uma solução diferente.

## Alocador de Lista Encadeada

Um truque comum para acompanhar um número arbitrário de áreas de memória livres ao implementar alocadores é usar essas áreas em si como armazenamento de suporte. Isso utiliza o fato de que as regiões ainda estão mapeadas para um endereço virtual e apoiadas por um frame físico, mas a informação armazenada não é mais necessária. Ao armazenar a informação sobre a região liberada na própria região, podemos acompanhar um número ilimitado de regiões liberadas sem precisar de memória adicional.

A abordagem de implementação mais comum é construir uma lista encadeada única na memória liberada, com cada nó sendo uma região de memória liberada:

![](linked-list-allocation.svg)

Cada nó da lista contém dois campos: o tamanho da região de memória e um ponteiro para a próxima região de memória não utilizada. Com esta abordagem, só precisamos de um ponteiro para a primeira região não utilizada (chamada `head`) para acompanhar todas as regiões não utilizadas, independentemente de seu número. A estrutura de dados resultante é frequentemente chamada de [_lista livre_].

[_lista livre_]: https://en.wikipedia.org/wiki/Free_list

Como você pode adivinhar pelo nome, esta é a técnica que a crate `linked_list_allocator` usa. Alocadores que usam esta técnica também são frequentemente chamados de _alocadores de pool_.

### Implementação

A seguir, criaremos nosso próprio tipo simples `LinkedListAllocator` que usa a abordagem acima para acompanhar regiões de memória liberadas. Esta parte do post não é necessária para posts futuros, então você pode pular os detalhes de implementação se quiser.

#### O Tipo Alocador

Começamos criando uma struct privada `ListNode` em um novo submódulo `allocator::linked_list`:

```rust
// em src/allocator.rs

pub mod linked_list;
```

```rust
// em src/allocator/linked_list.rs

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}
```

Como no gráfico, um nó da lista tem um campo `size` e um ponteiro opcional para o próximo nó, representado pelo tipo `Option<&'static mut ListNode>`. O tipo `&'static mut` descreve semanticamente um objeto [possuído] por trás de um ponteiro. Basicamente, é um [`Box`] sem um destruidor que libera o objeto no final do escopo.

[possuído]: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html
[`Box`]: https://doc.rust-lang.org/alloc/boxed/index.html

Implementamos o seguinte conjunto de métodos para `ListNode`:

```rust
// em src/allocator/linked_list.rs

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

O tipo tem uma simples função construtora chamada `new` e métodos para calcular os endereços inicial e final da região representada. Tornamos a função `new` uma [função const], que será necessária mais tarde ao construir um alocador de lista encadeada estático.

[função const]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

Com a struct `ListNode` como um bloco de construção, agora podemos criar a struct `LinkedListAllocator`:

```rust
// em src/allocator/linked_list.rs

pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    /// Cria um LinkedListAllocator vazio.
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// Inicializa o alocador com os limites de heap fornecidos.
    ///
    /// Esta função é unsafe porque o chamador deve garantir que os
    /// limites de heap fornecidos sejam válidos e que o heap esteja não utilizado. Este método deve ser
    /// chamado apenas uma vez.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    /// Adiciona a região de memória fornecida à frente da lista.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        todo!();
    }
}
```

A struct contém um nó `head` que aponta para a primeira região heap. Estamos interessados apenas no valor do ponteiro `next`, então definimos o `size` como 0 na função `ListNode::new`. Tornar `head` um `ListNode` em vez de apenas um `&'static mut ListNode` tem a vantagem de que a implementação do método `alloc` será mais simples.

Como para o alocador bump, a função `new` não inicializa o alocador com os limites do heap. Além de manter compatibilidade com a API, a razão é que a rotina de inicialização requer escrever um nó na memória heap, o que só pode acontecer em tempo de execução. A função `new`, no entanto, precisa ser uma [função `const`] que pode ser avaliada em tempo de compilação porque será usada para inicializar o static `ALLOCATOR`. Por essa razão, fornecemos novamente um método `init` separado e não constante.

[função `const`]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

O método `init` usa um método `add_free_region`, cuja implementação será mostrada em um momento. Por enquanto, usamos a macro [`todo!`] para fornecer uma implementação placeholder que sempre entra em pânico.

[`todo!`]: https://doc.rust-lang.org/core/macro.todo.html

#### O Método `add_free_region`

O método `add_free_region` fornece a operação fundamental de _push_ na lista encadeada. Atualmente só chamamos este método de `init`, mas ele também será o método central em nossa implementação de `dealloc`. Lembre-se, o método `dealloc` é chamado quando uma região de memória alocada é liberada novamente. Para acompanhar esta região de memória liberada, queremos empurrá-la para a lista encadeada.

A implementação do método `add_free_region` se parece com isso:

```rust
// em src/allocator/linked_list.rs

use super::align_up;
use core::mem;

impl LinkedListAllocator {
    /// Adiciona a região de memória fornecida à frente da lista.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // garantir que a região liberada seja capaz de conter ListNode
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // criar um novo nó da lista e anexá-lo no início da lista
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

O método recebe o endereço e tamanho de uma região de memória como argumento e a adiciona à frente da lista. Primeiro, ele garante que a região fornecida tenha o tamanho e alinhamento necessários para armazenar um `ListNode`. Então ele cria o nó e o insere na lista através dos seguintes passos:

![](linked-list-allocator-push.svg)

O passo 0 mostra o estado do heap antes de `add_free_region` ser chamado. No passo 1, o método é chamado com a região de memória marcada como `freed` no gráfico. Após as verificações iniciais, o método cria um novo `node` em sua pilha com o tamanho da região liberada. Então ele usa o método [`Option::take`] para definir o ponteiro `next` do nó para o ponteiro `head` atual, redefinindo assim o ponteiro `head` para `None`.

[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

No passo 2, o método escreve o `node` recém-criado no início da região de memória liberada através do método [`write`]. Então ele aponta o ponteiro `head` para o novo nó. A estrutura de ponteiros resultante parece um pouco caótica porque a região liberada é sempre inserida no início da lista, mas se seguirmos os ponteiros, vemos que cada região livre ainda é alcançável a partir do ponteiro `head`.

[`write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write

#### O Método `find_region`

A segunda operação fundamental em uma lista encadeada é encontrar uma entrada e removê-la da lista. Esta é a operação central necessária para implementar o método `alloc`. Implementamos a operação como um método `find_region` da seguinte maneira:

```rust
// em src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// Procura por uma região livre com o tamanho e alinhamento fornecidos e a remove
    /// da lista.
    ///
    /// Retorna uma tupla do nó da lista e o endereço inicial da alocação.
    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut ListNode, usize)>
    {
        // referência ao nó atual da lista, atualizada para cada iteração
        let mut current = &mut self.head;
        // procurar uma região de memória grande o suficiente na lista encadeada
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // região adequada para alocação -> remover nó da lista
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // região não adequada -> continuar com a próxima região
                current = current.next.as_mut().unwrap();
            }
        }

        // nenhuma região adequada encontrada
        None
    }
}
```

O método usa uma variável `current` e um [loop `while let`] para iterar sobre os elementos da lista. No início, `current` é definido como o nó `head` (dummy). Em cada iteração, ele é então atualizado para o campo `next` do nó atual (no bloco `else`). Se a região for adequada para uma alocação com o tamanho e alinhamento fornecidos, a região é removida da lista e retornada junto com o endereço `alloc_start`.

[loop `while let`]: https://doc.rust-lang.org/reference/expressions/loop-expr.html#predicate-pattern-loops

Quando o ponteiro `current.next` se torna `None`, o loop sai. Isso significa que iteramos sobre toda a lista mas não encontramos nenhuma região adequada para uma alocação. Nesse caso, retornamos `None`. Se uma região é adequada é verificado pela função `alloc_from_region`, cuja implementação será mostrada em um momento.

Vamos dar uma olhada mais detalhada em como uma região adequada é removida da lista:

![](linked-list-allocator-remove-region.svg)

O passo 0 mostra a situação antes de quaisquer ajustes de ponteiros. As regiões `region` e `current` e os ponteiros `region.next` e `current.next` estão marcados no gráfico. No passo 1, tanto o ponteiro `region.next` quanto `current.next` são redefinidos para `None` usando o método [`Option::take`]. Os ponteiros originais são armazenados em variáveis locais chamadas `next` e `ret`.

No passo 2, o ponteiro `current.next` é definido para o ponteiro local `next`, que é o ponteiro original `region.next`. O efeito é que `current` agora aponta diretamente para a região depois de `region`, de modo que `region` não é mais um elemento da lista encadeada. A função então retorna o ponteiro para `region` armazenado na variável local `ret`.

##### A Função `alloc_from_region`

A função `alloc_from_region` retorna se uma região é adequada para uma alocação com um dado tamanho e alinhamento. Ela é definida assim:

```rust
// em src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// Tenta usar a região fornecida para uma alocação com tamanho e
    /// alinhamento dados.
    ///
    /// Retorna o endereço inicial da alocação em caso de sucesso.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
        -> Result<usize, ()>
    {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            // região muito pequena
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            // resto da região muito pequeno para conter um ListNode (necessário porque a
            // alocação divide a região em uma parte usada e uma parte livre)
            return Err(());
        }

        // região adequada para alocação
        Ok(alloc_start)
    }
}
```

Primeiro, a função calcula os endereços inicial e final de uma alocação potencial, usando a função `align_up` que definimos anteriormente e o método [`checked_add`]. Se ocorrer um overflow ou se o endereço final estiver além do endereço final da região, a alocação não cabe na região e retornamos um erro.

A função realiza uma verificação menos óbvia depois disso. Esta verificação é necessária porque na maioria das vezes uma alocação não se encaixa perfeitamente em uma região adequada, de modo que uma parte da região permanece utilizável após a alocação. Esta parte da região deve armazenar seu próprio `ListNode` após a alocação, então deve ser grande o suficiente para fazê-lo. A verificação verifica exatamente isso: ou a alocação se encaixa perfeitamente (`excess_size == 0`) ou o tamanho excedente é grande o suficiente para armazenar um `ListNode`.

#### Implementando `GlobalAlloc`

Com as operações fundamentais fornecidas pelos métodos `add_free_region` e `find_region`, agora podemos finalmente implementar a trait `GlobalAlloc`. Como com o alocador bump, não implementamos a trait diretamente para o `LinkedListAllocator`, mas apenas para um `Locked<LinkedListAllocator>` envolvido. O [wrapper `Locked`] adiciona mutabilidade interior através de um spinlock, que nos permite modificar a instância do alocador mesmo que os métodos `alloc` e `dealloc` recebam apenas referências `&self`.

[wrapper `Locked`]: @/edition-2/posts/11-allocator-designs/index.md#a-locked-wrapper-type

A implementação se parece com isso:

```rust
// em src/allocator/linked_list.rs

use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // realizar ajustes de layout
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
        // realizar ajustes de layout
        let (size, _) = LinkedListAllocator::size_align(layout);

        unsafe { self.lock().add_free_region(ptr as usize, size) }
    }
}
```

Vamos começar com o método `dealloc` porque ele é mais simples: Primeiro, ele realiza alguns ajustes de layout, que explicaremos em um momento. Então, ele recupera uma referência `&mut LinkedListAllocator` chamando a função [`Mutex::lock`] no [wrapper `Locked`]. Por último, ele chama a função `add_free_region` para adicionar a região desalocada à lista livre.

O método `alloc` é um pouco mais complexo. Ele começa com os mesmos ajustes de layout e também chama a função [`Mutex::lock`] para receber uma referência mutável do alocador. Então ele usa o método `find_region` para encontrar uma região de memória adequada para a alocação e removê-la da lista. Se isso não tiver sucesso e `None` for retornado, ele retorna `null_mut` para sinalizar um erro, já que não há nenhuma região de memória adequada.

No caso de sucesso, o método `find_region` retorna uma tupla da região adequada (não mais na lista) e do endereço inicial da alocação. Usando `alloc_start`, o tamanho da alocação e o endereço final da região, ele calcula o endereço final da alocação e o tamanho excedente novamente. Se o tamanho excedente não for nulo, ele chama `add_free_region` para adicionar o tamanho excedente da região de memória de volta à lista livre. Finalmente, ele retorna o endereço `alloc_start` convertido como um ponteiro `*mut u8`.

#### Ajustes de Layout

Então, o que são esses ajustes de layout que fazemos no início de tanto `alloc` quanto `dealloc`? Eles garantem que cada bloco alocado é capaz de armazenar um `ListNode`. Isso é importante porque o bloco de memória vai ser desalocado em algum ponto, onde queremos escrever um `ListNode` nele. Se o bloco for menor que um `ListNode` ou não tiver o alinhamento correto, comportamento indefinido pode ocorrer.

Os ajustes de layout são realizados pela função `size_align`, que é definida assim:

```rust
// em src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// Ajusta o layout fornecido para que a região de memória alocada resultante
    /// também seja capaz de armazenar um `ListNode`.
    ///
    /// Retorna o tamanho e alinhamento ajustados como uma tupla (size, align).
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

Primeiro, a função usa o método [`align_to`] no [`Layout`] passado para aumentar o alinhamento para o alinhamento de um `ListNode` se necessário. Então ela usa o método [`pad_to_align`] para arredondar o tamanho para um múltiplo do alinhamento para garantir que o endereço inicial do próximo bloco de memória também terá o alinhamento correto para armazenar um `ListNode`.
No segundo passo, ela usa o método [`max`] para impor um tamanho mínimo de alocação de `mem::size_of::<ListNode>`. Dessa forma, a função `dealloc` pode com segurança escrever um `ListNode` no bloco de memória liberado.

[`align_to`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align_to
[`pad_to_align`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.pad_to_align
[`max`]: https://doc.rust-lang.org/std/cmp/trait.Ord.html#method.max

### Usando-o

Agora podemos atualizar o static `ALLOCATOR` no módulo `allocator` para usar nosso novo `LinkedListAllocator`:

```rust
// em src/allocator.rs

use linked_list::LinkedListAllocator;

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> =
    Locked::new(LinkedListAllocator::new());
```

Como a função `init` se comporta da mesma forma para os alocadores bump e de lista encadeada, não precisamos modificar a chamada `init` em `init_heap`.

Quando agora executamos nossos testes `heap_allocation` novamente, vemos que todos os testes passam agora, incluindo o teste `many_boxes_long_lived` que falhou com o alocador bump:

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

Isso mostra que nosso alocador de lista encadeada é capaz de reutilizar memória liberada para alocações subsequentes.

### Discussão

Em contraste com o alocador bump, o alocador de lista encadeada é muito mais adequado como um alocador de propósito geral, principalmente porque é capaz de reutilizar diretamente memória liberada. No entanto, ele também tem algumas desvantagens. Algumas delas são causadas apenas pela nossa implementação básica, mas também existem desvantagens fundamentais do próprio design do alocador.

#### Mesclando Blocos Liberados

O principal problema com nossa implementação é que ela apenas divide o heap em blocos menores, mas nunca os mescla de volta juntos. Considere este exemplo:

![](linked-list-allocator-fragmentation-on-dealloc.svg)

Na primeira linha, três alocações são criadas no heap. Duas delas são liberadas novamente na linha 2 e a terceira é liberada na linha 3. Agora o heap completo está não utilizado novamente, mas ainda está dividido em quatro blocos individuais. Neste ponto, uma alocação grande pode não ser mais possível porque nenhum dos quatro blocos é grande o suficiente. Ao longo do tempo, o processo continua, e o heap é dividido em blocos cada vez menores. Em algum ponto, o heap fica tão fragmentado que até alocações de tamanho normal falharão.

Para corrigir este problema, precisamos mesclar blocos adjacentes liberados de volta juntos. Para o exemplo acima, isso significaria o seguinte:

![](linked-list-allocator-merge-on-dealloc.svg)

Como antes, duas das três alocações são liberadas na linha `2`. Em vez de manter o heap fragmentado, agora realizamos um passo adicional na linha `2a` para mesclar os dois blocos mais à direita de volta juntos. Na linha `3`, a terceira alocação é liberada (como antes), resultando em um heap completamente não utilizado representado por três blocos distintos. Em um passo de mesclagem adicional na linha `3a`, então mesclamos os três blocos adjacentes de volta juntos.

A crate `linked_list_allocator` implementa esta estratégia de mesclagem da seguinte maneira: Em vez de inserir blocos de memória liberados no início da lista encadeada em `deallocate`, ela sempre mantém a lista ordenada por endereço inicial. Dessa forma, a mesclagem pode ser realizada diretamente na chamada `deallocate` examinando os endereços e tamanhos dos dois blocos vizinhos na lista. É claro que a operação de desalocação é mais lenta dessa forma, mas previne a fragmentação heap que vimos acima.

#### Desempenho

Como aprendemos acima, o alocador bump é extremamente rápido e pode ser otimizado para apenas algumas operações assembly. O alocador de lista encadeada tem um desempenho muito pior nesta categoria. O problema é que uma requisição de alocação pode precisar percorrer a lista encadeada completa até encontrar um bloco adequado.

Como o comprimento da lista depende do número de blocos de memória não utilizados, o desempenho pode variar extremamente para diferentes programas. Um programa que cria apenas algumas alocações experimentará um desempenho de alocação relativamente rápido. Para um programa que fragmenta o heap com muitas alocações, no entanto, o desempenho de alocação será muito ruim porque a lista encadeada será muito longa e conterá principalmente blocos muito pequenos.

Vale a pena notar que este problema de desempenho não é um problema causado pela nossa implementação básica, mas um problema fundamental da abordagem de lista encadeada. Como o desempenho de alocação pode ser muito importante para código a nível de kernel, exploramos um terceiro design de alocador a seguir que troca utilização de memória melhorada por desempenho reduzido.

## Alocador de Bloco de Tamanho Fixo

A seguir, apresentamos um design de alocador que usa blocos de memória de tamanho fixo para atender requisições de alocação. Dessa forma, o alocador frequentemente retorna blocos que são maiores do que necessário para alocações, o que resulta em memória desperdiçada devido à [fragmentação interna]. Por outro lado, ele reduz drasticamente o tempo necessário para encontrar um bloco adequado (comparado ao alocador de lista encadeada), resultando em muito melhor desempenho de alocação.

### Introdução

A ideia por trás de um _alocador de bloco de tamanho fixo_ é a seguinte: Em vez de alocar exatamente a quantidade de memória solicitada, definimos um pequeno número de tamanhos de bloco e arredondamos cada alocação para cima até o próximo tamanho de bloco. Por exemplo, com tamanhos de bloco de 16, 64 e 512 bytes, uma alocação de 4 bytes retornaria um bloco de 16 bytes, uma alocação de 48 bytes um bloco de 64 bytes, e uma alocação de 128 bytes um bloco de 512 bytes.

Como o alocador de lista encadeada, mantemos o controle da memória não utilizada criando uma lista encadeada na memória não utilizada. No entanto, em vez de usar uma única lista com diferentes tamanhos de bloco, criamos uma lista separada para cada classe de tamanho. Cada lista então armazena apenas blocos de um único tamanho. Por exemplo, com tamanhos de bloco de 16, 64 e 512, haveria três listas encadeadas separadas na memória:

![](fixed-size-block-example.svg).

Em vez de um único ponteiro `head`, temos os três ponteiros head `head_16`, `head_64` e `head_512` que cada um aponta para o primeiro bloco não utilizado do tamanho correspondente. Todos os nós em uma única lista têm o mesmo tamanho. Por exemplo, a lista iniciada pelo ponteiro `head_16` contém apenas blocos de 16 bytes. Isso significa que não precisamos mais armazenar o tamanho em cada nó da lista, já que ele já está especificado pelo nome do ponteiro head.

Como cada elemento em uma lista tem o mesmo tamanho, cada elemento da lista é igualmente adequado para uma requisição de alocação. Isso significa que podemos realizar uma alocação de forma muito eficiente usando os seguintes passos:

- Arredondar o tamanho de alocação solicitado para cima até o próximo tamanho de bloco. Por exemplo, quando uma alocação de 12 bytes é solicitada, escolheríamos o tamanho de bloco de 16 no exemplo acima.
- Recuperar o ponteiro head para a lista, por exemplo, para tamanho de bloco 16, precisamos usar `head_16`.
- Remover o primeiro bloco da lista e retorná-lo.

Mais notavelmente, sempre podemos retornar o primeiro elemento da lista e não precisamos mais percorrer a lista completa. Assim, alocações são muito mais rápidas do que com o alocador de lista encadeada.

#### Tamanhos de Bloco e Memória Desperdiçada

Dependendo dos tamanhos de bloco, perdemos muita memória ao arredondar para cima. Por exemplo, quando um bloco de 512 bytes é retornado para uma alocação de 128 bytes, três quartos da memória alocada estão não utilizados. Ao definir tamanhos de bloco razoáveis, é possível limitar a quantidade de memória desperdiçada até certo ponto. Por exemplo, ao usar as potências de 2 (4, 8, 16, 32, 64, 128, …) como tamanhos de bloco, podemos limitar o desperdício de memória a metade do tamanho de alocação no pior caso e um quarto do tamanho de alocação no caso médio.

Também é comum otimizar tamanhos de bloco com base em tamanhos de alocação comuns em um programa. Por exemplo, poderíamos adicionar adicionalmente o tamanho de bloco 24 para melhorar o uso de memória para programas que frequentemente realizam alocações de 24 bytes. Dessa forma, a quantidade de memória desperdiçada frequentemente pode ser reduzida sem perder os benefícios de desempenho.

#### Desalocação

Assim como a alocação, a desalocação também é muito performática. Ela envolve os seguintes passos:

- Arredondar o tamanho de alocação liberado para cima até o próximo tamanho de bloco. Isso é necessário já que o compilador passa apenas o tamanho de alocação solicitado para `dealloc`, não o tamanho do bloco que foi retornado por `alloc`. Ao usar a mesma função de ajuste de tamanho em tanto `alloc` quanto `dealloc`, podemos garantir que sempre liberamos a quantidade correta de memória.
- Recuperar o ponteiro head para a lista.
- Adicionar o bloco liberado à frente da lista atualizando o ponteiro head.

Mais notavelmente, nenhum percurso da lista é necessário para desalocação também. Isso significa que o tempo necessário para uma chamada `dealloc` permanece o mesmo independentemente do comprimento da lista.

#### Alocador de Fallback

Dado que alocações grandes (>2&nbsp;KB) são frequentemente raras, especialmente em kernels de sistemas operacionais, pode fazer sentido recorrer a um alocador diferente para essas alocações. Por exemplo, poderíamos recorrer a um alocador de lista encadeada para alocações maiores que 2048 bytes a fim de reduzir o desperdício de memória. Como apenas muito poucas alocações desse tamanho são esperadas, a lista encadeada permaneceria pequena e as (des)alocações ainda seriam razoavelmente rápidas.

#### Criando Novos Blocos

Acima, sempre assumimos que há blocos suficientes de um tamanho específico na lista para atender todas as requisições de alocação. No entanto, em algum ponto, a lista encadeada para um determinado tamanho de bloco fica vazia. Neste ponto, existem duas maneiras pelas quais podemos criar novos blocos não utilizados de um tamanho específico para atender uma requisição de alocação:

- Alocar um novo bloco do alocador de fallback (se houver um).
- Dividir um bloco maior de uma lista diferente. Isso funciona melhor se os tamanhos de bloco forem potências de dois. Por exemplo, um bloco de 32 bytes pode ser dividido em dois blocos de 16 bytes.

Para nossa implementação, alocaremos novos blocos do alocador de fallback, já que a implementação é muito mais simples.

### Implementação

Agora que sabemos como um alocador de bloco de tamanho fixo funciona, podemos começar nossa implementação. Não dependeremos da implementação do alocador de lista encadeada criado na seção anterior, então você pode seguir esta parte mesmo se pulou a implementação do alocador de lista encadeada.

#### Nó da Lista

Começamos nossa implementação criando um tipo `ListNode` em um novo módulo `allocator::fixed_size_block`:

```rust
// em src/allocator.rs

pub mod fixed_size_block;
```

```rust
// em src/allocator/fixed_size_block.rs

struct ListNode {
    next: Option<&'static mut ListNode>,
}
```

Este tipo é similar ao tipo `ListNode` de nossa [implementação de alocador de lista encadeada], com a diferença de que não temos um campo `size`. Ele não é necessário porque cada bloco em uma lista tem o mesmo tamanho com o design de alocador de bloco de tamanho fixo.

[implementação de alocador de lista encadeada]: #the-allocator-type

#### Tamanhos de Bloco

Em seguida, definimos uma slice constante `BLOCK_SIZES` com os tamanhos de bloco usados para nossa implementação:

```rust
// em src/allocator/fixed_size_block.rs

/// Os tamanhos de bloco a usar.
///
/// Os tamanhos devem cada um ser potência de 2 porque também são usados como
/// o alinhamento de bloco (alinhamentos devem ser sempre potências de 2).
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];
```

Como tamanhos de bloco, usamos potências de 2, começando de 8 até 2048. Não definimos tamanhos de bloco menores que 8 porque cada bloco deve ser capaz de armazenar um ponteiro de 64 bits para o próximo bloco quando liberado. Para alocações maiores que 2048 bytes, recorreremos a um alocador de lista encadeada.

Para simplificar a implementação, definimos o tamanho de um bloco como seu alinhamento necessário na memória. Então um bloco de 16 bytes sempre está alinhado em um limite de 16 bytes e um bloco de 512 bytes está alinhado em um limite de 512 bytes. Como alinhamentos sempre precisam ser potências de 2, isso exclui quaisquer outros tamanhos de bloco. Se precisarmos de tamanhos de bloco que não são potências de 2 no futuro, ainda podemos ajustar nossa implementação para isso (por exemplo, definindo um segundo array `BLOCK_ALIGNMENTS`).

#### O Tipo Alocador

Usando o tipo `ListNode` e a slice `BLOCK_SIZES`, agora podemos definir nosso tipo alocador:

```rust
// em src/allocator/fixed_size_block.rs

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}
```

O campo `list_heads` é um array de ponteiros `head`, um para cada tamanho de bloco. Isso é implementado usando o `len()` da slice `BLOCK_SIZES` como o comprimento do array. Como um alocador de fallback para alocações maiores que o maior tamanho de bloco, usamos o alocador fornecido pela crate `linked_list_allocator`. Também poderíamos usar o `LinkedListAllocator` que implementamos nós mesmos em vez disso, mas ele tem a desvantagem de que não [mescla blocos liberados].

[mescla blocos liberados]: #merging-freed-blocks

Para construir um `FixedSizeBlockAllocator`, fornecemos as mesmas funções `new` e `init` que implementamos para os outros tipos de alocadores também:

```rust
// em src/allocator/fixed_size_block.rs

impl FixedSizeBlockAllocator {
    /// Cria um FixedSizeBlockAllocator vazio.
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Inicializa o alocador com os limites de heap fornecidos.
    ///
    /// Esta função é unsafe porque o chamador deve garantir que os
    /// limites de heap fornecidos sejam válidos e que o heap esteja não utilizado. Este método deve ser
    /// chamado apenas uma vez.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe { self.fallback_allocator.init(heap_start, heap_size); }
    }
}
```

A função `new` apenas inicializa o array `list_heads` com nós vazios e cria um alocador de lista encadeada [`empty`] como `fallback_allocator`. A constante `EMPTY` é necessária para dizer ao compilador Rust que queremos inicializar o array com um valor constante. Inicializar o array diretamente como `[None; BLOCK_SIZES.len()]` não funciona, porque então o compilador exigiria que `Option<&'static mut ListNode>` implementasse a trait `Copy`, o que ele não faz. Esta é uma limitação atual do compilador Rust, que pode desaparecer no futuro.

[`empty`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.empty

A função `init` unsafe apenas chama a função [`init`] do `fallback_allocator` sem fazer nenhuma inicialização adicional do array `list_heads`. Em vez disso, inicializaremos as listas preguiçosamente em chamadas `alloc` e `dealloc`.

[`init`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.init

Por conveniência, também criamos um método privado `fallback_alloc` que aloca usando o `fallback_allocator`:

```rust
// em src/allocator/fixed_size_block.rs

use alloc::alloc::Layout;
use core::ptr;

impl FixedSizeBlockAllocator {
    /// Aloca usando o alocador de fallback.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}
```

O tipo [`Heap`] da crate `linked_list_allocator` não implementa [`GlobalAlloc`] (já que [não é possível sem bloqueio]). Em vez disso, ele fornece um método [`allocate_first_fit`] que tem uma interface ligeiramente diferente. Em vez de retornar um `*mut u8` e usar um ponteiro nulo para sinalizar um erro, ele retorna um `Result<NonNull<u8>, ()>`. O tipo [`NonNull`] é uma abstração para um ponteiro bruto que é garantido de não ser um ponteiro nulo. Ao mapear o caso `Ok` para o método [`NonNull::as_ptr`] e o caso `Err` para um ponteiro nulo, podemos facilmente traduzir isso de volta para um tipo `*mut u8`.

[`Heap`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html
[não é possível sem bloqueio]: #globalalloc-and-mutability
[`allocate_first_fit`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.allocate_first_fit
[`NonNull`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html
[`NonNull::as_ptr`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html#method.as_ptr

#### Calculando o Índice da Lista

Antes de implementarmos a trait `GlobalAlloc`, definimos uma função auxiliar `list_index` que retorna o menor tamanho de bloco possível para um dado [`Layout`]:

```rust
// em src/allocator/fixed_size_block.rs

/// Escolhe um tamanho de bloco apropriado para o layout fornecido.
///
/// Retorna um índice no array `BLOCK_SIZES`.
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}
```

O bloco deve ter pelo menos o tamanho e alinhamento exigidos pelo `Layout` fornecido. Como definimos que o tamanho do bloco também é seu alinhamento, isso significa que o `required_block_size` é o [máximo] dos atributos [`size()`] e [`align()`] do layout. Para encontrar o próximo bloco maior na slice `BLOCK_SIZES`, primeiro usamos o método [`iter()`] para obter um iterador e então o método [`position()`] para encontrar o índice do primeiro bloco que é pelo menos tão grande quanto o `required_block_size`.

[máximo]: https://doc.rust-lang.org/core/cmp/trait.Ord.html#method.max
[`size()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.size
[`align()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align
[`iter()`]: https://doc.rust-lang.org/std/primitive.slice.html#method.iter
[`position()`]:  https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.position

Note que não retornamos o próprio tamanho de bloco, mas o índice na slice `BLOCK_SIZES`. A razão é que queremos usar o índice retornado como um índice no array `list_heads`.

#### Implementando `GlobalAlloc`

O último passo é implementar a trait `GlobalAlloc`:

```rust
// em src/allocator/fixed_size_block.rs

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

Como para os outros alocadores, não implementamos a trait `GlobalAlloc` diretamente para nosso tipo alocador, mas usamos o [wrapper `Locked`] para adicionar mutabilidade interior sincronizada. Como as implementações de `alloc` e `dealloc` são relativamente grandes, as introduzimos uma por uma a seguir.

##### `alloc`

A implementação do método `alloc` se parece com isso:

```rust
// no bloco `impl` em src/allocator/fixed_size_block.rs

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
                    // nenhum bloco existe na lista => alocar novo bloco
                    let block_size = BLOCK_SIZES[index];
                    // só funciona se todos os tamanhos de bloco forem uma potência de 2
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

Vamos passar por isso passo a passo:

Primeiro, usamos o método `Locked::lock` para obter uma referência mutável à instância do alocador envolvido. Em seguida, chamamos a função `list_index` que acabamos de definir para calcular o tamanho de bloco apropriado para o layout fornecido e obter o índice correspondente no array `list_heads`. Se este índice for `None`, nenhum tamanho de bloco se encaixa para a alocação, portanto usamos o `fallback_allocator` usando a função `fallback_alloc`.

Se o índice da lista for `Some`, tentamos remover o primeiro nó na lista correspondente iniciada por `list_heads[index]` usando o método [`Option::take`]. Se a lista não estiver vazia, entramos no branch `Some(node)` da instrução `match`, onde apontamos o ponteiro head da lista para o sucessor do `node` removido (usando [`take`][`Option::take`] novamente). Finalmente, retornamos o ponteiro `node` removido como um `*mut u8`.

[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

Se o head da lista for `None`, indica que a lista de blocos está vazia. Isso significa que precisamos construir um novo bloco como [descrito acima](#creating-new-blocks). Para isso, primeiro obtemos o tamanho do bloco atual da slice `BLOCK_SIZES` e o usamos como tanto o tamanho quanto o alinhamento para o novo bloco. Então criamos um novo `Layout` a partir dele e chamamos o método `fallback_alloc` para realizar a alocação. A razão para ajustar o layout e alinhamento é que o bloco será adicionado à lista de blocos na desalocação.

#### `dealloc`

A implementação do método `dealloc` se parece com isso:

```rust
// em src/allocator/fixed_size_block.rs

use core::{mem, ptr::NonNull};

// dentro do bloco `unsafe impl GlobalAlloc`

unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            let new_node = ListNode {
                next: allocator.list_heads[index].take(),
            };
            // verificar que o bloco tem tamanho e alinhamento necessários para armazenar nó
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

Como em `alloc`, primeiro usamos o método `lock` para obter uma referência mutável do alocador e então a função `list_index` para obter a lista de blocos correspondente ao `Layout` fornecido. Se o índice for `None`, nenhum tamanho de bloco adequado existe em `BLOCK_SIZES`, o que indica que a alocação foi criada pelo alocador de fallback. Portanto, usamos seu método [`deallocate`][`Heap::deallocate`] para liberar a memória novamente. O método espera um [`NonNull`] em vez de um `*mut u8`, então precisamos converter o ponteiro primeiro. (A chamada `unwrap` só falha quando o ponteiro é nulo, o que nunca deve acontecer quando o compilador chama `dealloc`.)

[`Heap::deallocate`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.deallocate

Se `list_index` retorna um índice de bloco, precisamos adicionar o bloco de memória liberado à lista. Para isso, primeiro criamos um novo `ListNode` que aponta para o head atual da lista (usando [`Option::take`] novamente). Antes de escrevermos o novo nó no bloco de memória liberado, primeiro afirmamos que o tamanho do bloco atual especificado por `index` tem o tamanho e alinhamento necessários para armazenar um `ListNode`. Então realizamos a escrita convertendo o ponteiro `*mut u8` fornecido para um ponteiro `*mut ListNode` e então chamando o método [`write`][`pointer::write`] unsafe nele. O último passo é definir o ponteiro head da lista, que atualmente é `None` já que chamamos `take` nele, para nosso `ListNode` recém-escrito. Para isso, convertemos o ponteiro bruto `new_node_ptr` para uma referência mutável.

[`pointer::write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write

Há algumas coisas que vale a pena notar:

- Não diferenciamos entre blocos alocados de uma lista de blocos e blocos alocados do alocador de fallback. Isso significa que novos blocos criados em `alloc` são adicionados à lista de blocos em `dealloc`, aumentando assim o número de blocos daquele tamanho.
- O método `alloc` é o único lugar onde novos blocos são criados em nossa implementação. Isso significa que inicialmente começamos com listas de blocos vazias e só preenchemos essas listas preguiçosamente quando alocações de seu tamanho de bloco são realizadas.
- Não precisamos de blocos `unsafe` em `alloc` e `dealloc`, mesmo que realizemos algumas operações `unsafe`. A razão é que Rust atualmente trata o corpo completo de funções unsafe como um grande bloco `unsafe`. Como usar blocos `unsafe` explícitos tem a vantagem de que é óbvio quais operações são unsafe e quais não são, há uma [RFC proposta](https://github.com/rust-lang/rfcs/pull/2585) para mudar este comportamento.

### Usando-o

Para usar nosso novo `FixedSizeBlockAllocator`, precisamos atualizar o static `ALLOCATOR` no módulo `allocator`:

```rust
// em src/allocator.rs

use fixed_size_block::FixedSizeBlockAllocator;

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(
    FixedSizeBlockAllocator::new());
```

Como a função `init` se comporta da mesma forma para todos os alocadores que implementamos, não precisamos modificar a chamada `init` em `init_heap`.

Quando agora executamos nossos testes `heap_allocation` novamente, todos os testes ainda devem passar:

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

Nosso novo alocador parece funcionar!

### Discussão

Embora a abordagem de bloco de tamanho fixo tenha um desempenho muito melhor do que a abordagem de lista encadeada, ela desperdiça até metade da memória ao usar potências de 2 como tamanhos de bloco. Se este trade-off vale a pena depende muito do tipo de aplicação. Para um kernel de sistema operacional, onde o desempenho é crítico, a abordagem de bloco de tamanho fixo parece ser a melhor escolha.

No lado da implementação, existem várias coisas que poderíamos melhorar em nossa implementação atual:

- Em vez de alocar blocos preguiçosamente apenas usando o alocador de fallback, pode ser melhor pré-preencher as listas para melhorar o desempenho das alocações iniciais.
- Para simplificar a implementação, permitimos apenas tamanhos de bloco que são potências de 2 para que também possamos usá-los como o alinhamento do bloco. Ao armazenar (ou calcular) o alinhamento de uma maneira diferente, também poderíamos permitir outros tamanhos de bloco arbitrários. Dessa forma, poderíamos adicionar mais tamanhos de bloco, por exemplo, para tamanhos de alocação comuns, a fim de minimizar a memória desperdiçada.
- Atualmente apenas criamos novos blocos, mas nunca os liberamos novamente. Isso resulta em fragmentação e pode eventualmente resultar em falha de alocação para alocações grandes. Pode fazer sentido impor um comprimento máximo de lista para cada tamanho de bloco. Quando o comprimento máximo é atingido, desalocações subsequentes são liberadas usando o alocador de fallback em vez de serem adicionadas à lista.
- Em vez de recorrer a um alocador de lista encadeada, poderíamos ter um alocador especial para alocações maiores que 4&nbsp;KiB. A ideia é utilizar [paginação], que opera em páginas de 4&nbsp;KiB, para mapear um bloco contínuo de memória virtual a frames físicos não contínuos. Dessa forma, fragmentação de memória não utilizada não é mais um problema para alocações grandes.
- Com tal alocador de página, pode fazer sentido adicionar tamanhos de bloco até 4&nbsp;KiB e descartar o alocador de lista encadeada completamente. As principais vantagens disso seriam fragmentação reduzida e melhor previsibilidade de desempenho, ou seja, melhor desempenho de pior caso.

[paginação]: @/edition-2/posts/08-paging-introduction/index.md

É importante notar que as melhorias de implementação descritas acima são apenas sugestões. Alocadores usados em kernels de sistemas operacionais são tipicamente altamente otimizados para a carga de trabalho específica do kernel, o que só é possível através de profiling extensivo.

### Variações

Também existem muitas variações do design de alocador de bloco de tamanho fixo. Dois exemplos populares são o _alocador slab_ e o _alocador buddy_, que também são usados em kernels populares como o Linux. A seguir, damos uma breve introdução a esses dois designs.

#### Alocador Slab

A ideia por trás de um [alocador slab] é usar tamanhos de bloco que correspondem diretamente a tipos selecionados no kernel. Dessa forma, alocações desses tipos se encaixam em um tamanho de bloco exatamente e nenhuma memória é desperdiçada. Às vezes, pode até ser possível pré-inicializar instâncias de tipo em blocos não utilizados para melhorar ainda mais o desempenho.

[alocador slab]: https://en.wikipedia.org/wiki/Slab_allocation

Alocação slab é frequentemente combinada com outros alocadores. Por exemplo, ela pode ser usada junto com um alocador de bloco de tamanho fixo para dividir ainda mais um bloco alocado a fim de reduzir o desperdício de memória. Também é frequentemente usada para implementar um [padrão de pool de objetos] em cima de uma única grande alocação.

[padrão de pool de objetos]: https://en.wikipedia.org/wiki/Object_pool_pattern

#### Alocador Buddy

Em vez de usar uma lista encadeada para gerenciar blocos liberados, o design [alocador buddy] usa uma estrutura de dados de [árvore binária] junto com tamanhos de bloco que são potências de 2. Quando um novo bloco de um certo tamanho é necessário, ele divide um bloco de tamanho maior em duas metades, criando assim dois nós filhos na árvore. Sempre que um bloco é liberado novamente, seu bloco vizinho na árvore é analisado. Se o vizinho também estiver livre, os dois blocos são unidos de volta para formar um bloco de duas vezes o tamanho.

A vantagem deste processo de mesclagem é que a [fragmentação externa] é reduzida para que pequenos blocos liberados possam ser reutilizados para uma alocação grande. Também não usa um alocador de fallback, então o desempenho é mais previsível. A maior desvantagem é que apenas tamanhos de bloco que são potências de 2 são possíveis, o que pode resultar em uma grande quantidade de memória desperdiçada devido à [fragmentação interna]. Por essa razão, alocadores buddy são frequentemente combinados com um alocador slab para dividir ainda mais um bloco alocado em múltiplos blocos menores.

[alocador buddy]: https://en.wikipedia.org/wiki/Buddy_memory_allocation
[árvore binária]: https://en.wikipedia.org/wiki/Binary_tree
[fragmentação externa]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#External_fragmentation
[fragmentação interna]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#Internal_fragmentation


## Resumo

Este post deu uma visão geral de diferentes designs de alocadores. Aprendemos como implementar um [alocador bump] básico, que distribui memória linearmente aumentando um único ponteiro `next`. Embora a alocação bump seja muito rápida, ela só pode reutilizar memória depois que todas as alocações foram liberadas. Por essa razão, raramente é usada como um alocador global.

[alocador bump]: @/edition-2/posts/11-allocator-designs/index.md#bump-allocator

Em seguida, criamos um [alocador de lista encadeada] que usa os próprios blocos de memória liberados para criar uma lista encadeada, a chamada [lista livre]. Esta lista torna possível armazenar um número arbitrário de blocos liberados de diferentes tamanhos. Embora nenhum desperdício de memória ocorra, a abordagem sofre de desempenho pobre porque uma requisição de alocação pode requerer um percurso completo da lista. Nossa implementação também sofre de [fragmentação externa] porque não mescla blocos adjacentes liberados de volta juntos.

[alocador de lista encadeada]: @/edition-2/posts/11-allocator-designs/index.md#linked-list-allocator
[lista livre]: https://en.wikipedia.org/wiki/Free_list

Para corrigir os problemas de desempenho da abordagem de lista encadeada, criamos um [alocador de bloco de tamanho fixo] que predefine um conjunto fixo de tamanhos de bloco. Para cada tamanho de bloco, uma [lista livre] separada existe, de modo que alocações e desalocações só precisam inserir/remover na frente da lista e são assim muito rápidas. Como cada alocação é arredondada para cima até o próximo tamanho de bloco maior, alguma memória é desperdiçada devido à [fragmentação interna].

[alocador de bloco de tamanho fixo]: @/edition-2/posts/11-allocator-designs/index.md#fixed-size-block-allocator

Existem muitos outros designs de alocadores com diferentes trade-offs. [Alocação slab] funciona bem para otimizar a alocação de estruturas comuns de tamanho fixo, mas não é aplicável em todas as situações. [Alocação buddy] usa uma árvore binária para mesclar blocos liberados de volta juntos, mas desperdiça uma grande quantidade de memória porque só suporta tamanhos de bloco que são potências de 2. Também é importante lembrar que cada implementação de kernel tem uma carga de trabalho única, então não há design de alocador "melhor" que se encaixe em todos os casos.

[Alocação slab]: @/edition-2/posts/11-allocator-designs/index.md#slab-allocator
[Alocação buddy]: @/edition-2/posts/11-allocator-designs/index.md#buddy-allocator


## O que vem a seguir?

Com este post, concluímos nossa implementação de gerenciamento de memória por enquanto. Em seguida, começaremos a explorar [_multitarefa_], começando com multitarefa cooperativa na forma de [_async/await_]. Em posts subsequentes, então exploraremos [_threads_], [_multiprocessamento_] e [_processos_].

[_multitarefa_]: https://en.wikipedia.org/wiki/Computer_multitasking
[_threads_]: https://en.wikipedia.org/wiki/Thread_(computing)
[_processos_]: https://en.wikipedia.org/wiki/Process_(computing)
[_multiprocessamento_]: https://en.wikipedia.org/wiki/Multiprocessing
[_async/await_]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html