+++
title = "Testes"
weight = 4
path = "pt-BR/testing"
date = 2019-04-27

[extra]
chapter = "Bare Bones"
comments_search_term = 1009
# Please update this when updating the translation
translation_based_on_commit = "33b7979468235b8637584e91e4c599cef37d9687"
# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

Este post explora testes unitários e de integração em executáveis `no_std`. Usaremos o suporte do Rust para frameworks de teste customizados para executar funções de teste dentro do nosso kernel. Para reportar os resultados para fora do QEMU, usaremos diferentes recursos do QEMU e da ferramenta `bootimage`.

<!-- more -->

Este blog é desenvolvido abertamente no [GitHub]. Se você tiver algum problema ou dúvida, abra um issue lá. Você também pode deixar comentários [na parte inferior]. O código-fonte completo desta publicação pode ser encontrado na branch [`post-04`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[na parte inferior]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-04

<!-- toc -->

## Requisitos

Este post substitui os posts (agora deprecados) [_Unit Testing_] e [_Integration Tests_]. Ele assume que você seguiu o post [_A Minimal Rust Kernel_] depois de 27-04-2019. Principalmente, ele requer que você tenha um arquivo `.cargo/config.toml` que [define um alvo padrão] e [define um executável runner].

[_Unit Testing_]: @/edition-2/posts/deprecated/04-unit-testing/index.md
[_Integration Tests_]: @/edition-2/posts/deprecated/05-integration-tests/index.md
[_A Minimal Rust Kernel_]: @/edition-2/posts/02-minimal-rust-kernel/index.pt-BR.md
[define um alvo padrão]: @/edition-2/posts/02-minimal-rust-kernel/index.pt-BR.md#definir-um-alvo-padrao
[define um executável runner]: @/edition-2/posts/02-minimal-rust-kernel/index.pt-BR.md#usando-cargo-run

## Testes em Rust

Rust tem um [framework de testes integrado] que é capaz de executar testes unitários sem a necessidade de configurar nada. Basta criar uma função que verifica alguns resultados através de assertions e adicionar o atributo `#[test]` ao cabeçalho da função. Então `cargo test` automaticamente encontrará e executará todas as funções de teste da sua crate.

[framework de testes integrado]: https://doc.rust-lang.org/book/ch11-00-testing.html

Para habilitar testes para nosso binário kernel, podemos definir a flag `test` no Cargo.toml como `true`:

```toml
# em Cargo.toml

[[bin]]
name = "blog_os"
test = true
bench = false
```

Esta [seção `[[bin]]`](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#configuring-a-target) especifica como o `cargo` deve compilar nosso executável `blog_os`. O campo `test` especifica se testes são suportados para este executável. Definimos `test = false` no primeiro post para [deixar o `rust-analyzer` feliz](@/edition-2/posts/01-freestanding-rust-binary/index.pt-BR.md#deixando-rust-analyzer-feliz), mas agora queremos habilitar testes, então o definimos de volta para `true`.

Infelizmente, testes são um pouco mais complicados para aplicações `no_std` como nosso kernel. O problema é que o framework de testes do Rust usa implicitamente a biblioteca [`test`] integrada, que depende da biblioteca padrão. Isso significa que não podemos usar o framework de testes padrão para nosso kernel `#[no_std]`.

[`test`]: https://doc.rust-lang.org/test/index.html

Podemos ver isso quando tentamos executar `cargo test` no nosso projeto:

```
> cargo test
   Compiling blog_os v0.1.0 (/…/blog_os)
error[E0463]: can't find crate for `test`
```

Como a crate `test` depende da biblioteca padrão, ela não está disponível para nosso alvo bare metal. Embora portar a crate `test` para um contexto `#[no_std]` [seja possível][utest], é altamente instável e requer alguns hacks, como redefinir a macro `panic`.

[utest]: https://github.com/japaric/utest

### Frameworks de Teste Customizados

Felizmente, Rust suporta substituir o framework de testes padrão através do recurso instável [`custom_test_frameworks`]. Este recurso não requer bibliotecas externas e, portanto, também funciona em ambientes `#[no_std]`. Funciona coletando todas as funções anotadas com um atributo `#[test_case]` e então invocando uma função runner especificada pelo usuário com a lista de testes como argumento. Assim, dá à implementação controle máximo sobre o processo de teste.

[`custom_test_frameworks`]: https://doc.rust-lang.org/unstable-book/language-features/custom-test-frameworks.html

A desvantagem comparada ao framework de testes padrão é que muitos recursos avançados, como [testes `should_panic`], não estão disponíveis. Em vez disso, cabe à implementação fornecer tais recursos ela mesma se necessário. Isso é ideal para nós, pois temos um ambiente de execução muito especial onde as implementações padrão de tais recursos avançados provavelmente não funcionariam de qualquer forma. Por exemplo, o atributo `#[should_panic]` depende de stack unwinding para capturar os panics, que desabilitamos para nosso kernel.

[testes `should_panic`]: https://doc.rust-lang.org/book/ch11-01-writing-tests.html#checking-for-panics-with-should_panic

Para implementar um framework de testes customizado para nosso kernel, adicionamos o seguinte ao nosso `main.rs`:

```rust
// em src/main.rs

#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}
```

Nosso runner apenas imprime uma breve mensagem de debug e então chama cada função de teste na lista. O tipo de argumento `&[&dyn Fn()]` é uma [_slice_] de referências a [_trait object_] da trait [_Fn()_]. É basicamente uma lista de referências a tipos que podem ser chamados como uma função. Como a função é inútil para execuções não-teste, usamos o atributo `#[cfg(test)]` para incluí-la apenas para testes.

[_slice_]: https://doc.rust-lang.org/std/primitive.slice.html
[_trait object_]: https://doc.rust-lang.org/1.30.0/book/first-edition/trait-objects.html
[_Fn()_]: https://doc.rust-lang.org/std/ops/trait.Fn.html

Quando executamos `cargo test` agora, vemos que ele agora é bem-sucedido (se não for, veja a nota abaixo). No entanto, ainda vemos nosso "Hello World" em vez da mensagem do nosso `test_runner`. A razão é que nossa função `_start` ainda é usada como ponto de entrada. O recurso de frameworks de teste customizados gera uma função `main` que chama `test_runner`, mas esta função é ignorada porque usamos o atributo `#[no_main]` e fornecemos nosso próprio ponto de entrada.

<div class = "warning">

**Nota:** Atualmente há um bug no cargo que leva a erros de "duplicate lang item" no `cargo test` em alguns casos. Ocorre quando você definiu `panic = "abort"` para um profile no seu `Cargo.toml`. Tente removê-lo, então `cargo test` deve funcionar. Alternativamente, se isso não funcionar, então adicione `panic-abort-tests = true` à seção `[unstable]` do seu arquivo `.cargo/config.toml`. Veja o [issue do cargo](https://github.com/rust-lang/cargo/issues/7359) para mais informações sobre isso.

</div>

Para corrigir isso, primeiro precisamos mudar o nome da função gerada para algo diferente de `main` através do atributo `reexport_test_harness_main`. Então podemos chamar a função renomeada da nossa função `_start`:

```rust
// em src/main.rs

#![reexport_test_harness_main = "test_main"]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}
```

Definimos o nome da função de entrada do framework de testes como `test_main` e a chamamos do nosso ponto de entrada `_start`. Usamos [compilação condicional] para adicionar a chamada a `test_main` apenas em contextos de teste porque a função não é gerada em uma execução normal.

Quando agora executamos `cargo test`, vemos a mensagem "Running 0 tests" do nosso `test_runner` na tela. Agora estamos prontos para criar nossa primeira função de teste:

```rust
// em src/main.rs

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}
```

Quando executamos `cargo test` agora, vemos a seguinte saída:

![QEMU imprimindo "Hello World!", "Running 1 tests" e "trivial assertion... [ok]"](qemu-test-runner-output.png)

A slice `tests` passada para nossa função `test_runner` agora contém uma referência à função `trivial_assertion`. Da saída `trivial assertion... [ok]` na tela, vemos que o teste foi chamado e que foi bem-sucedido.

Após executar os testes, nosso `test_runner` retorna à função `test_main`, que por sua vez retorna à nossa função de ponto de entrada `_start`. No final de `_start`, entramos em um loop infinito porque a função de ponto de entrada não tem permissão para retornar. Isso é um problema, porque queremos que `cargo test` saia após executar todos os testes.

## Saindo do QEMU

Agora, temos um loop infinito no final da nossa função `_start` e precisamos fechar o QEMU manualmente em cada execução de `cargo test`. Isso é infeliz porque também queremos executar `cargo test` em scripts sem interação do usuário. A solução limpa para isso seria implementar uma maneira adequada de desligar nosso SO. Infelizmente, isso é relativamente complexo porque requer implementar suporte para o padrão de gerenciamento de energia [APM] ou [ACPI].

[APM]: https://wiki.osdev.org/APM
[ACPI]: https://wiki.osdev.org/ACPI

Felizmente, há uma saída de emergência: O QEMU suporta um dispositivo especial `isa-debug-exit`, que fornece uma maneira fácil de sair do QEMU do sistema guest. Para habilitá-lo, precisamos passar um argumento `-device` ao QEMU. Podemos fazer isso adicionando uma chave de configuração `package.metadata.bootimage.test-args` no nosso `Cargo.toml`:

```toml
# em Cargo.toml

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
```

O `bootimage runner` anexa os `test-args` ao comando QEMU padrão para todos os executáveis de teste. Para um `cargo run` normal, os argumentos são ignorados.

Junto com o nome do dispositivo (`isa-debug-exit`), passamos os dois parâmetros `iobase` e `iosize` que especificam a _porta I/O_ através da qual o dispositivo pode ser alcançado do nosso kernel.

### Portas I/O

Existem duas abordagens diferentes para comunicação entre a CPU e hardware periférico no x86, **I/O mapeado em memória** e **I/O mapeado em porta**. Já usamos I/O mapeado em memória para acessar o [buffer de texto VGA] através do endereço de memória `0xb8000`. Este endereço não é mapeado para RAM, mas para alguma memória no dispositivo VGA.

[buffer de texto VGA]: @/edition-2/posts/03-vga-text-buffer/index.pt-BR.md

Em contraste, I/O mapeado em porta usa um barramento I/O separado para comunicação. Cada periférico conectado tem um ou mais números de porta. Para comunicar com tal porta I/O, existem instruções especiais de CPU chamadas `in` e `out`, que recebem um número de porta e um byte de dados (também há variações desses comandos que permitem enviar um `u16` ou `u32`).

O dispositivo `isa-debug-exit` usa I/O mapeado em porta. O parâmetro `iobase` especifica em qual endereço de porta o dispositivo deve viver (`0xf4` é uma porta [geralmente não utilizada][lista de portas I/O x86] no barramento IO do x86) e o `iosize` especifica o tamanho da porta (`0x04` significa quatro bytes).

[lista de portas I/O x86]: https://wiki.osdev.org/I/O_Ports#The_list

### Usando o Dispositivo de Saída

A funcionalidade do dispositivo `isa-debug-exit` é muito simples. Quando um `value` é escrito na porta I/O especificada por `iobase`, ele faz com que o QEMU saia com [status de saída] `(value << 1) | 1`. Então, quando escrevemos `0` na porta, o QEMU sairá com status de saída `(0 << 1) | 1 = 1`, e quando escrevemos `1` na porta, ele sairá com status de saída `(1 << 1) | 1 = 3`.

[status de saída]: https://en.wikipedia.org/wiki/Exit_status

Em vez de invocar manualmente as instruções assembly `in` e `out`, usamos as abstrações fornecidas pela crate [`x86_64`]. Para adicionar uma dependência nessa crate, a adicionamos à seção `dependencies` no nosso `Cargo.toml`:

[`x86_64`]: https://docs.rs/x86_64/0.14.2/x86_64/

```toml
# em Cargo.toml

[dependencies]
x86_64 = "0.14.2"
```

Agora podemos usar o tipo [`Port`] fornecido pela crate para criar uma função `exit_qemu`:

[`Port`]: https://docs.rs/x86_64/0.14.2/x86_64/instructions/port/struct.Port.html

```rust
// em src/main.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

A função cria um novo [`Port`] em `0xf4`, que é o `iobase` do dispositivo `isa-debug-exit`. Então ela escreve o código de saída passado para a porta. Usamos `u32` porque especificamos o `iosize` do dispositivo `isa-debug-exit` como 4 bytes. Ambas as operações são unsafe porque escrever em uma porta I/O geralmente pode resultar em comportamento arbitrário.

Para especificar o status de saída, criamos um enum `QemuExitCode`. A ideia é sair com o código de saída de sucesso se todos os testes foram bem-sucedidos e com o código de saída de falha caso contrário. O enum é marcado como `#[repr(u32)]` para representar cada variante por um inteiro `u32`. Usamos o código de saída `0x10` para sucesso e `0x11` para falha. Os códigos de saída reais não importam muito, desde que não colidam com os códigos de saída padrão do QEMU. Por exemplo, usar código de saída `0` para sucesso não é uma boa ideia porque ele se torna `(0 << 1) | 1 = 1` após a transformação, que é o código de saída padrão quando o QEMU falha ao executar. Então não poderíamos diferenciar um erro do QEMU de uma execução de teste bem-sucedida.

Agora podemos atualizar nosso `test_runner` para sair do QEMU após todos os testes terem sido executados:

```rust
// em src/main.rs

fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    /// novo
    exit_qemu(QemuExitCode::Success);
}
```

Quando executamos `cargo test` agora, vemos que o QEMU fecha imediatamente após executar os testes. O problema é que `cargo test` interpreta o teste como falhado mesmo que tenhamos passado nosso código de saída `Success`:

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.03s
     Running target/x86_64-blog_os/debug/deps/blog_os-5804fc7d2dd4c9be
Building bootloader
   Compiling bootloader v0.5.3 (/home/philipp/Documents/bootloader)
    Finished release [optimized + debuginfo] target(s) in 1.07s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-5804fc7d2dd4c9be.bin -device isa-debug-exit,iobase=0xf4,
    iosize=0x04`
error: test failed, to rerun pass '--bin blog_os'
```

O problema é que `cargo test` considera todos os códigos de erro diferentes de `0` como falha.

### Código de Saída de Sucesso

Para contornar isso, `bootimage` fornece uma chave de configuração `test-success-exit-code` que mapeia um código de saída especificado para o código de saída `0`:

```toml
# em Cargo.toml

[package.metadata.bootimage]
test-args = […]
test-success-exit-code = 33         # (0x10 << 1) | 1
```

Com esta configuração, `bootimage` mapeia nosso código de saída de sucesso para o código de saída 0, para que `cargo test` reconheça corretamente o caso de sucesso e não conte o teste como falhado.

Nosso test runner agora fecha automaticamente o QEMU e reporta corretamente os resultados do teste. Ainda vemos a janela do QEMU abrir por um tempo muito curto, mas não é suficiente para ler os resultados. Seria bom se pudéssemos imprimir os resultados do teste no console em vez disso, para que ainda possamos vê-los após o QEMU sair.

## Imprimindo no Console

Para ver a saída do teste no console, precisamos enviar os dados do nosso kernel para o sistema host de alguma forma. Existem várias maneiras de conseguir isso, por exemplo, enviando os dados por uma interface de rede TCP. No entanto, configurar uma pilha de rede é uma tarefa bastante complexa, então escolheremos uma solução mais simples em vez disso.

### Porta Serial

Uma maneira simples de enviar os dados é usar a [porta serial], um antigo padrão de interface que não é mais encontrado em computadores modernos. É fácil de programar e o QEMU pode redirecionar os bytes enviados pela porta serial para a saída padrão do host ou um arquivo.

[porta serial]: https://en.wikipedia.org/wiki/Serial_port

Os chips que implementam uma interface serial são chamados [UARTs]. Existem [muitos modelos de UART] no x86, mas felizmente as únicas diferenças entre eles são alguns recursos avançados que não precisamos. Os UARTs comuns hoje são todos compatíveis com o [UART 16550], então usaremos esse modelo para nosso framework de testes.

[UARTs]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter
[muitos modelos de UART]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter#Models
[UART 16550]: https://en.wikipedia.org/wiki/16550_UART

Usaremos a crate [`uart_16550`] para inicializar o UART e enviar dados pela porta serial. Para adicioná-la como dependência, atualizamos nosso `Cargo.toml` e `main.rs`:

[`uart_16550`]: https://docs.rs/uart_16550

```toml
# em Cargo.toml

[dependencies]
uart_16550 = "0.2.0"
```

A crate `uart_16550` contém uma struct `SerialPort` que representa os registradores UART, mas ainda precisamos construir uma instância dela nós mesmos. Para isso, criamos um novo módulo `serial` com o seguinte conteúdo:

```rust
// em src/main.rs

mod serial;
```

```rust
// em src/serial.rs

use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}
```

Como com o [buffer de texto VGA][vga lazy-static], usamos `lazy_static` e um spinlock para criar uma instância writer `static`. Ao usar `lazy_static` podemos garantir que o método `init` seja chamado exatamente uma vez em seu primeiro uso.

Como o dispositivo `isa-debug-exit`, o UART é programado usando I/O de porta. Como o UART é mais complexo, ele usa múltiplas portas I/O para programar diferentes registradores do dispositivo. A função unsafe `SerialPort::new` espera o endereço da primeira porta I/O do UART como argumento, a partir do qual ela pode calcular os endereços de todas as portas necessárias. Estamos passando o endereço de porta `0x3F8`, que é o número de porta padrão para a primeira interface serial.

[vga lazy-static]: @/edition-2/posts/03-vga-text-buffer/index.pt-BR.md#lazy-statics

Para tornar a porta serial facilmente utilizável, adicionamos macros `serial_print!` e `serial_println!`:

```rust
// em src/serial.rs

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
}

/// Imprime no host através da interface serial.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Imprime no host através da interface serial, anexando uma newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
```

A implementação é muito similar à implementação das nossas macros `print` e `println`. Como o tipo `SerialPort` já implementa a trait [`fmt::Write`], não precisamos fornecer nossa própria implementação.

[`fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

Agora podemos imprimir na interface serial em vez do buffer de texto VGA no nosso código de teste:

```rust
// em src/main.rs

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    […]
}

#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

Note que a macro `serial_println` vive diretamente sob o namespace raiz porque usamos o atributo `#[macro_export]`, então importá-la através de `use crate::serial::serial_println` não funcionará.

### Argumentos do QEMU

Para ver a saída serial do QEMU, precisamos usar o argumento `-serial` para redirecionar a saída para stdout:

```toml
# em Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio"
]
```

Quando executamos `cargo test` agora, vemos a saída do teste diretamente no console:

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [ok]
```

No entanto, quando um teste falha, ainda vemos a saída dentro do QEMU porque nosso handler de panic ainda usa `println`. Para simular isso, podemos mudar a assertion no nosso teste `trivial_assertion` para `assert_eq!(0, 1)`:

![QEMU imprimindo "Hello World!" e "panicked at 'assertion failed: `(left == right)`
    left: `0`, right: `1`', src/main.rs:55:5](qemu-failed-test.png)

Vemos que a mensagem de panic ainda é impressa no buffer VGA, enquanto a outra saída de teste é impressa na porta serial. A mensagem de panic é bastante útil, então seria útil vê-la no console também.

### Imprimir uma Mensagem de Erro no Panic

Para sair do QEMU com uma mensagem de erro em um panic, podemos usar [compilação condicional] para usar um handler de panic diferente no modo de teste:

[compilação condicional]: https://doc.rust-lang.org/1.30.0/book/first-edition/conditional-compilation.html

```rust
// em src/main.rs

// nosso handler de panic existente
#[cfg(not(test))] // novo atributo
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

// nosso handler de panic em modo de teste
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}
```

Para nosso handler de panic de teste, usamos `serial_println` em vez de `println` e então saímos do QEMU com um código de saída de falha. Note que ainda precisamos de um `loop` infinito após a chamada `exit_qemu` porque o compilador não sabe que o dispositivo `isa-debug-exit` causa uma saída do programa.

Agora o QEMU também sai para testes falhados e imprime uma mensagem de erro útil no console:

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [failed]

Error: panicked at 'assertion failed: `(left == right)`
  left: `0`,
 right: `1`', src/main.rs:65:5
```

Como agora vemos toda a saída do teste no console, não precisamos mais da janela do QEMU que aparece por um curto tempo. Então podemos ocultá-la completamente.

### Ocultando o QEMU

Como reportamos os resultados completos do teste usando o dispositivo `isa-debug-exit` e a porta serial, não precisamos mais da janela do QEMU. Podemos ocultá-la passando o argumento `-display none` ao QEMU:

```toml
# em Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio",
    "-display", "none"
]
```

Agora o QEMU executa completamente em segundo plano e nenhuma janela é mais aberta. Isso não é apenas menos irritante, mas também permite que nosso framework de testes execute em ambientes sem interface gráfica do usuário, como serviços de CI ou conexões [SSH].

[SSH]: https://en.wikipedia.org/wiki/Secure_Shell

### Timeouts

Como `cargo test` espera até que o test runner saia, um teste que nunca retorna pode bloquear o test runner para sempre. Isso é infeliz, mas não é um grande problema na prática, pois geralmente é fácil evitar loops infinitos. No nosso caso, no entanto, loops infinitos podem ocorrer em várias situações:

- O bootloader falha ao carregar nosso kernel, o que causa o sistema reiniciar infinitamente.
- O firmware BIOS/UEFI falha ao carregar o bootloader, o que causa a mesma reinicialização infinita.
- A CPU entra em uma declaração `loop {}` no final de algumas das nossas funções, por exemplo porque o dispositivo de saída do QEMU não funciona corretamente.
- O hardware causa um reset do sistema, por exemplo quando uma exceção de CPU não é capturada (explicado em um post futuro).

Como loops infinitos podem ocorrer em tantas situações, a ferramenta `bootimage` define um timeout de 5 minutos para cada executável de teste por padrão. Se o teste não terminar dentro deste tempo, ele é marcado como falhado e um erro "Timed Out" é impresso no console. Este recurso garante que testes que estão presos em um loop infinito não bloqueiem `cargo test` para sempre.

Você pode tentar você mesmo adicionando uma declaração `loop {}` no teste `trivial_assertion`. Quando você executa `cargo test`, vê que o teste é marcado como timed out após 5 minutos. A duração do timeout é [configurável][bootimage config] através de uma chave `test-timeout` no Cargo.toml:

[bootimage config]: https://github.com/rust-osdev/bootimage#configuration

```toml
# em Cargo.toml

[package.metadata.bootimage]
test-timeout = 300          # (em segundos)
```

Se você não quiser esperar 5 minutos para o teste `trivial_assertion` dar timeout, pode diminuir temporariamente o valor acima.

### Inserir Impressão Automaticamente

Nosso teste `trivial_assertion` atualmente precisa imprimir suas próprias informações de status usando `serial_print!`/`serial_println!`:

```rust
#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

Adicionar manualmente essas declarações de impressão para cada teste que escrevemos é trabalhoso, então vamos atualizar nosso `test_runner` para imprimir essas mensagens automaticamente. Para fazer isso, precisamos criar uma nova trait `Testable`:

```rust
// em src/main.rs

pub trait Testable {
    fn run(&self) -> ();
}
```

O truque agora é implementar esta trait para todos os tipos `T` que implementam a [trait `Fn()`]:

[trait `Fn()`]: https://doc.rust-lang.org/stable/core/ops/trait.Fn.html

```rust
// em src/main.rs

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}
```

Implementamos a função `run` primeiro imprimindo o nome da função usando a função [`any::type_name`]. Esta função é implementada diretamente no compilador e retorna uma descrição em string de cada tipo. Para funções, o tipo é seu nome, então isso é exatamente o que queremos neste caso. O caractere `\t` é o [caractere tab], que adiciona algum alinhamento às mensagens `[ok]`.

[`any::type_name`]: https://doc.rust-lang.org/stable/core/any/fn.type_name.html
[caractere tab]: https://en.wikipedia.org/wiki/Tab_key#Tab_characters

Após imprimir o nome da função, invocamos a função de teste através de `self()`. Isso só funciona porque exigimos que `self` implemente a trait `Fn()`. Após a função de teste retornar, imprimimos `[ok]` para indicar que a função não entrou em panic.

O último passo é atualizar nosso `test_runner` para usar a nova trait `Testable`:

```rust
// em src/main.rs

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) { // novo
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run(); // novo
    }
    exit_qemu(QemuExitCode::Success);
}
```

As únicas duas mudanças são o tipo do argumento `tests` de `&[&dyn Fn()]` para `&[&dyn Testable]` e o fato de que agora chamamos `test.run()` em vez de `test()`.

Agora podemos remover as declarações de impressão do nosso teste `trivial_assertion` já que elas são impressas automaticamente:

```rust
// em src/main.rs

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
```

A saída de `cargo test` agora se parece com isto:

```
Running 1 tests
blog_os::trivial_assertion...	[ok]
```

O nome da função agora inclui o caminho completo para a função, o que é útil quando funções de teste em diferentes módulos têm o mesmo nome. Caso contrário, a saída parece igual a antes, mas não precisamos mais adicionar declarações de impressão aos nossos testes manualmente.

## Testando o Buffer VGA

Agora que temos um framework de testes funcionando, podemos criar alguns testes para nossa implementação de buffer VGA. Primeiro, criamos um teste muito simples para verificar que `println` funciona sem entrar em panic:

```rust
// em src/vga_buffer.rs

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}
```

O teste apenas imprime algo no buffer VGA. Se ele terminar sem entrar em panic, significa que a invocação de `println` também não entrou em panic.

Para garantir que nenhum panic ocorra mesmo se muitas linhas forem impressas e as linhas forem deslocadas para fora da tela, podemos criar outro teste:

```rust
// em src/vga_buffer.rs

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}
```

Também podemos criar uma função de teste para verificar que as linhas impressas realmente aparecem na tela:

```rust
// em src/vga_buffer.rs

#[test_case]
fn test_println_output() {
    let s = "Some test string that fits on a single line";
    println!("{}", s);
    for (i, c) in s.chars().enumerate() {
        let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
        assert_eq!(char::from(screen_char.ascii_character), c);
    }
}
```

A função define uma string de teste, a imprime usando `println`, e então itera sobre os caracteres da tela do `WRITER` static, que representa o buffer de texto VGA. Como `println` imprime na última linha da tela e então anexa imediatamente uma newline, a string deve aparecer na linha `BUFFER_HEIGHT - 2`.

Ao usar [`enumerate`], contamos o número de iterações na variável `i`, que então usamos para carregar o caractere da tela correspondente a `c`. Ao comparar o `ascii_character` do caractere da tela com `c`, garantimos que cada caractere da string realmente aparece no buffer de texto VGA.

[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate

Como você pode imaginar, poderíamos criar muitas mais funções de teste. Por exemplo, uma função que testa que nenhum panic ocorre ao imprimir linhas muito longas e que elas são quebradas corretamente, ou uma função para testar que newlines, caracteres não imprimíveis e caracteres não-unicode são tratados corretamente.

Para o resto deste post, no entanto, explicaremos como criar _testes de integração_ para testar a interação de diferentes componentes juntos.

## Testes de Integração

A convenção para [testes de integração] em Rust é colocá-los em um diretório `tests` na raiz do projeto (ou seja, ao lado do diretório `src`). Tanto o framework de testes padrão quanto frameworks de testes customizados detectarão e executarão automaticamente todos os testes naquele diretório.

[testes de integração]: https://doc.rust-lang.org/book/ch11-03-test-organization.html#integration-tests

Todos os testes de integração são seus próprios executáveis e completamente separados do nosso `main.rs`. Isso significa que cada teste precisa definir sua própria função de ponto de entrada. Vamos criar um teste de integração de exemplo chamado `basic_boot` para ver como funciona em detalhes:

```rust
// em tests/basic_boot.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

#[unsafe(no_mangle)] // não altere (mangle) o nome desta função
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

fn test_runner(tests: &[&dyn Fn()]) {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
```

Como testes de integração são executáveis separados, precisamos fornecer todos os atributos da crate (`no_std`, `no_main`, `test_runner`, etc.) novamente. Também precisamos criar uma nova função de ponto de entrada `_start`, que chama a função de ponto de entrada de teste `test_main`. Não precisamos de nenhum atributo `cfg(test)` porque executáveis de teste de integração nunca são construídos em modo não-teste.

Usamos a macro [`unimplemented`] que sempre entra em panic como placeholder para a função `test_runner` e apenas fazemos `loop` no handler de `panic` por enquanto. Idealmente, queremos implementar essas funções exatamente como fizemos no nosso `main.rs` usando a macro `serial_println` e a função `exit_qemu`. O problema é que não temos acesso a essas funções porque os testes são construídos completamente separados do nosso executável `main.rs`.

[`unimplemented`]: https://doc.rust-lang.org/core/macro.unimplemented.html

Se você executar `cargo test` neste estágio, entrará em um loop infinito porque o handler de panic faz loop infinitamente. Você precisa usar o atalho de teclado `ctrl+c` para sair do QEMU.

### Criar uma Biblioteca

Para tornar as funções necessárias disponíveis para nosso teste de integração, precisamos separar uma biblioteca do nosso `main.rs`, que pode ser incluída por outras crates e executáveis de teste de integração. Para fazer isso, criamos um novo arquivo `src/lib.rs`:

```rust
// src/lib.rs

#![no_std]

```

Como o `main.rs`, o `lib.rs` é um arquivo especial que é automaticamente reconhecido pelo cargo. A biblioteca é uma unidade de compilação separada, então precisamos especificar o atributo `#![no_std]` novamente.

Para fazer nossa biblioteca funcionar com `cargo test`, precisamos também mover as funções de teste e atributos de `main.rs` para `lib.rs`:

```rust
// em src/lib.rs

#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

/// Ponto de entrada para `cargo test`
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    test_main();
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
```

Para tornar nosso `test_runner` disponível para executáveis e testes de integração, o tornamos público e não aplicamos o atributo `cfg(test)` a ele. Também fatoramos a implementação do nosso handler de panic em uma função pública `test_panic_handler`, para que ela esteja disponível para executáveis também.

Como nosso `lib.rs` é testado independentemente do nosso `main.rs`, precisamos adicionar um ponto de entrada `_start` e um handler de panic quando a biblioteca é compilada em modo de teste. Ao usar o atributo de crate [`cfg_attr`], habilitamos condicionalmente o atributo `no_main` neste caso.

[`cfg_attr`]: https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute

Também movemos o enum `QemuExitCode` e a função `exit_qemu` e os tornamos públicos:

```rust
// em src/lib.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

Agora executáveis e testes de integração podem importar essas funções da biblioteca e não precisam definir suas próprias implementações. Para também tornar `println` e `serial_println` disponíveis, movemos as declarações de módulo também:

```rust
// em src/lib.rs

pub mod serial;
pub mod vga_buffer;
```

Tornamos os módulos públicos para torná-los utilizáveis fora da nossa biblioteca. Isso também é necessário para tornar nossas macros `println` e `serial_println` utilizáveis, já que elas usam as funções `_print` dos módulos.

Agora podemos atualizar nosso `main.rs` para usar a biblioteca:

```rust
// em src/main.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use blog_os::println;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}

/// Esta função é chamada em caso de pânico.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

A biblioteca é utilizável como uma crate externa normal. É chamada `blog_os`, como nossa crate. O código acima usa a função `blog_os::test_runner` no atributo `test_runner` e a função `blog_os::test_panic_handler` no nosso handler de `panic` `cfg(test)`. Também importa a macro `println` para torná-la disponível para nossas funções `_start` e `panic`.

Neste ponto, `cargo run` e `cargo test` devem funcionar novamente. É claro que `cargo test` ainda faz loop infinitamente (você pode sair com `ctrl+c`). Vamos corrigir isso usando as funções necessárias da biblioteca no nosso teste de integração.

### Completando o Teste de Integração

Como nosso `src/main.rs`, nosso executável `tests/basic_boot.rs` pode importar tipos da nossa nova biblioteca. Isso nos permite importar os componentes faltantes para completar nosso teste:

```rust
// em tests/basic_boot.rs

#![test_runner(blog_os::test_runner)]

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

Em vez de reimplementar o test runner, usamos a função `test_runner` da nossa biblioteca mudando o atributo `#![test_runner(crate::test_runner)]` para `#![test_runner(blog_os::test_runner)]`. Então não precisamos mais da função stub `test_runner` em `basic_boot.rs`, então podemos removê-la. Para nosso handler de `panic`, chamamos a função `blog_os::test_panic_handler` como fizemos no nosso `main.rs`.

Agora `cargo test` sai normalmente novamente. Quando você o executa, verá que ele constrói e executa os testes para nosso `lib.rs`, `main.rs` e `basic_boot.rs` separadamente um após o outro. Para o `main.rs` e os testes de integração `basic_boot`, ele reporta "Running 0 tests" já que esses arquivos não têm nenhuma função anotada com `#[test_case]`.

Agora podemos adicionar testes ao nosso `basic_boot.rs`. Por exemplo, podemos testar que `println` funciona sem entrar em panic, como fizemos nos testes do buffer VGA:

```rust
// em tests/basic_boot.rs

use blog_os::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}
```

Quando executamos `cargo test` agora, vemos que ele encontra e executa a função de teste.

O teste pode parecer um pouco inútil agora já que é quase idêntico a um dos testes do buffer VGA. No entanto, no futuro, as funções `_start` do nosso `main.rs` e `lib.rs` podem crescer e chamar várias rotinas de inicialização antes de executar a função `test_main`, então os dois testes são executados em ambientes muito diferentes.

Ao testar `println` em um ambiente `basic_boot` sem chamar nenhuma rotina de inicialização em `_start`, podemos garantir que `println` funciona logo após o boot. Isso é importante porque dependemos dele, por exemplo, para imprimir mensagens de panic.

### Testes Futuros

O poder dos testes de integração é que eles são tratados como executáveis completamente separados. Isso lhes dá controle completo sobre o ambiente, o que torna possível testar que o código interage corretamente com a CPU ou dispositivos de hardware.

Nosso teste `basic_boot` é um exemplo muito simples de um teste de integração. No futuro, nosso kernel se tornará muito mais cheio de recursos e interagirá com o hardware de várias maneiras. Ao adicionar testes de integração, podemos garantir que essas interações funcionem (e continuem funcionando) como esperado. Algumas ideias para possíveis testes futuros são:

- **Exceções de CPU**: Quando o código executa operações inválidas (por exemplo, divide por zero), a CPU lança uma exceção. O kernel pode registrar funções handler para tais exceções. Um teste de integração poderia verificar que o handler de exceção correto é chamado quando uma exceção de CPU ocorre ou que a execução continua corretamente após uma exceção resolvível.
- **Tabelas de Página**: Tabelas de página definem quais regiões de memória são válidas e acessíveis. Ao modificar as tabelas de página, é possível alocar novas regiões de memória, por exemplo ao lançar programas. Um teste de integração poderia modificar as tabelas de página na função `_start` e verificar que as modificações têm os efeitos desejados nas funções `#[test_case]`.
- **Programas Userspace**: Programas userspace são programas com acesso limitado aos recursos do sistema. Por exemplo, eles não têm acesso a estruturas de dados do kernel ou à memória de outros programas. Um teste de integração poderia lançar programas userspace que executam operações proibidas e verificar que o kernel as impede todas.

Como você pode imaginar, muitos mais testes são possíveis. Ao adicionar tais testes, podemos garantir que não os quebramos acidentalmente quando adicionamos novos recursos ao nosso kernel ou refatoramos nosso código. Isso é especialmente importante quando nosso kernel se torna maior e mais complexo.

### Testes que Devem Entrar em Panic

O framework de testes da biblioteca padrão suporta um [atributo `#[should_panic]`][should_panic] que permite construir testes que devem falhar. Isso é útil, por exemplo, para verificar que uma função falha quando um argumento inválido é passado. Infelizmente, este atributo não é suportado em crates `#[no_std]` porque requer suporte da biblioteca padrão.

[should_panic]: https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html#testing-panics

Embora não possamos usar o atributo `#[should_panic]` no nosso kernel, podemos obter comportamento similar criando um teste de integração que sai com um código de erro de sucesso do handler de panic. Vamos começar a criar tal teste com o nome `should_panic`:

```rust
// em tests/should_panic.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use blog_os::{QemuExitCode, exit_qemu, serial_println};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

Este teste ainda está incompleto, pois não define uma função `_start` ou nenhum dos atributos customizados de test runner ainda. Vamos adicionar as partes faltantes:

```rust
// em tests/should_panic.rs

#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test();
        serial_println!("[test did not panic]");
        exit_qemu(QemuExitCode::Failed);
    }
    exit_qemu(QemuExitCode::Success);
}
```

Em vez de reutilizar o `test_runner` do nosso `lib.rs`, o teste define sua própria função `test_runner` que sai com um código de saída de falha quando um teste retorna sem entrar em panic (queremos que nossos testes entrem em panic). Se nenhuma função de teste for definida, o runner sai com um código de erro de sucesso. Como o runner sempre sai após executar um único teste, não faz sentido definir mais de uma função `#[test_case]`.

Agora podemos criar um teste que deveria falhar:

```rust
// em tests/should_panic.rs

use blog_os::serial_print;

#[test_case]
fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}
```

O teste usa `assert_eq` para afirmar que `0` e `1` são iguais. É claro que isso falha, então nosso teste entra em panic como desejado. Note que precisamos imprimir manualmente o nome da função usando `serial_print!` aqui porque não usamos a trait `Testable`.

Quando executamos o teste através de `cargo test --test should_panic` vemos que ele é bem-sucedido porque o teste entrou em panic como esperado. Quando comentamos a assertion e executamos o teste novamente, vemos que ele de fato falha com a mensagem _"test did not panic"_.

Uma desvantagem significativa desta abordagem é que ela só funciona para uma única função de teste. Com múltiplas funções `#[test_case]`, apenas a primeira função é executada porque a execução não pode continuar após o handler de panic ter sido chamado. Atualmente não conheço uma boa maneira de resolver este problema, então me avise se você tiver uma ideia!

### Testes Sem Harness

Para testes de integração que têm apenas uma única função de teste (como nosso teste `should_panic`), o test runner realmente não é necessário. Para casos como este, podemos desabilitar o test runner completamente e executar nosso teste diretamente na função `_start`.

A chave para isso é desabilitar a flag `harness` para o teste no `Cargo.toml`, que define se um test runner é usado para um teste de integração. Quando está definido como `false`, tanto o test runner padrão quanto o recurso de test runner customizado são desabilitados, de modo que o teste é tratado como um executável normal.

Vamos desabilitar a flag `harness` para nosso teste `should_panic`:

```toml
# em Cargo.toml

[[test]]
name = "should_panic"
harness = false
```

Agora simplificamos vastamente nosso teste `should_panic` removendo o código relacionado ao `test_runner`. O resultado se parece com isto:

```rust
// em tests/should_panic.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    loop{}
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

Agora chamamos a função `should_fail` diretamente da nossa função `_start` e saímos com um código de saída de falha se ela retornar. Quando executamos `cargo test --test should_panic` agora, vemos que o teste se comporta exatamente como antes.

Além de criar testes `should_panic`, desabilitar o atributo `harness` também pode ser útil para testes de integração complexos, por exemplo, quando as funções de teste individuais têm efeitos colaterais e precisam ser executadas em uma ordem especificada.

## Resumo

Testes são uma técnica muito útil para garantir que certos componentes tenham o comportamento desejado. Mesmo que não possam mostrar a ausência de bugs, ainda são uma ferramenta útil para encontrá-los e especialmente para evitar regressões.

Este post explicou como configurar um framework de testes para nosso kernel Rust. Usamos o recurso de frameworks de teste customizados do Rust para implementar suporte para um atributo `#[test_case]` simples no nosso ambiente bare metal. Usando o dispositivo `isa-debug-exit` do QEMU, nosso test runner pode sair do QEMU após executar os testes e reportar o status do teste. Para imprimir mensagens de erro no console em vez do buffer VGA, criamos um driver básico para a porta serial.

Após criar alguns testes para nossa macro `println`, exploramos testes de integração na segunda metade do post. Aprendemos que eles vivem no diretório `tests` e são tratados como executáveis completamente separados. Para dar a eles acesso à função `exit_qemu` e à macro `serial_println`, movemos a maior parte do nosso código para uma biblioteca que pode ser importada por todos os executáveis e testes de integração. Como testes de integração são executados em seu próprio ambiente separado, eles tornam possível testar interações com o hardware ou criar testes que devem entrar em panic.

Agora temos um framework de testes que executa em um ambiente realista dentro do QEMU. Ao criar mais testes em posts futuros, podemos manter nosso kernel sustentável quando ele se tornar mais complexo.

## O que vem a seguir?

No próximo post, exploraremos _exceções de CPU_. Essas exceções são lançadas pela CPU quando algo ilegal acontece, como uma divisão por zero ou um acesso a uma página de memória não mapeada (um chamado "page fault"). Ser capaz de capturar e examinar essas exceções é muito importante para depuração de erros futuros. O tratamento de exceções também é muito similar ao tratamento de interrupções de hardware, que é necessário para suporte a teclado.