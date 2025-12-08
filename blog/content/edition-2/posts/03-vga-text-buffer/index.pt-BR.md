+++
title = "Modo de Texto VGA"
weight = 3
path = "pt-BR/vga-text-mode"
date  = 2018-02-26

[extra]
chapter = "O Básico"
# Please update this when updating the translation
translation_based_on_commit = "9753695744854686a6b80012c89b0d850a44b4b0"
# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

O [modo de texto VGA] é uma maneira simples de imprimir texto na tela. Neste post, criamos uma interface que torna seu uso seguro e simples ao encapsular toda a unsafety em um módulo separado. Também implementamos suporte para as [macros de formatação] do Rust.

[modo de texto VGA]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode
[macros de formatação]: https://doc.rust-lang.org/std/fmt/#related-macros

<!-- more -->

Este blog é desenvolvido abertamente no [GitHub]. Se você tiver algum problema ou dúvida, abra um issue lá. Você também pode deixar comentários [na parte inferior]. O código-fonte completo desta publicação pode ser encontrado na branch [`post-03`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[na parte inferior]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-03

<!-- toc -->

## O Buffer de Texto VGA
Para imprimir um caractere na tela em modo de texto VGA, é preciso escrevê-lo no buffer de texto do hardware VGA. O buffer de texto VGA é um array bidimensional com tipicamente 25 linhas e 80 colunas, que é renderizado diretamente na tela. Cada entrada do array descreve um único caractere da tela através do seguinte formato:

Bit(s) | Valor
------ | ----------------
0-7    | Ponto de código ASCII
8-11   | Cor do primeiro plano
12-14  | Cor do fundo
15     | Piscar

O primeiro byte representa o caractere que deve ser impresso na [codificação ASCII]. Para ser mais específico, não é exatamente ASCII, mas um conjunto de caracteres chamado [_página de código 437_] com alguns caracteres adicionais e pequenas modificações. Para simplificar, continuaremos chamando-o de caractere ASCII neste post.

[codificação ASCII]: https://en.wikipedia.org/wiki/ASCII
[_página de código 437_]: https://en.wikipedia.org/wiki/Code_page_437

O segundo byte define como o caractere é exibido. Os primeiros quatro bits definem a cor do primeiro plano, os próximos três bits a cor do fundo, e o último bit se o caractere deve piscar. As seguintes cores estão disponíveis:

Número | Cor           | Número + Bit Brilhante | Cor Brilhante
------ | ------------- | ---------------------- | --------------
0x0    | Preto         | 0x8                    | Cinza Escuro
0x1    | Azul          | 0x9                    | Azul Claro
0x2    | Verde         | 0xa                    | Verde Claro
0x3    | Ciano         | 0xb                    | Ciano Claro
0x4    | Vermelho      | 0xc                    | Vermelho Claro
0x5    | Magenta       | 0xd                    | Rosa
0x6    | Marrom        | 0xe                    | Amarelo
0x7    | Cinza Claro   | 0xf                    | Branco

O bit 4 é o _bit brilhante_, que transforma, por exemplo, azul em azul claro. Para a cor de fundo, este bit é reaproveitado como o bit de piscar.

O buffer de texto VGA é acessível via [I/O mapeado em memória] no endereço `0xb8000`. Isso significa que leituras e escritas naquele endereço não acessam a RAM, mas acessam diretamente o buffer de texto no hardware VGA. Isso significa que podemos lê-lo e escrevê-lo através de operações normais de memória naquele endereço.

[I/O mapeado em memória]: https://en.wikipedia.org/wiki/Memory-mapped_I/O

Note que hardware mapeado em memória pode não suportar todas as operações normais de RAM. Por exemplo, um dispositivo poderia suportar apenas leituras byte a byte e retornar lixo quando um `u64` é lido. Felizmente, o buffer de texto [suporta leituras e escritas normais], então não precisamos tratá-lo de maneira especial.

[suporta leituras e escritas normais]: https://web.stanford.edu/class/cs140/projects/pintos/specs/freevga/vga/vgamem.htm#manip

## Um Módulo Rust
Agora que sabemos como o buffer VGA funciona, podemos criar um módulo Rust para lidar com a impressão:

```rust
// em src/main.rs
mod vga_buffer;
```

Para o conteúdo deste módulo, criamos um novo arquivo `src/vga_buffer.rs`. Todo o código abaixo vai para nosso novo módulo (a menos que especificado o contrário).

### Cores
Primeiro, representamos as diferentes cores usando um enum:

```rust
// em src/vga_buffer.rs

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
Usamos um [enum estilo C] aqui para especificar explicitamente o número para cada cor. Por causa do atributo `repr(u8)`, cada variante do enum é armazenada como um `u8`. Na verdade, 4 bits seriam suficientes, mas Rust não tem um tipo `u4`.

[enum estilo C]: https://doc.rust-lang.org/rust-by-example/custom_types/enum/c_like.html

Normalmente o compilador emitiria um aviso para cada variante não utilizada. Ao usar o atributo `#[allow(dead_code)]`, desabilitamos esses avisos para o enum `Color`.

Ao [derivar] as traits [`Copy`], [`Clone`], [`Debug`], [`PartialEq`] e [`Eq`], habilitamos [semântica de cópia] para o tipo e o tornamos imprimível e comparável.

[derivar]: https://doc.rust-lang.org/rust-by-example/trait/derive.html
[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[`Clone`]: https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html
[`Debug`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html
[`PartialEq`]: https://doc.rust-lang.org/nightly/core/cmp/trait.PartialEq.html
[`Eq`]: https://doc.rust-lang.org/nightly/core/cmp/trait.Eq.html
[semântica de cópia]: https://doc.rust-lang.org/1.30.0/book/first-edition/ownership.html#copy-types

Para representar um código de cor completo que especifica as cores de primeiro plano e de fundo, criamos um [newtype] em cima de `u8`:

[newtype]: https://doc.rust-lang.org/rust-by-example/generics/new_types.html

```rust
// em src/vga_buffer.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}
```
A struct `ColorCode` contém o byte de cor completo, contendo as cores de primeiro plano e de fundo. Como antes, derivamos as traits `Copy` e `Debug` para ela. Para garantir que o `ColorCode` tenha exatamente o mesmo layout de dados que um `u8`, usamos o atributo [`repr(transparent)`].

[`repr(transparent)`]: https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent

### Buffer de Texto
Agora podemos adicionar estruturas para representar um caractere da tela e o buffer de texto:

```rust
// em src/vga_buffer.rs

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
Como a ordenação dos campos em structs padrão é indefinida em Rust, precisamos do atributo [`repr(C)`]. Ele garante que os campos da struct sejam dispostos exatamente como em uma struct C e, portanto, garante a ordenação correta dos campos. Para a struct `Buffer`, usamos [`repr(transparent)`] novamente para garantir que ela tenha o mesmo layout de memória que seu único campo.

[`repr(C)`]: https://doc.rust-lang.org/nightly/nomicon/other-reprs.html#reprc

Para realmente escrever na tela, agora criamos um tipo writer:

```rust
// em src/vga_buffer.rs

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}
```
O writer sempre escreverá na última linha e deslocará as linhas para cima quando uma linha estiver cheia (ou no `\n`). O campo `column_position` acompanha a posição atual na última linha. As cores de primeiro plano e de fundo atuais são especificadas por `color_code` e uma referência ao buffer VGA é armazenada em `buffer`. Note que precisamos de um [lifetime explícito] aqui para dizer ao compilador por quanto tempo a referência é válida. O lifetime [`'static`] especifica que a referência é válida durante toda a execução do programa (o que é verdade para o buffer de texto VGA).

[lifetime explícito]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#lifetime-annotation-syntax
[`'static`]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime

### Impressão
Agora podemos usar o `Writer` para modificar os caracteres do buffer. Primeiro criamos um método para escrever um único byte ASCII:

```rust
// em src/vga_buffer.rs

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
Se o byte é o byte de [newline] `\n`, o writer não imprime nada. Em vez disso, ele chama um método `new_line`, que implementaremos mais tarde. Outros bytes são impressos na tela no segundo caso `match`.

[newline]: https://en.wikipedia.org/wiki/Newline

Ao imprimir um byte, o writer verifica se a linha atual está cheia. Nesse caso, uma chamada `new_line` é usada para quebrar a linha. Então ele escreve um novo `ScreenChar` no buffer na posição atual. Finalmente, a posição da coluna atual é avançada.

Para imprimir strings inteiras, podemos convertê-las em bytes e imprimi-los um por um:

```rust
// em src/vga_buffer.rs

impl Writer {
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // byte ASCII imprimível ou newline
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // não faz parte da faixa ASCII imprimível
                _ => self.write_byte(0xfe),
            }

        }
    }
}
```

O buffer de texto VGA suporta apenas ASCII e os bytes adicionais da [página de código 437]. Strings Rust são [UTF-8] por padrão, então podem conter bytes que não são suportados pelo buffer de texto VGA. Usamos um `match` para diferenciar bytes ASCII imprimíveis (um newline ou qualquer coisa entre um caractere de espaço e um caractere `~`) e bytes não imprimíveis. Para bytes não imprimíveis, imprimimos um caractere `■`, que tem o código hexadecimal `0xfe` no hardware VGA.

[página de código 437]: https://en.wikipedia.org/wiki/Code_page_437
[UTF-8]: https://www.fileformat.info/info/unicode/utf8.htm

#### Experimente!
Para escrever alguns caracteres na tela, você pode criar uma função temporária:

```rust
// em src/vga_buffer.rs

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
Primeiro ele cria um novo Writer que aponta para o buffer VGA em `0xb8000`. A sintaxe para isso pode parecer um pouco estranha: Primeiro, convertemos o inteiro `0xb8000` como um [ponteiro bruto] mutável. Então o convertemos em uma referência mutável ao desreferenciá-lo (através de `*`) e imediatamente emprestar novamente (através de `&mut`). Esta conversão requer um [bloco `unsafe`], pois o compilador não pode garantir que o ponteiro bruto é válido.

[ponteiro bruto]: https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
[bloco `unsafe`]: https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html

Então ele escreve o byte `b'H'` nele. O prefixo `b` cria um [byte literal], que representa um caractere ASCII. Ao escrever as strings `"ello "` e `"Wörld!"`, testamos nosso método `write_string` e o tratamento de caracteres não imprimíveis. Para ver a saída, precisamos chamar a função `print_something` da nossa função `_start`:

```rust
// em src/main.rs

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    vga_buffer::print_something();

    loop {}
}
```

Quando executamos nosso projeto agora, um `Hello W■■rld!` deve ser impresso no canto inferior _esquerdo_ da tela em amarelo:

[byte literal]: https://doc.rust-lang.org/reference/tokens.html#byte-literals

![QEMU exibindo um `Hello W■■rld!` amarelo no canto inferior esquerdo](vga-hello.png)

Note que o `ö` é impresso como dois caracteres `■`. Isso ocorre porque `ö` é representado por dois bytes em [UTF-8], que ambos não se enquadram na faixa ASCII imprimível. Na verdade, esta é uma propriedade fundamental do UTF-8: os bytes individuais de valores multi-byte nunca são ASCII válido.

### Volatile
Acabamos de ver que nossa mensagem foi impressa corretamente. No entanto, pode não funcionar com futuros compiladores Rust que otimizam de forma mais agressiva.

O problema é que escrevemos apenas no `Buffer` e nunca lemos dele novamente. O compilador não sabe que realmente acessamos memória do buffer VGA (em vez de RAM normal) e não sabe nada sobre o efeito colateral de que alguns caracteres aparecem na tela. Então ele pode decidir que essas escritas são desnecessárias e podem ser omitidas. Para evitar esta otimização errônea, precisamos especificar essas escritas como _[volatile]_. Isso diz ao compilador que a escrita tem efeitos colaterais e não deve ser otimizada.

[volatile]: https://en.wikipedia.org/wiki/Volatile_(computer_programming)

Para usar escritas volatile para o buffer VGA, usamos a biblioteca [volatile][volatile crate]. Esta _crate_ (é assim que os pacotes são chamados no mundo Rust) fornece um tipo wrapper `Volatile` com métodos `read` e `write`. Esses métodos usam internamente as funções [read_volatile] e [write_volatile] da biblioteca core e, portanto, garantem que as leituras/escritas não sejam otimizadas.

[volatile crate]: https://docs.rs/volatile
[read_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.read_volatile.html
[write_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.write_volatile.html

Podemos adicionar uma dependência na crate `volatile` adicionando-a à seção `dependencies` do nosso `Cargo.toml`:

```toml
# em Cargo.toml

[dependencies]
volatile = "0.2.6"
```

Certifique-se de especificar a versão `0.2.6` do `volatile`. Versões mais novas da crate não são compatíveis com este post.
`0.2.6` é o número de versão [semântico]. Para mais informações, veja o guia [Specifying Dependencies] da documentação do cargo.

[semântico]: https://semver.org/
[Specifying Dependencies]: https://doc.crates.io/specifying-dependencies.html

Vamos usá-lo para tornar as escritas no buffer VGA volatile. Atualizamos nosso tipo `Buffer` da seguinte forma:

```rust
// em src/vga_buffer.rs

use volatile::Volatile;

struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```
Em vez de um `ScreenChar`, agora estamos usando um `Volatile<ScreenChar>`. (O tipo `Volatile` é [genérico] e pode envolver (quase) qualquer tipo). Isso garante que não possamos escrever nele "normalmente" acidentalmente. Em vez disso, temos que usar o método `write` agora.

[genérico]: https://doc.rust-lang.org/book/ch10-01-syntax.html

Isso significa que temos que atualizar nosso método `Writer::write_byte`:

```rust
// em src/vga_buffer.rs

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

Em vez de uma atribuição típica usando `=`, agora estamos usando o método `write`. Agora podemos garantir que o compilador nunca otimizará esta escrita.

### Macros de Formatação
Seria bom suportar as macros de formatação do Rust também. Dessa forma, podemos facilmente imprimir diferentes tipos, como inteiros ou floats. Para suportá-las, precisamos implementar a trait [`core::fmt::Write`]. O único método necessário desta trait é `write_str`, que se parece bastante com nosso método `write_string`, apenas com um tipo de retorno `fmt::Result`:

[`core::fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

```rust
// em src/vga_buffer.rs

use core::fmt;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
```
O `Ok(())` é apenas um Result `Ok` contendo o tipo `()`.

Agora podemos usar as macros de formatação `write!`/`writeln!` embutidas do Rust:

```rust
// em src/vga_buffer.rs

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

Agora você deve ver um `Hello! The numbers are 42 and 0.3333333333333333` na parte inferior da tela. A chamada `write!` retorna um `Result` que causa um aviso se não usado, então chamamos a função [`unwrap`] nele, que entra em panic se ocorrer um erro. Isso não é um problema no nosso caso, pois escritas no buffer VGA nunca falham.

[`unwrap`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.unwrap

### Newlines
Agora, simplesmente ignoramos newlines e caracteres que não cabem mais na linha. Em vez disso, queremos mover cada caractere uma linha para cima (a linha superior é excluída) e começar no início da última linha novamente. Para fazer isso, adicionamos uma implementação para o método `new_line` do `Writer`:

```rust
// em src/vga_buffer.rs

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
Iteramos sobre todos os caracteres da tela e movemos cada caractere uma linha para cima. Note que o limite superior da notação de intervalo (`..`) é exclusivo. Também omitimos a linha 0 (o primeiro intervalo começa em `1`) porque é a linha que é deslocada para fora da tela.

Para finalizar o código de newline, adicionamos o método `clear_row`:

```rust
// em src/vga_buffer.rs

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
Este método limpa uma linha sobrescrevendo todos os seus caracteres com um caractere de espaço.

## Uma Interface Global
Para fornecer um writer global que possa ser usado como uma interface de outros módulos sem carregar uma instância `Writer` por aí, tentamos criar um `WRITER` static:

```rust
// em src/vga_buffer.rs

pub static WRITER: Writer = Writer {
    column_position: 0,
    color_code: ColorCode::new(Color::Yellow, Color::Black),
    buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
};
```

No entanto, se tentarmos compilá-lo agora, os seguintes erros ocorrem:

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

Para entender o que está acontecendo aqui, precisamos saber que statics são inicializados em tempo de compilação, ao contrário de variáveis normais que são inicializadas em tempo de execução. O componente do compilador Rust que avalia tais expressões de inicialização é chamado de "[const evaluator]". Sua funcionalidade ainda é limitada, mas há trabalho contínuo para expandi-la, por exemplo no RFC "[Allow panicking in constants]".

[const evaluator]: https://rustc-dev-guide.rust-lang.org/const-eval.html
[Allow panicking in constants]: https://github.com/rust-lang/rfcs/pull/2345

O problema com `ColorCode::new` seria solucionável usando [funções `const`], mas o problema fundamental aqui é que o const evaluator do Rust não é capaz de converter ponteiros brutos em referências em tempo de compilação. Talvez funcione algum dia, mas até lá, precisamos encontrar outra solução.

[funções `const`]: https://doc.rust-lang.org/reference/const_eval.html#const-functions

### Lazy Statics
A inicialização única de statics com funções não const é um problema comum em Rust. Felizmente, já existe uma boa solução em uma crate chamada [lazy_static]. Esta crate fornece uma macro `lazy_static!` que define um `static` inicializado lazily. Em vez de calcular seu valor em tempo de compilação, o `static` se inicializa lazily quando é acessado pela primeira vez. Assim, a inicialização acontece em tempo de execução, então código de inicialização arbitrariamente complexo é possível.

[lazy_static]: https://docs.rs/lazy_static/1.0.1/lazy_static/

Vamos adicionar a crate `lazy_static` ao nosso projeto:

```toml
# em Cargo.toml

[dependencies.lazy_static]
version = "1.0"
features = ["spin_no_std"]
```

Precisamos do recurso `spin_no_std`, já que não vinculamos a biblioteca padrão.

Com `lazy_static`, podemos definir nosso `WRITER` static sem problemas:

```rust
// em src/vga_buffer.rs

use lazy_static::lazy_static;

lazy_static! {
    pub static ref WRITER: Writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };
}
```

No entanto, este `WRITER` é praticamente inútil, pois é imutável. Isso significa que não podemos escrever nada nele (já que todos os métodos de escrita recebem `&mut self`). Uma solução possível seria usar um [static mutável]. Mas então cada leitura e escrita nele seria unsafe, pois poderia facilmente introduzir data races e outras coisas ruins. Usar `static mut` é altamente desencorajado. Até houve propostas para [removê-lo][remove static mut]. Mas quais são as alternativas? Poderíamos tentar usar um static imutável com um tipo de célula como [RefCell] ou até [UnsafeCell] que fornece [mutabilidade interior]. Mas esses tipos não são [Sync] \(com boa razão), então não podemos usá-los em statics.

[static mutável]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable
[remove static mut]: https://internals.rust-lang.org/t/pre-rfc-remove-static-mut/1437
[RefCell]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html#keeping-track-of-borrows-at-runtime-with-refcellt
[UnsafeCell]: https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html
[mutabilidade interior]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
[Sync]: https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html

### Spinlocks
Para obter mutabilidade interior sincronizada, usuários da biblioteca padrão podem usar [Mutex]. Ele fornece exclusão mútua bloqueando threads quando o recurso já está bloqueado. Mas nosso kernel básico não tem nenhum suporte de bloqueio ou mesmo um conceito de threads, então também não podemos usá-lo. No entanto, há um tipo realmente básico de mutex na ciência da computação que não requer nenhum recurso de sistema operacional: o [spinlock]. Em vez de bloquear, as threads simplesmente tentam bloqueá-lo novamente e novamente em um loop apertado, queimando assim tempo de CPU até que o mutex esteja livre novamente.

[Mutex]: https://doc.rust-lang.org/nightly/std/sync/struct.Mutex.html
[spinlock]: https://en.wikipedia.org/wiki/Spinlock

Para usar um spinning mutex, podemos adicionar a [crate spin] como uma dependência:

[crate spin]: https://crates.io/crates/spin

```toml
# em Cargo.toml
[dependencies]
spin = "0.5.2"
```

Então podemos usar o spinning mutex para adicionar [mutabilidade interior] segura ao nosso `WRITER` static:

```rust
// em src/vga_buffer.rs

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
Agora podemos deletar a função `print_something` e imprimir diretamente da nossa função `_start`:

```rust
// em src/main.rs
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    write!(vga_buffer::WRITER.lock(), ", some numbers: {} {}", 42, 1.337).unwrap();

    loop {}
}
```
Precisamos importar a trait `fmt::Write` para poder usar suas funções.

### Segurança
Note que temos apenas um bloco unsafe no nosso código, que é necessário para criar uma referência `Buffer` apontando para `0xb8000`. Depois disso, todas as operações são seguras. Rust usa verificação de limites para acessos a arrays por padrão, então não podemos escrever acidentalmente fora do buffer. Assim, codificamos as condições necessárias no sistema de tipos e somos capazes de fornecer uma interface segura para o exterior.

### Uma Macro println
Agora que temos um writer global, podemos adicionar uma macro `println` que pode ser usada de qualquer lugar na base de código. A [sintaxe de macro] do Rust é um pouco estranha, então não tentaremos escrever uma macro do zero. Em vez disso, olhamos para o código-fonte da [macro `println!`] na biblioteca padrão:

[sintaxe de macro]: https://doc.rust-lang.org/nightly/book/ch19-06-macros.html#declarative-macros-with-macro_rules-for-general-metaprogramming
[macro `println!`]: https://doc.rust-lang.org/nightly/std/macro.println!.html

```rust
#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}
```

Macros são definidas através de uma ou mais regras, semelhantes a braços `match`. A macro `println` tem duas regras: A primeira regra é para invocações sem argumentos, por exemplo, `println!()`, que é expandida para `print!("\n")` e, portanto, apenas imprime um newline. A segunda regra é para invocações com parâmetros como `println!("Hello")` ou `println!("Number: {}", 4)`. Ela também é expandida para uma invocação da macro `print!`, passando todos os argumentos e um newline `\n` adicional no final.

O atributo `#[macro_export]` torna a macro disponível para toda a crate (não apenas o módulo em que é definida) e crates externas. Ele também coloca a macro na raiz da crate, o que significa que temos que importar a macro através de `use std::println` em vez de `std::macros::println`.

A [macro `print!`] é definida como:

[macro `print!`]: https://doc.rust-lang.org/nightly/std/macro.print!.html

```rust
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
```

A macro se expande para uma chamada da [função `_print`] no módulo `io`. A [variável `$crate`] garante que a macro também funcione de fora da crate `std` ao se expandir para `std` quando é usada em outras crates.

A [macro `format_args`] constrói um tipo [fmt::Arguments] dos argumentos passados, que é passado para `_print`. A [função `_print`] da libstd chama `print_to`, que é bastante complicado porque suporta diferentes dispositivos `Stdout`. Não precisamos dessa complexidade, pois só queremos imprimir no buffer VGA.

[função `_print`]: https://github.com/rust-lang/rust/blob/29f5c699b11a6a148f097f82eaa05202f8799bbc/src/libstd/io/stdio.rs#L698
[variável `$crate`]: https://doc.rust-lang.org/1.30.0/book/first-edition/macros.html#the-variable-crate
[macro `format_args`]: https://doc.rust-lang.org/nightly/std/macro.format_args.html
[fmt::Arguments]: https://doc.rust-lang.org/nightly/core/fmt/struct.Arguments.html

Para imprimir no buffer VGA, apenas copiamos as macros `println!` e `print!`, mas as modificamos para usar nossa própria função `_print`:

```rust
// em src/vga_buffer.rs

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

Uma coisa que mudamos da definição original de `println` é que também prefixamos as invocações da macro `print!` com `$crate`. Isso garante que não precisamos importar a macro `print!` também se quisermos usar apenas `println`.

Como na biblioteca padrão, adicionamos o atributo `#[macro_export]` a ambas as macros para torná-las disponíveis em todo lugar na nossa crate. Note que isso coloca as macros no namespace raiz da crate, então importá-las via `use crate::vga_buffer::println` não funciona. Em vez disso, temos que fazer `use crate::println`.

A função `_print` bloqueia nosso `WRITER` static e chama o método `write_fmt` nele. Este método é da trait `Write`, que precisamos importar. O `unwrap()` adicional no final entra em panic se a impressão não for bem-sucedida. Mas como sempre retornamos `Ok` em `write_str`, isso não deve acontecer.

Como as macros precisam ser capazes de chamar `_print` de fora do módulo, a função precisa ser pública. No entanto, como consideramos isso um detalhe de implementação privado, adicionamos o [atributo `doc(hidden)`] para ocultá-la da documentação gerada.

[atributo `doc(hidden)`]: https://doc.rust-lang.org/nightly/rustdoc/write-documentation/the-doc-attribute.html#hidden

### Hello World usando `println`
Agora podemos usar `println` na nossa função `_start`:

```rust
// em src/main.rs

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    loop {}
}
```

Note que não precisamos importar a macro na função main, porque ela já vive no namespace raiz.

Como esperado, agora vemos um _"Hello World!"_ na tela:

![QEMU imprimindo "Hello World!"](vga-hello-world.png)

### Imprimindo Mensagens de Panic

Agora que temos uma macro `println`, podemos usá-la na nossa função panic para imprimir a mensagem de panic e a localização do panic:

```rust
// em main.rs

/// Esta função é chamada em caso de pânico.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
```

Quando agora inserimos `panic!("Some panic message");` na nossa função `_start`, obtemos a seguinte saída:

![QEMU imprimindo "panicked at 'Some panic message', src/main.rs:28:5](vga-panic.png)

Então sabemos não apenas que um panic ocorreu, mas também a mensagem de panic e onde no código aconteceu.

## Resumo
Neste post, aprendemos sobre a estrutura do buffer de texto VGA e como ele pode ser escrito através do mapeamento de memória no endereço `0xb8000`. Criamos um módulo Rust que encapsula a unsafety de escrever neste buffer mapeado em memória e apresenta uma interface segura e conveniente para o exterior.

Graças ao cargo, também vimos como é fácil adicionar dependências em bibliotecas de terceiros. As duas dependências que adicionamos, `lazy_static` e `spin`, são muito úteis no desenvolvimento de SO e as usaremos em mais lugares em posts futuros.

## O que vem a seguir?
O próximo post explica como configurar o framework de testes unitários embutido do Rust. Criaremos então alguns testes unitários básicos para o módulo de buffer VGA deste post.