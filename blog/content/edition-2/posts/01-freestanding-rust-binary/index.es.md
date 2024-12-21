+++
title = "Un Binario Rust Autónomo"
weight = 1
path = "freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
+++

El primer paso para crear nuestro propio kernel de sistema operativo es crear un ejecutable en Rust que no enlace con la biblioteca estándar. Esto hace posible ejecutar código Rust directamente en el [bare metal] sin un sistema operativo subyacente.

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

Este blog se desarrolla abiertamente en [GitHub]. Si tienes algún problema o pregunta, por favor abre un issue allí. También puedes dejar comentarios [al final]. El código fuente completo para esta publicación se encuentra en la rama [`post-01`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[al final]: #comments
<!-- solución para el verificador de anclajes de zola (el objetivo está en la plantilla): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## Introducción
Para escribir un kernel de sistema operativo, necesitamos código que no dependa de características del sistema operativo. Esto significa que no podemos usar hilos, archivos, memoria dinámica, redes, números aleatorios, salida estándar ni ninguna otra característica que requiera abstracciones de sistema operativo o hardware específico. Esto tiene sentido, ya que estamos intentando escribir nuestro propio sistema operativo y nuestros propios controladores.

Esto implica que no podemos usar la mayor parte de la [biblioteca estándar de Rust], pero hay muchas características de Rust que sí _podemos_ usar. Por ejemplo, podemos utilizar [iteradores], [closures], [pattern matching], [option] y [result], [formateo de cadenas] y, por supuesto, el [sistema de ownership]. Estas características hacen posible escribir un kernel de una manera muy expresiva y de alto nivel, sin preocuparnos por el [comportamiento indefinido] o la [seguridad de la memoria].

[option]: https://doc.rust-lang.org/core/option/
[result]: https://doc.rust-lang.org/core/result/
[biblioteca estándar de Rust]: https://doc.rust-lang.org/std/
[iteradores]: https://doc.rust-lang.org/book/ch13-02-iterators.html
[closures]: https://doc.rust-lang.org/book/ch13-01-closures.html
[pattern matching]: https://doc.rust-lang.org/book/ch06-00-enums.html
[formateo de cadenas]: https://doc.rust-lang.org/core/macro.write.html
[sistema de ownership]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
[comportamiento indefinido]: https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs
[seguridad de la memoria]: https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention

Para crear un kernel de sistema operativo en Rust, necesitamos crear un ejecutable que pueda ejecutarse sin un sistema operativo subyacente. Dicho ejecutable se llama frecuentemente un ejecutable “autónomo” o de “bare metal”.

Esta publicación describe los pasos necesarios para crear un binario autónomo en Rust y explica por qué son necesarios. Si solo te interesa un ejemplo mínimo, puedes **[saltar al resumen](#summary)**.

## Deshabilitando la Biblioteca Estándar
Por defecto, todos los crates de Rust enlazan con la [biblioteca estándar], que depende del sistema operativo para características como hilos, archivos o redes. También depende de la biblioteca estándar de C, `libc`, que interactúa estrechamente con los servicios del sistema operativo. Como nuestro plan es escribir un sistema operativo, no podemos usar ninguna biblioteca que dependa del sistema operativo. Por lo tanto, tenemos que deshabilitar la inclusión automática de la biblioteca estándar mediante el atributo [`no_std`].

[biblioteca estándar]: https://doc.rust-lang.org/std/
[`no_std`]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

Comenzamos creando un nuevo proyecto de aplicación en Cargo. La forma más fácil de hacerlo es a través de la línea de comandos:


```
cargo new blog_os --bin --edition 2018
```

Nombré el proyecto `blog_os`, pero, por supuesto, puedes elegir tu propio nombre. La bandera `--bin` especifica que queremos crear un binario ejecutable (en contraste con una biblioteca), y la bandera `--edition 2018` indica que queremos usar la [edición 2018] de Rust para nuestro crate. Al ejecutar el comando, Cargo crea la siguiente estructura de directorios para nosotros:

[2018 edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

El archivo `Cargo.toml` contiene la configuración del crate, como el nombre del crate, el autor, el número de [versión semántica] y las dependencias. El archivo `src/main.rs` contiene el módulo raíz de nuestro crate y nuestra función `main`. Puedes compilar tu crate utilizando `cargo build` y luego ejecutar el binario compilado `blog_os` ubicado en la subcarpeta `target/debug`.

[semantic version]: https://semver.org/

### El Atributo `no_std`

Actualmente, nuestro crate enlaza implícitamente con la biblioteca estándar. Intentemos deshabilitar esto añadiendo el [`atributo no_std`]:

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

Cuando intentamos compilarlo ahora (ejecutando `cargo build`), ocurre el siguiente error:

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

La razón de este error es que la [macro `println`] forma parte de la biblioteca estándar, la cual ya no estamos incluyendo. Por lo tanto, ya no podemos imprimir cosas. Esto tiene sentido, ya que `println` escribe en la [salida estándar], que es un descriptor de archivo especial proporcionado por el sistema operativo.

[macro `println`]: https://doc.rust-lang.org/std/macro.println.html
[salida estándar]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

Así que eliminemos la impresión e intentemos de nuevo con una función `main` vacía:

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

Ahora el compilador indica que falta una función `#[panic_handler]` y un _elemento de lenguaje_ (_language item_).

## Implementación de Panic

El atributo `panic_handler` define la función que el compilador invoca cuando ocurre un [panic]. La biblioteca estándar proporciona su propia función de panico, pero en un entorno `no_std` debemos definirla nosotros mismos:

[panic]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// in main.rs

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

El [parámetro `PanicInfo`][PanicInfo] contiene el archivo y la línea donde ocurrió el panic, así como el mensaje opcional del panic. La función no debería retornar nunca, por lo que se marca como una [función divergente][diverging function] devolviendo el [tipo “never”][“never” type] `!`. Por ahora, no hay mucho que podamos hacer en esta función, así que simplemente entramos en un bucle infinito.

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[“never” type]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## El Elemento de Lenguaje `eh_personality`

Los elementos de lenguaje son funciones y tipos especiales que el compilador requiere internamente. Por ejemplo, el trait [`Copy`] es un elemento de lenguaje que indica al compilador qué tipos tienen [_semántica de copia_][`Copy`]. Si observamos su [implementación][copy code], veremos que tiene el atributo especial `#[lang = "copy"]`, que lo define como un elemento de lenguaje.

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

Aunque es posible proporcionar implementaciones personalizadas de elementos de lenguaje, esto debería hacerse solo como último recurso. La razón es que los elementos de lenguaje son detalles de implementación altamente inestables y ni siquiera están verificados por tipos (el compilador no comprueba si una función tiene los tipos de argumento correctos). Afortunadamente, hay una forma más estable de solucionar el error relacionado con el elemento de lenguaje mencionado.

El [elemento de lenguaje `eh_personality`][`eh_personality` language item] marca una función utilizada para implementar el [desenrollado de pila][stack unwinding]. Por defecto, Rust utiliza unwinding para ejecutar los destructores de todas las variables de pila activas en caso de un [pánico][panic]. Esto asegura que toda la memoria utilizada sea liberada y permite que el hilo principal capture el pánico y continúe ejecutándose. Sin embargo, el unwinding es un proceso complicado y requiere algunas bibliotecas específicas del sistema operativo (por ejemplo, [libunwind] en Linux o [manejadores estructurados de excepciones][structured exception handling] en Windows), por lo que no queremos usarlo en nuestro sistema operativo.

[`eh_personality` language item]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: https://www.nongnu.org/libunwind/
[structured exception handling]: https://docs.microsoft.com/en-us/windows/win32/debug/structured-exception-handling
[panic]: https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html

### Deshabilitando el Unwinding

Existen otros casos de uso en los que el  no es deseable, por lo que Rust proporciona una opción para [abortar en caso de pánico][abort on panic]. Esto desactiva la generación de información de símbolos de unwinding y, por lo tanto, reduce considerablemente el tamaño del binario. Hay múltiples lugares donde podemos deshabilitar el unwinding. La forma más sencilla es agregar las siguientes líneas a nuestro archivo `Cargo.toml`:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

Esto establece la estrategia de pánico en `abort` tanto para el perfil `dev` (utilizado en `cargo build`) como para el perfil `release` (utilizado en `cargo build --release`). Ahora, el elemento de lenguaje `eh_personality` ya no debería ser necesario.

[abort on panic]: https://github.com/rust-lang/rust/pull/32900

Ahora hemos solucionado ambos errores anteriores. Sin embargo, si intentamos compilarlo ahora, ocurre otro error:

```
> cargo build
error: requires `start` lang_item
```

Nuestro programa carece del elemento de lenguaje `start`, que define el punto de entrada.

## El Atributo `start`

Podría pensarse que la función `main` es la primera que se ejecuta al correr un programa. Sin embargo, la mayoría de los lenguajes tienen un [sistema de tiempo de ejecución][runtime system], encargado de tareas como la recolección de basura (por ejemplo, en Java) o los hilos de software (por ejemplo, goroutines en Go). Este sistema de tiempo de ejecución necesita ejecutarse antes de `main`, ya que debe inicializarse.

[runtime system]: https://en.wikipedia.org/wiki/Runtime_system

En un binario típico de Rust que enlaza con la biblioteca estándar, la ejecución comienza en una biblioteca de tiempo de ejecución de C llamada `crt0` ("C runtime zero"), que configura el entorno para una aplicación en C. Esto incluye la creación de una pila y la colocación de los argumentos en los registros adecuados. Luego, el tiempo de ejecución de C invoca el [punto de entrada del tiempo de ejecución de Rust][rt::lang_start], que está marcado por el elemento de lenguaje `start`. Rust tiene un tiempo de ejecución muy minimalista, que se encarga de tareas menores como configurar los guardias de desbordamiento de pila o imprimir un backtrace en caso de pánico. Finalmente, el tiempo de ejecución llama a la función `main`.

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

Nuestro ejecutable autónomo no tiene acceso al tiempo de ejecución de Rust ni a `crt0`, por lo que necesitamos definir nuestro propio punto de entrada. Implementar el elemento de lenguaje `start` no ayudaría, ya que aún requeriría `crt0`. En su lugar, debemos sobrescribir directamente el punto de entrada de `crt0`.

### Sobrescribiendo el Punto de Entrada

Para indicar al compilador de Rust que no queremos usar la cadena normal de puntos de entrada, agregamos el atributo `#![no_main]`:

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

Podrás notar que eliminamos la función `main`. La razón es que una función `main` no tiene sentido sin un sistema de tiempo de ejecución subyacente que la invoque. En su lugar, estamos sobrescribiendo el punto de entrada del sistema operativo con nuestra propia función `_start`:

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

Al usar el atributo `#[no_mangle]`, deshabilitamos el [name mangling] para asegurarnos de que el compilador de Rust realmente genere una función con el nombre `_start`. Sin este atributo, el compilador generaría un símbolo críptico como `_ZN3blog_os4_start7hb173fedf945531caE` para dar un nombre único a cada función. Este atributo es necesario porque necesitamos informar al enlazador el nombre de la función de punto de entrada en el siguiente paso.

También debemos marcar la función como `extern "C"` para indicar al compilador que debe usar la [convención de llamadas en C][C calling convention] para esta función (en lugar de la convención de llamadas de Rust, que no está especificada). El motivo para nombrar la función `_start` es que este es el nombre predeterminado del punto de entrada en la mayoría de los sistemas.

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[C calling convention]: https://en.wikipedia.org/wiki/Calling_convention

El tipo de retorno `!` significa que la función es divergente, es decir, no está permitido que retorne nunca. Esto es necesario porque el punto de entrada no es llamado por ninguna función, sino que es invocado directamente por el sistema operativo o el bootloader. En lugar de retornar, el punto de entrada debería, por ejemplo, invocar la [llamada al sistema `exit`][`exit` system call] del sistema operativo. En nuestro caso, apagar la máquina podría ser una acción razonable, ya que no queda nada por hacer si un binario autónomo regresa. Por ahora, cumplimos con este requisito entrando en un bucle infinito.

[`exit` system call]: https://en.wikipedia.org/wiki/Exit_(system_call)

Cuando ejecutamos `cargo build` ahora, obtenemos un feo error del _linker_ (enlazador).

## Errores del Enlazador

El enlazador es un programa que combina el código generado en un ejecutable. Dado que el formato del ejecutable varía entre Linux, Windows y macOS, cada sistema tiene su propio enlazador que lanza errores diferentes. Sin embargo, la causa fundamental de los errores es la misma: la configuración predeterminada del enlazador asume que nuestro programa depende del tiempo de ejecución de C, lo cual no es cierto.

Para solucionar los errores, necesitamos informar al enlazador que no debe incluir el tiempo de ejecución de C. Esto puede hacerse pasando un conjunto específico de argumentos al enlazador o construyendo para un destino de bare metal.

### Construyendo para un Destino de Bare Metal

Por defecto, Rust intenta construir un ejecutable que pueda ejecutarse en el entorno actual de tu sistema. Por ejemplo, si estás usando Windows en `x86_64`, Rust intenta construir un ejecutable `.exe` para Windows que utilice instrucciones `x86_64`. Este entorno se llama tu sistema "host".

Para describir diferentes entornos, Rust utiliza una cadena llamada [_target triple_]. Puedes ver el _target triple_ de tu sistema host ejecutando:

```
rustc 1.35.0-nightly (474e7a648 2019-04-07)
binary: rustc
commit-hash: 474e7a6486758ea6fc761893b1a49cd9076fb0ab
commit-date: 2019-04-07
host: x86_64-unknown-linux-gnu
release: 1.35.0-nightly
LLVM version: 8.0
```

El resultado anterior es de un sistema Linux `x86_64`. Vemos que la tripleta del `host` es `x86_64-unknown-linux-gnu`, lo que incluye la arquitectura de la CPU (`x86_64`), el proveedor (`unknown`), el sistema operativo (`linux`) y el [ABI] (`gnu`).

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

Al compilar para la tripleta del host, el compilador de Rust y el enlazador asumen que hay un sistema operativo subyacente como Linux o Windows que utiliza el tiempo de ejecución de C de forma predeterminada, lo que provoca los errores del enlazador. Para evitar estos errores, podemos compilar para un entorno diferente que no tenga un sistema operativo subyacente.

Un ejemplo de este tipo de entorno bare metal es la tripleta de destino `thumbv7em-none-eabihf`, que describe un sistema [embebido][embedded] basado en [ARM]. Los detalles no son importantes, lo que importa es que la tripleta de destino no tiene un sistema operativo subyacente, lo cual se indica por el `none` en la tripleta de destino. Para poder compilar para este destino, necesitamos agregarlo usando `rustup`:

```
rustup target add thumbv7em-none-eabihf
```

Esto descarga una copia de las bibliotecas estándar (y core) para el sistema. Ahora podemos compilar nuestro ejecutable autónomo para este destino:

```
cargo build --target thumbv7em-none-eabihf
```

Al pasar un argumento `--target`, realizamos un [compilado cruzado][cross compile] de nuestro ejecutable para un sistema bare metal. Dado que el sistema de destino no tiene un sistema operativo, el enlazador no intenta enlazar con el tiempo de ejecución de C, y nuestra compilación se completa sin errores del enlazador.

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

Este es el enfoque que utilizaremos para construir nuestro kernel de sistema operativo. En lugar de `thumbv7em-none-eabihf`, utilizaremos un [destino personalizado][custom target] que describa un entorno bare metal `x86_64`. Los detalles se explicarán en la siguiente publicación.

[custom target]: https://doc.rust-lang.org/rustc/targets/custom.html

### Argumentos del Enlazador

En lugar de compilar para un sistema bare metal, también es posible resolver los errores del enlazador pasando un conjunto específico de argumentos al enlazador. Este no es el enfoque que usaremos para nuestro kernel, por lo tanto, esta sección es opcional y se proporciona solo para completar. Haz clic en _"Argumentos del Enlazador"_ a continuación para mostrar el contenido opcional.

<details>

<summary>Argumentos del Enlazador</summary>

En esta sección discutimos los errores del enlazador que ocurren en Linux, Windows y macOS, y explicamos cómo resolverlos pasando argumentos adicionales al enlazador. Ten en cuenta que el formato del ejecutable y el enlazador varían entre sistemas operativos, por lo que se requiere un conjunto diferente de argumentos para cada sistema.

#### Linux

En Linux ocurre el siguiente error del enlazador (resumido):

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

El problema es que el enlazador incluye por defecto la rutina de inicio del tiempo de ejecución de C, que también se llama `_start`. Esta rutina requiere algunos símbolos de la biblioteca estándar de C `libc` que no incluimos debido al atributo `no_std`, por lo que el enlazador no puede resolver estas referencias. Para solucionar esto, podemos indicar al enlazador que no enlace la rutina de inicio de C pasando la bandera `-nostartfiles`.

Una forma de pasar atributos al enlazador a través de Cargo es usar el comando `cargo rustc`. Este comando se comporta exactamente como `cargo build`, pero permite pasar opciones a `rustc`, el compilador subyacente de Rust. `rustc` tiene la bandera `-C link-arg`, que pasa un argumento al enlazador. Combinados, nuestro nuevo comando de compilación se ve así:

```
cargo rustc -- -C link-arg=-nostartfiles
```

¡Ahora nuestro crate se compila como un ejecutable autónomo en Linux!

No fue necesario especificar explícitamente el nombre de nuestra función de punto de entrada, ya que el enlazador busca una función con el nombre `_start` por defecto.

#### Windows

En Windows, ocurre un error del enlazador diferente (resumido):

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

El error "entry point must be defined" significa que el enlazador no puede encontrar el punto de entrada. En Windows, el nombre predeterminado del punto de entrada [depende del subsistema utilizado][windows-subsystems]. Para el subsistema `CONSOLE`, el enlazador busca una función llamada `mainCRTStartup`, y para el subsistema `WINDOWS`, busca una función llamada `WinMainCRTStartup`. Para anular este comportamiento predeterminado y decirle al enlazador que busque nuestra función `_start`, podemos pasar un argumento `/ENTRY` al enlazador:

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

Por el formato diferente del argumento, podemos ver claramente que el enlazador de Windows es un programa completamente distinto al enlazador de Linux.

Ahora ocurre un error diferente del enlazador:

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

Este error ocurre porque los ejecutables de Windows pueden usar diferentes [subsistemas][windows-subsystems]. En programas normales, se infieren dependiendo del nombre del punto de entrada: si el punto de entrada se llama `main`, se usa el subsistema `CONSOLE`, y si el punto de entrada se llama `WinMain`, se usa el subsistema `WINDOWS`. Dado que nuestra función `_start` tiene un nombre diferente, necesitamos especificar el subsistema explícitamente:

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

Aquí usamos el subsistema `CONSOLE`, pero el subsistema `WINDOWS` también funcionaría. En lugar de pasar `-C link-arg` varias veces, podemos usar `-C link-args`, que acepta una lista de argumentos separados por espacios.

Con este comando, nuestro ejecutable debería compilarse exitosamente en Windows.

#### macOS

En macOS, ocurre el siguiente error del enlazador (resumido):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

Este mensaje de error nos indica que el enlazador no puede encontrar una función de punto de entrada con el nombre predeterminado `main` (por alguna razón, en macOS todas las funciones tienen un prefijo `_`). Para establecer el punto de entrada en nuestra función `_start`, pasamos el argumento del enlazador `-e`:

```
cargo rustc -- -C link-args="-e __start"
```

La bandera `-e` especifica el nombre de la función de punto de entrada. Dado que en macOS todas las funciones tienen un prefijo adicional `_`, necesitamos establecer el punto de entrada en `__start` en lugar de `_start`.

Ahora ocurre el siguiente error del enlazador:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS [no admite oficialmente binarios enlazados estáticamente] y requiere que los programas enlacen la biblioteca `libSystem` por defecto. Para anular esto y enlazar un binario estático, se pasa la bandera `-static` al enlazador:

[no admite oficialmente binarios enlazados estáticamente]: https://developer.apple.com/library/archive/qa/qa1118/_index.html


```
cargo rustc -- -C link-args="-e __start -static"
```

Esto aún no es suficiente, ya que ocurre un tercer error del enlazador:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

Este error ocurre porque los programas en macOS enlazan con `crt0` (“C runtime zero”) por defecto. Esto es similar al error que tuvimos en Linux y también se puede resolver añadiendo el argumento del enlazador `-nostartfiles`:

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Ahora nuestro programa debería compilarse exitosamente en macOS.

#### Unificando los Comandos de Construcción

Actualmente, tenemos diferentes comandos de construcción dependiendo de la plataforma host, lo cual no es ideal. Para evitar esto, podemos crear un archivo llamado `.cargo/config.toml` que contenga los argumentos específicos de cada plataforma:

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

La clave `rustflags` contiene argumentos que se añaden automáticamente a cada invocación de `rustc`. Para más información sobre el archivo `.cargo/config.toml`, consulta la [documentación oficial](https://doc.rust-lang.org/cargo/reference/config.html).

Ahora nuestro programa debería poder construirse en las tres plataformas con un simple `cargo build`.

#### ¿Deberías Hacer Esto?

Aunque es posible construir un ejecutable autónomo para Linux, Windows y macOS, probablemente no sea una buena idea. La razón es que nuestro ejecutable aún espera varias cosas, por ejemplo, que una pila esté inicializada cuando se llama a la función `_start`. Sin el tiempo de ejecución de C, algunos de estos requisitos podrían no cumplirse, lo que podría hacer que nuestro programa falle, por ejemplo, con un error de segmentación.

Si deseas crear un binario mínimo que se ejecute sobre un sistema operativo existente, incluir `libc` y configurar el atributo `#[start]` como se describe [aquí](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html) probablemente sea una mejor idea.

</details>

## Resumen

Un binario mínimo autónomo en Rust se ve así:

`src/main.rs`:

```rust
#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use core::panic::PanicInfo;

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}

/// This function is called on panic.
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

# the profile used for `cargo build`
[profile.dev]
panic = "abort" # disable stack unwinding on panic

# the profile used for `cargo build --release`
[profile.release]
panic = "abort" # disable stack unwinding on panic
```

Para construir este binario, necesitamos compilar para un destino bare metal, como `thumbv7em-none-eabihf`:

```
cargo build --target thumbv7em-none-eabihf
```

Alternativamente, podemos compilarlo para el sistema host pasando argumentos adicionales al enlazador:

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Ten en cuenta que este es solo un ejemplo mínimo de un binario autónomo en Rust. Este binario espera varias cosas, por ejemplo, que una pila esté inicializada cuando se llama a la función `_start`. **Por lo tanto, para cualquier uso real de un binario como este, se requieren más pasos**.

## ¿Qué sigue?

La [próxima publicación][next post] explica los pasos necesarios para convertir nuestro binario autónomo en un kernel de sistema operativo mínimo. Esto incluye crear un destino personalizado, combinar nuestro ejecutable con un bootloader y aprender cómo imprimir algo en la pantalla.

[next post]: @/edition-2/posts/02-minimal-rust-kernel/index.md
