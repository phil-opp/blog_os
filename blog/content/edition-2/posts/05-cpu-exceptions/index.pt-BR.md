+++
title = "Exceções de CPU"
weight = 5
path = "pt-BR/cpu-exceptions"
date = 2018-06-17

[extra]
chapter = "Interrupções"
# Please update this when updating the translation
translation_based_on_commit = "9753695744854686a6b80012c89b0d850a44b4b0"

# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

Exceções de CPU ocorrem em várias situações errôneas, por exemplo, ao acessar um endereço de memória inválido ou ao dividir por zero. Para reagir a elas, precisamos configurar uma _tabela de descritores de interrupção_ que fornece funções manipuladoras. Ao final desta postagem, nosso kernel será capaz de capturar [exceções de breakpoint] e retomar a execução normal posteriormente.

[exceções de breakpoint]: https://wiki.osdev.org/Exceptions#Breakpoint

<!-- more -->

Este blog é desenvolvido abertamente no [GitHub]. Se você tiver algum problema ou dúvida, abra um issue lá. Você também pode deixar comentários [na parte inferior]. O código-fonte completo desta publicação pode ser encontrado na branch [`post-05`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[na parte inferior]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-05

<!-- toc -->

## Visão Geral
Uma exceção sinaliza que algo está errado com a instrução atual. Por exemplo, a CPU emite uma exceção se a instrução atual tenta dividir por 0. Quando uma exceção ocorre, a CPU interrompe seu trabalho atual e imediatamente chama uma função manipuladora de exceção específica, dependendo do tipo de exceção.

No x86, existem cerca de 20 tipos diferentes de exceções de CPU. As mais importantes são:

- **Page Fault**: Um page fault ocorre em acessos ilegais à memória. Por exemplo, se a instrução atual tenta ler de uma página não mapeada ou tenta escrever em uma página somente leitura.
- **Invalid Opcode**: Esta exceção ocorre quando a instrução atual é inválida, por exemplo, quando tentamos usar novas [instruções SSE] em uma CPU antiga que não as suporta.
- **General Protection Fault**: Esta é a exceção com a gama mais ampla de causas. Ela ocorre em vários tipos de violações de acesso, como tentar executar uma instrução privilegiada em código de nível de usuário ou escrever em campos reservados de registradores de configuração.
- **Double Fault**: Quando uma exceção ocorre, a CPU tenta chamar a função manipuladora correspondente. Se outra exceção ocorre _enquanto chama o manipulador de exceção_, a CPU levanta uma exceção de double fault. Esta exceção também ocorre quando não há função manipuladora registrada para uma exceção.
- **Triple Fault**: Se uma exceção ocorre enquanto a CPU tenta chamar a função manipuladora de double fault, ela emite um _triple fault_ fatal. Não podemos capturar ou manipular um triple fault. A maioria dos processadores reage redefinindo-se e reinicializando o sistema operacional.

[instruções SSE]: https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions

Para a lista completa de exceções, consulte a [wiki do OSDev][exceptions].

[exceptions]: https://wiki.osdev.org/Exceptions

### A Tabela de Descritores de Interrupção
Para capturar e manipular exceções, precisamos configurar uma chamada _Tabela de Descritores de Interrupção_ (IDT - Interrupt Descriptor Table). Nesta tabela, podemos especificar uma função manipuladora para cada exceção de CPU. O hardware usa esta tabela diretamente, então precisamos seguir um formato predefinido. Cada entrada deve ter a seguinte estrutura de 16 bytes:

Tipo| Nome                     | Descrição
----|--------------------------|-----------------------------------
u16 | Function Pointer [0:15]  | Os bits inferiores do ponteiro para a função manipuladora.
u16 | GDT selector             | Seletor de um segmento de código na [tabela de descritores globais].
u16 | Options                  | (veja abaixo)
u16 | Function Pointer [16:31] | Os bits do meio do ponteiro para a função manipuladora.
u32 | Function Pointer [32:63] | Os bits restantes do ponteiro para a função manipuladora.
u32 | Reserved                 |

[tabela de descritores globais]: https://en.wikipedia.org/wiki/Global_Descriptor_Table

O campo options tem o seguinte formato:

Bits  | Nome                              | Descrição
------|-----------------------------------|-----------------------------------
0-2   | Interrupt Stack Table Index       | 0: Não troca stacks, 1-7: Troca para a n-ésima stack na Interrupt Stack Table quando este manipulador é chamado.
3-7   | Reserved              |
8     | 0: Interrupt Gate, 1: Trap Gate   | Se este bit é 0, as interrupções são desativadas quando este manipulador é chamado.
9-11  | must be one                       |
12    | must be zero                      |
13‑14 | Descriptor Privilege Level (DPL)  | O nível mínimo de privilégio necessário para chamar este manipulador.
15    | Present                           |

Cada exceção tem um índice predefinido na IDT. Por exemplo, a exceção invalid opcode tem índice de tabela 6 e a exceção page fault tem índice de tabela 14. Assim, o hardware pode automaticamente carregar a entrada IDT correspondente para cada exceção. A [Tabela de Exceções][exceptions] na wiki do OSDev mostra os índices IDT de todas as exceções na coluna "Vector nr.".

Quando uma exceção ocorre, a CPU aproximadamente faz o seguinte:

1. Empurra alguns registradores na pilha, incluindo o ponteiro de instrução e o registrador [RFLAGS]. (Usaremos esses valores mais tarde nesta postagem.)
2. Lê a entrada correspondente da Tabela de Descritores de Interrupção (IDT). Por exemplo, a CPU lê a 14ª entrada quando ocorre um page fault.
3. Verifica se a entrada está presente e, se não estiver, levanta um double fault.
4. Desativa interrupções de hardware se a entrada é um interrupt gate (bit 40 não está definido).
5. Carrega o seletor [GDT] especificado no CS (segmento de código).
6. Pula para a função manipuladora especificada.

[RFLAGS]: https://en.wikipedia.org/wiki/FLAGS_register
[GDT]: https://en.wikipedia.org/wiki/Global_Descriptor_Table

Não se preocupe com os passos 4 e 5 por enquanto; aprenderemos sobre a tabela de descritores globais e interrupções de hardware em postagens futuras.

## Um Tipo IDT
Em vez de criar nosso próprio tipo IDT, usaremos a [struct `InterruptDescriptorTable`] da crate `x86_64`, que se parece com isto:

[struct `InterruptDescriptorTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html

``` rust
#[repr(C)]
pub struct InterruptDescriptorTable {
    pub divide_by_zero: Entry<HandlerFunc>,
    pub debug: Entry<HandlerFunc>,
    pub non_maskable_interrupt: Entry<HandlerFunc>,
    pub breakpoint: Entry<HandlerFunc>,
    pub overflow: Entry<HandlerFunc>,
    pub bound_range_exceeded: Entry<HandlerFunc>,
    pub invalid_opcode: Entry<HandlerFunc>,
    pub device_not_available: Entry<HandlerFunc>,
    pub double_fault: Entry<HandlerFuncWithErrCode>,
    pub invalid_tss: Entry<HandlerFuncWithErrCode>,
    pub segment_not_present: Entry<HandlerFuncWithErrCode>,
    pub stack_segment_fault: Entry<HandlerFuncWithErrCode>,
    pub general_protection_fault: Entry<HandlerFuncWithErrCode>,
    pub page_fault: Entry<PageFaultHandlerFunc>,
    pub x87_floating_point: Entry<HandlerFunc>,
    pub alignment_check: Entry<HandlerFuncWithErrCode>,
    pub machine_check: Entry<HandlerFunc>,
    pub simd_floating_point: Entry<HandlerFunc>,
    pub virtualization: Entry<HandlerFunc>,
    pub security_exception: Entry<HandlerFuncWithErrCode>,
    // alguns campos omitidos
}
```

Os campos têm o tipo [`idt::Entry<F>`], que é uma struct que representa os campos de uma entrada IDT (veja a tabela acima). O parâmetro de tipo `F` define o tipo de função manipuladora esperado. Vemos que algumas entradas requerem uma [`HandlerFunc`] e algumas entradas requerem uma [`HandlerFuncWithErrCode`]. O page fault tem até seu próprio tipo especial: [`PageFaultHandlerFunc`].

[`idt::Entry<F>`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.Entry.html
[`HandlerFunc`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.HandlerFunc.html
[`HandlerFuncWithErrCode`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.HandlerFuncWithErrCode.html
[`PageFaultHandlerFunc`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.PageFaultHandlerFunc.html

Vamos olhar primeiro para o tipo `HandlerFunc`:

```rust
type HandlerFunc = extern "x86-interrupt" fn(_: InterruptStackFrame);
```

É um [type alias] para um tipo `extern "x86-interrupt" fn`. A palavra-chave `extern` define uma função com uma [convenção de chamada estrangeira] e é frequentemente usada para se comunicar com código C (`extern "C" fn`). Mas o que é a convenção de chamada `x86-interrupt`?

[type alias]: https://doc.rust-lang.org/book/ch20-03-advanced-types.html#creating-type-synonyms-with-type-aliases
[convenção de chamada estrangeira]: https://doc.rust-lang.org/nomicon/ffi.html#foreign-calling-conventions

## A Convenção de Chamada de Interrupção
Exceções são bastante similares a chamadas de função: A CPU pula para a primeira instrução da função chamada e a executa. Depois, a CPU pula para o endereço de retorno e continua a execução da função pai.

No entanto, há uma diferença importante entre exceções e chamadas de função: Uma chamada de função é invocada voluntariamente por uma instrução `call` inserida pelo compilador, enquanto uma exceção pode ocorrer em _qualquer_ instrução. Para entender as consequências desta diferença, precisamos examinar as chamadas de função em mais detalhes.

[Convenções de chamada] especificam os detalhes de uma chamada de função. Por exemplo, elas especificam onde os parâmetros da função são colocados (por exemplo, em registradores ou na pilha) e como os resultados são retornados. No x86_64 Linux, as seguintes regras se aplicam para funções C (especificadas no [System V ABI]):

[Convenções de chamada]: https://en.wikipedia.org/wiki/Calling_convention
[System V ABI]: https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf

- os primeiros seis argumentos inteiros são passados nos registradores `rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9`
- argumentos adicionais são passados na pilha
- resultados são retornados em `rax` e `rdx`

Note que Rust não segue a ABI do C (na verdade, [nem existe uma ABI Rust ainda][rust abi]), então essas regras se aplicam apenas a funções declaradas como `extern "C" fn`.

[rust abi]: https://github.com/rust-lang/rfcs/issues/600

### Registradores Preservados e Scratch
A convenção de chamada divide os registradores em duas partes: registradores _preservados_ e _scratch_.

Os valores dos registradores _preservados_ devem permanecer inalterados entre chamadas de função. Portanto, uma função chamada (a _"callee"_) só tem permissão para sobrescrever esses registradores se restaurar seus valores originais antes de retornar. Portanto, esses registradores são chamados de _"callee-saved"_. Um padrão comum é salvar esses registradores na pilha no início da função e restaurá-los logo antes de retornar.

Em contraste, uma função chamada tem permissão para sobrescrever registradores _scratch_ sem restrições. Se o chamador quiser preservar o valor de um registrador scratch entre uma chamada de função, ele precisa fazer backup e restaurá-lo antes da chamada de função (por exemplo, empurrando-o para a pilha). Portanto, os registradores scratch são _caller-saved_.

No x86_64, a convenção de chamada C especifica os seguintes registradores preservados e scratch:

registradores preservados | registradores scratch
---|---
`rbp`, `rbx`, `rsp`, `r12`, `r13`, `r14`, `r15` | `rax`, `rcx`, `rdx`, `rsi`, `rdi`, `r8`, `r9`, `r10`, `r11`
_callee-saved_ | _caller-saved_

O compilador conhece essas regras, então gera o código de acordo. Por exemplo, a maioria das funções começa com um `push rbp`, que faz backup de `rbp` na pilha (porque é um registrador callee-saved).

### Preservando Todos os Registradores
Em contraste com chamadas de função, exceções podem ocorrer em _qualquer_ instrução. Na maioria dos casos, não sabemos nem em tempo de compilação se o código gerado causará uma exceção. Por exemplo, o compilador não pode saber se uma instrução causa um stack overflow ou um page fault.

Como não sabemos quando uma exceção ocorre, não podemos fazer backup de nenhum registrador antes. Isso significa que não podemos usar uma convenção de chamada que depende de registradores caller-saved para manipuladores de exceção. Em vez disso, precisamos de uma convenção de chamada que preserva _todos os registradores_. A convenção de chamada `x86-interrupt` é tal convenção de chamada, então garante que todos os valores de registrador são restaurados para seus valores originais no retorno da função.

Note que isso não significa que todos os registradores são salvos na pilha na entrada da função. Em vez disso, o compilador apenas faz backup dos registradores que são sobrescritos pela função. Desta forma, código muito eficiente pode ser gerado para funções curtas que usam apenas alguns registradores.

### O Stack Frame de Interrupção
Em uma chamada de função normal (usando a instrução `call`), a CPU empurra o endereço de retorno antes de pular para a função alvo. No retorno da função (usando a instrução `ret`), a CPU retira este endereço de retorno e pula para ele. Então o stack frame de uma chamada de função normal se parece com isto:

![function stack frame](function-stack-frame.svg)

Para manipuladores de exceção e interrupção, no entanto, empurrar um endereço de retorno não seria suficiente, já que manipuladores de interrupção frequentemente executam em um contexto diferente (ponteiro de pilha, flags da CPU, etc.). Em vez disso, a CPU executa os seguintes passos quando uma interrupção ocorre:

0. **Salvando o antigo ponteiro de pilha**: A CPU lê os valores dos registradores ponteiro de pilha (`rsp`) e segmento de pilha (`ss`) e os lembra em um buffer interno.
1. **Alinhando o ponteiro de pilha**: Uma interrupção pode ocorrer em qualquer instrução, então o ponteiro de pilha pode ter qualquer valor também. No entanto, algumas instruções de CPU (por exemplo, algumas instruções SSE) requerem que o ponteiro de pilha esteja alinhado em um limite de 16 bytes, então a CPU realiza tal alinhamento logo após a interrupção.
2. **Trocando pilhas** (em alguns casos): Uma troca de pilha ocorre quando o nível de privilégio da CPU muda, por exemplo, quando uma exceção de CPU ocorre em um programa em modo usuário. Também é possível configurar trocas de pilha para interrupções específicas usando a chamada _Interrupt Stack Table_ (descrita na próxima postagem).
3. **Empurrando o antigo ponteiro de pilha**: A CPU empurra os valores `rsp` e `ss` do passo 0 para a pilha. Isso torna possível restaurar o ponteiro de pilha original ao retornar de um manipulador de interrupção.
4. **Empurrando e atualizando o registrador `RFLAGS`**: O registrador [`RFLAGS`] contém vários bits de controle e status. Na entrada de interrupção, a CPU muda alguns bits e empurra o valor antigo.
5. **Empurrando o ponteiro de instrução**: Antes de pular para a função manipuladora de interrupção, a CPU empurra o ponteiro de instrução (`rip`) e o segmento de código (`cs`). Isso é comparável ao push de endereço de retorno de uma chamada de função normal.
6. **Empurrando um código de erro** (para algumas exceções): Para algumas exceções específicas, como page faults, a CPU empurra um código de erro, que descreve a causa da exceção.
7. **Invocando o manipulador de interrupção**: A CPU lê o endereço e o descritor de segmento da função manipuladora de interrupção do campo correspondente na IDT. Ela então invoca este manipulador carregando os valores nos registradores `rip` e `cs`.

[`RFLAGS`]: https://en.wikipedia.org/wiki/FLAGS_register

Então o _interrupt stack frame_ se parece com isto:

![interrupt stack frame](exception-stack-frame.svg)

Na crate `x86_64`, o interrupt stack frame é representado pela struct [`InterruptStackFrame`]. Ela é passada para manipuladores de interrupção como `&mut` e pode ser usada para recuperar informações adicionais sobre a causa da exceção. A struct não contém campo de código de erro, já que apenas algumas exceções empurram um código de erro. Essas exceções usam o tipo de função [`HandlerFuncWithErrCode`] separado, que tem um argumento adicional `error_code`.

[`InterruptStackFrame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptStackFrame.html

### Por Trás das Cortinas
A convenção de chamada `x86-interrupt` é uma abstração poderosa que esconde quase todos os detalhes confusos do processo de manipulação de exceção. No entanto, às vezes é útil saber o que está acontecendo por trás das cortinas. Aqui está uma breve visão geral das coisas das quais a convenção de chamada `x86-interrupt` cuida:

- **Recuperando os argumentos**: A maioria das convenções de chamada espera que os argumentos sejam passados em registradores. Isso não é possível para manipuladores de exceção, já que não devemos sobrescrever nenhum valor de registrador antes de fazer backup deles na pilha. Em vez disso, a convenção de chamada `x86-interrupt` está ciente de que os argumentos já estão na pilha em um deslocamento específico.
- **Retornando usando `iretq`**: Como o interrupt stack frame difere completamente dos stack frames de chamadas de função normais, não podemos retornar de funções manipuladoras através da instrução `ret` normal. Então, em vez disso, a instrução `iretq` deve ser usada.
- **Manipulando o código de erro**: O código de erro, que é empurrado para algumas exceções, torna as coisas muito mais complexas. Ele muda o alinhamento da pilha (veja o próximo ponto) e precisa ser retirado da pilha antes de retornar. A convenção de chamada `x86-interrupt` manipula toda essa complexidade. No entanto, ela não sabe qual função manipuladora é usada para qual exceção, então precisa deduzir essa informação do número de argumentos da função. Isso significa que o programador ainda é responsável por usar o tipo de função correto para cada exceção. Felizmente, o tipo `InterruptDescriptorTable` definido pela crate `x86_64` garante que os tipos de função corretos são usados.
- **Alinhando a pilha**: Algumas instruções (especialmente instruções SSE) requerem um alinhamento de pilha de 16 bytes. A CPU garante esse alinhamento sempre que uma exceção ocorre, mas para algumas exceções ela o destrói novamente mais tarde quando empurra um código de erro. A convenção de chamada `x86-interrupt` cuida disso realinhando a pilha neste caso.

Se você estiver interessado em mais detalhes, também temos uma série de postagens que explica a manipulação de exceção usando [funções nuas] vinculadas [no final desta postagem][too-much-magic].

[funções nuas]: https://github.com/rust-lang/rfcs/blob/master/text/1201-naked-fns.md
[too-much-magic]: #muita-magica

## Implementação
Agora que entendemos a teoria, é hora de manipular exceções de CPU em nosso kernel. Começaremos criando um novo módulo interrupts em `src/interrupts.rs`, que primeiro cria uma função `init_idt` que cria uma nova `InterruptDescriptorTable`:

``` rust
// em src/lib.rs

pub mod interrupts;

// em src/interrupts.rs

use x86_64::structures::idt::InterruptDescriptorTable;

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
}
```

Agora podemos adicionar funções manipuladoras. Começamos adicionando um manipulador para a [exceção de breakpoint]. A exceção de breakpoint é a exceção perfeita para testar a manipulação de exceção. Seu único propósito é pausar temporariamente um programa quando a instrução de breakpoint `int3` é executada.

[exceção de breakpoint]: https://wiki.osdev.org/Exceptions#Breakpoint

A exceção de breakpoint é comumente usada em debuggers: Quando o usuário define um breakpoint, o debugger sobrescreve a instrução correspondente com a instrução `int3` para que a CPU lance a exceção de breakpoint quando atinge aquela linha. Quando o usuário quer continuar o programa, o debugger substitui a instrução `int3` pela instrução original novamente e continua o programa. Para mais detalhes, veja a série ["_How debuggers work_"].

["_How debuggers work_"]: https://eli.thegreenplace.net/2011/01/27/how-debuggers-work-part-2-breakpoints

Para nosso caso de uso, não precisamos sobrescrever nenhuma instrução. Em vez disso, queremos apenas imprimir uma mensagem quando a instrução de breakpoint é executada e então continuar o programa. Então vamos criar uma função `breakpoint_handler` simples e adicioná-la à nossa IDT:

```rust
// em src/interrupts.rs

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::println;

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    println!("EXCEÇÃO: BREAKPOINT\n{:#?}", stack_frame);
}
```

Nosso manipulador apenas produz uma mensagem e imprime de forma bonita o interrupt stack frame.

Quando tentamos compilá-lo, o seguinte erro ocorre:

```
error[E0658]: x86-interrupt ABI is experimental and subject to change (see issue #40180)
  --> src/main.rs:53:1
   |
53 | / extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
54 | |     println!("EXCEÇÃO: BREAKPOINT\n{:#?}", stack_frame);
55 | | }
   | |_^
   |
   = help: add #![feature(abi_x86_interrupt)] to the crate attributes to enable
```

Este erro ocorre porque a convenção de chamada `x86-interrupt` ainda é instável. Para usá-la de qualquer forma, temos que habilitá-la explicitamente adicionando `#![feature(abi_x86_interrupt)]` no topo do nosso `lib.rs`.

### Carregando a IDT
Para que a CPU use nossa nova tabela de descritores de interrupção, precisamos carregá-la usando a instrução [`lidt`]. A struct `InterruptDescriptorTable` da crate `x86_64` fornece um método [`load`][InterruptDescriptorTable::load] para isso. Vamos tentar usá-lo:

[`lidt`]: https://www.felixcloutier.com/x86/lgdt:lidt
[InterruptDescriptorTable::load]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html#method.load

```rust
// em src/interrupts.rs

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.load();
}
```

Quando tentamos compilar agora, o seguinte erro ocorre:

```
error: `idt` does not live long enough
  --> src/interrupts/mod.rs:43:5
   |
43 |     idt.load();
   |     ^^^ does not live long enough
44 | }
   | - borrowed value only lives until here
   |
   = note: borrowed value must be valid for the static lifetime...
```

Então o método `load` espera um `&'static self`, isto é, uma referência válida para o tempo de execução completo do programa. A razão é que a CPU acessará esta tabela em cada interrupção até carregarmos uma IDT diferente. Então usar um tempo de vida menor que `'static` poderia levar a bugs de use-after-free.

De fato, isso é exatamente o que acontece aqui. Nossa `idt` é criada na pilha, então ela é válida apenas dentro da função `init`. Depois, a memória da pilha é reutilizada para outras funções, então a CPU interpretaria memória aleatória da pilha como IDT. Felizmente, o método `InterruptDescriptorTable::load` codifica este requisito de tempo de vida em sua definição de função, para que o compilador Rust seja capaz de prevenir este possível bug em tempo de compilação.

Para corrigir este problema, precisamos armazenar nossa `idt` em um lugar onde ela tenha um tempo de vida `'static`. Para conseguir isso, poderíamos alocar nossa IDT no heap usando [`Box`] e então convertê-la para uma referência `'static`, mas estamos escrevendo um kernel de SO e, portanto, não temos um heap (ainda).

[`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html


Como alternativa, poderíamos tentar armazenar a IDT como uma `static`:

```rust
static IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
    IDT.breakpoint.set_handler_fn(breakpoint_handler);
    IDT.load();
}
```

No entanto, há um problema: Statics são imutáveis, então não podemos modificar a entrada de breakpoint da nossa função `init`. Poderíamos resolver este problema usando uma [`static mut`]:

[`static mut`]: https://doc.rust-lang.org/1.30.0/book/second-edition/ch19-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable

```rust
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
    unsafe {
        IDT.breakpoint.set_handler_fn(breakpoint_handler);
        IDT.load();
    }
}
```

Esta variante compila sem erros, mas está longe de ser idiomática. `static mut`s são muito propensas a data races, então precisamos de um [bloco `unsafe`] em cada acesso.

[bloco `unsafe`]: https://doc.rust-lang.org/1.30.0/book/second-edition/ch19-01-unsafe-rust.html#unsafe-superpowers

#### Lazy Statics ao Resgate
Felizmente, a macro `lazy_static` existe. Em vez de avaliar uma `static` em tempo de compilação, a macro realiza a inicialização quando a `static` é referenciada pela primeira vez. Assim, podemos fazer quase tudo no bloco de inicialização e somos até capazes de ler valores de tempo de execução.

Já importamos a crate `lazy_static` quando [criamos uma abstração para o buffer de texto VGA][vga text buffer lazy static]. Então podemos usar diretamente a macro `lazy_static!` para criar nossa IDT estática:

[vga text buffer lazy static]: @/edition-2/posts/03-vga-text-buffer/index.md#lazy-statics

```rust
// em src/interrupts.rs

use lazy_static::lazy_static;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}
```

Note como esta solução não requer blocos `unsafe`. A macro `lazy_static!` usa `unsafe` por trás dos panos, mas é abstraída em uma interface segura.

### Executando

O último passo para fazer exceções funcionarem em nosso kernel é chamar a função `init_idt` do nosso `main.rs`. Em vez de chamá-la diretamente, introduzimos uma função geral `init` em nosso `lib.rs`:

```rust
// em src/lib.rs

pub fn init() {
    interrupts::init_idt();
}
```

Com esta função, agora temos um lugar central para rotinas de inicialização que podem ser compartilhadas entre as diferentes funções `_start` em nosso `main.rs`, `lib.rs` e testes de integração.

Agora podemos atualizar a função `_start` do nosso `main.rs` para chamar `init` e então disparar uma exceção de breakpoint:

```rust
// em src/main.rs

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Olá Mundo{}", "!");

    blog_os::init(); // novo

    // invoca uma exceção de breakpoint
    x86_64::instructions::interrupts::int3(); // novo

    // como antes
    #[cfg(test)]
    test_main();

    println!("Não crashou!");
    loop {}
}
```

Quando executamos agora no QEMU (usando `cargo run`), vemos o seguinte:

![QEMU printing `EXCEÇÃO: BREAKPOINT` and the interrupt stack frame](qemu-breakpoint-exception.png)

Funciona! A CPU invoca com sucesso nosso manipulador de breakpoint, que imprime a mensagem, e então retorna de volta para a função `_start`, onde a mensagem `Não crashou!` é impressa.

Vemos que o interrupt stack frame nos diz os ponteiros de instrução e pilha no momento em que a exceção ocorreu. Esta informação é muito útil ao depurar exceções inesperadas.

### Adicionando um Teste

Vamos criar um teste que garante que o acima continue funcionando. Primeiro, atualizamos a função `_start` para também chamar `init`:

```rust
// em src/lib.rs

/// Ponto de entrada para `cargo test`
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    init();      // novo
    test_main();
    loop {}
}
```

Lembre-se, esta função `_start` é usada quando executamos `cargo test --lib`, já que Rust testa o `lib.rs` completamente independente do `main.rs`. Precisamos chamar `init` aqui para configurar uma IDT antes de executar os testes.

Agora podemos criar um teste `test_breakpoint_exception`:

```rust
// em src/interrupts.rs

#[test_case]
fn test_breakpoint_exception() {
    // invoca uma exceção de breakpoint
    x86_64::instructions::interrupts::int3();
}
```

O teste invoca a função `int3` para disparar uma exceção de breakpoint. Ao verificar que a execução continua depois, verificamos que nosso manipulador de breakpoint está funcionando corretamente.

Você pode tentar este novo teste executando `cargo test` (todos os testes) ou `cargo test --lib` (apenas testes de `lib.rs` e seus módulos). Você deve ver o seguinte na saída:

```
blog_os::interrupts::test_breakpoint_exception...	[ok]
```

## Muita Mágica?
A convenção de chamada `x86-interrupt` e o tipo [`InterruptDescriptorTable`] tornaram o processo de manipulação de exceção relativamente simples e indolor. Se isso foi muita mágica para você e você gostaria de aprender todos os detalhes sórdidos da manipulação de exceção, nós temos você coberto: Nossa série ["Manipulando Exceções com Funções Nuas"] mostra como manipular exceções sem a convenção de chamada `x86-interrupt` e também cria seu próprio tipo IDT. Historicamente, essas postagens eram as principais postagens de manipulação de exceção antes que a convenção de chamada `x86-interrupt` e a crate `x86_64` existissem. Note que essas postagens são baseadas na [primeira edição] deste blog e podem estar desatualizadas.

["Manipulando Exceções com Funções Nuas"]: @/edition-1/extra/naked-exceptions/_index.md
[`InterruptDescriptorTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html
[primeira edição]: @/edition-1/_index.md

## O Que Vem a Seguir?
Capturamos com sucesso nossa primeira exceção e retornamos dela! O próximo passo é garantir que capturemos todas as exceções porque uma exceção não capturada causa um [triple fault] fatal, que leva a uma redefinição do sistema. A próxima postagem explica como podemos evitar isso capturando corretamente [double faults].

[triple fault]: https://wiki.osdev.org/Triple_Fault
[double faults]: https://wiki.osdev.org/Double_Fault#Double_Fault