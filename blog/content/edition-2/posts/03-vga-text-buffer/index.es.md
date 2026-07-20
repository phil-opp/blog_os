+++
title = "Modo de Texto VGA"
weight = 3
path = "es/modo-texto-vga"
date  = 2018-02-26

[extra]
# Please update this when updating the translation
translation_based_on_commit = "1132d7a3835dc6c0b3fd8f6b45c9295a9bc1f837"
chapter = "Fundamentos"

# GitHub usernames of the people that translated this post
translators = ["dobleuber"]
+++

El [modo de texto VGA] es una forma sencilla de imprimir texto en la pantalla. En esta publicación, creamos una interfaz que hace que su uso sea seguro y simple al encapsular toda la inseguridad en un módulo separado. También implementamos soporte para los [macros de formato] de Rust.

[modo de texto VGA]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode
[macros de formato]: https://doc.rust-lang.org/std/fmt/#related-macros

<!-- more -->

Este blog se desarrolla abiertamente en [GitHub]. Si tienes algún problema o pregunta, por favor abre un issue allí. También puedes dejar comentarios [al final]. El código fuente completo para esta publicación se puede encontrar en la rama [`post-03`][rama del post].

[GitHub]: https://github.com/phil-opp/blog_os
[al final]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[rama del post]: https://github.com/phil-opp/blog_os/tree/post-03

<!-- toc -->

## El Buffer de Texto VGA
Para imprimir un carácter en la pantalla en modo de texto VGA, uno tiene que escribirlo en el buffer de texto del hardware VGA. El buffer de texto VGA es un arreglo bidimensional con típicamente 25 filas y 80 columnas, que se renderiza directamente en la pantalla. Cada entrada del arreglo describe un solo carácter de pantalla a través del siguiente formato:

| Bit(s) | Valor                 |
| ------ | --------------------- |
| 0-7    | Código de punto ASCII |
| 8-11   | Color de primer plano |
| 12-14  | Color de fondo        |
| 15     | Parpadeo              |

El primer byte representa el carácter que debe imprimirse en la [codificación ASCII]. Para ser más específicos, no es exactamente ASCII, sino un conjunto de caracteres llamado [_página de códigos 437_] con algunos caracteres adicionales y ligeras modificaciones. Para simplificar, procederemos a llamarlo un carácter ASCII en esta publicación.

[codificación ASCII]: https://en.wikipedia.org/wiki/ASCII
[_página de códigos 437_]: https://en.wikipedia.org/wiki/Code_page_437

El segundo byte define cómo se muestra el carácter. Los primeros cuatro bits definen el color de primer plano, los siguientes tres bits el color de fondo, y el último bit si el carácter debe parpadear. Los siguientes colores están disponibles:

| Número | Color      | Número + Bit de Brillo | Color Brillante |
| ------ | ---------- | ---------------------- | --------------- |
| 0x0    | Negro      | 0x8                    | Gris Oscuro     |
| 0x1    | Azul       | 0x9                    | Azul Claro      |
| 0x2    | Verde      | 0xa                    | Verde Claro     |
| 0x3    | Cian       | 0xb                    | Cian Claro      |
| 0x4    | Rojo       | 0xc                    | Rojo Claro      |
| 0x5    | Magenta    | 0xd                    | Magenta Claro   |
| 0x6    | Marrón     | 0xe                    | Amarillo        |
| 0x7    | Gris Claro | 0xf                    | Blanco          |

Bit 4 es el _bit de brillo_, que convierte, por ejemplo, azul en azul claro. Para el color de fondo, este bit se reutiliza como el bit de parpadeo.

El buffer de texto VGA es accesible a través de [E/S mapeada en memoria] a la dirección `0xb8000`. Esto significa que las lecturas y escrituras a esa dirección no acceden a la RAM, sino que acceden directamente al buffer de texto en el hardware VGA. Esto significa que podemos leer y escribir a través de operaciones de memoria normales a esa dirección.

[E/S mapeada en memoria]: https://en.wikipedia.org/wiki/Memory-mapped_I/O

Ten en cuenta que el hardware mapeado en memoria podría no soportar todas las operaciones normales de RAM. Por ejemplo, un dispositivo podría soportar solo lecturas por byte y devolver basura cuando se lee un `u64`. Afortunadamente, el buffer de texto [soporta lecturas y escrituras normales], por lo que no tenemos que tratarlo de una manera especial.

[soporta lecturas y escrituras normales]: https://web.stanford.edu/class/cs140/projects/pintos/specs/freevga/vga/vgamem.htm#manip

## Un Módulo de Rust
Ahora que sabemos cómo funciona el buffer VGA, podemos crear un módulo de Rust para manejar la impresión:

```rust
// en src/main.rs
mod vga_buffer;
```

Para el contenido de este módulo, creamos un nuevo archivo `src/vga_buffer.rs`. Todo el código a continuación va en nuestro nuevo módulo (a menos que se especifique lo contrario).

### Colores
Primero, representamos los diferentes colores usando un enum:

```rust
// en src/vga_buffer.rs

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
Usamos un [enum similar a C] aquí para especificar explícitamente el número para cada color. Debido al atributo `repr(u8)`, cada variante del enum se almacena como un `u8`. En realidad, 4 bits serían suficientes, pero Rust no tiene un tipo `u4`.

[enum similar a C]: https://doc.rust-lang.org/rust-by-example/custom_types/enum/c_like.html

Normalmente, el compilador emitiría una advertencia por cada variante no utilizada. Al usar el atributo `#[allow(dead_code)]`, deshabilitamos estas advertencias para el enum `Color`.

Al [derivar] los rasgos [`Copy`], [`Clone`], [`Debug`], [`PartialEq`], y [`Eq`], habilitamos la [semántica de copia] para el tipo y lo hacemos imprimible y comparable.

[derivar]: https://doc.rust-lang.org/rust-by-example/trait/derive.html
[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[`Clone`]: https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html

Para representar un código de color completo que especifique el color de primer plano y de fondo, creamos un [nuevo tipo] sobre `u8`:

[nuevo tipo]: https://doc.rust-lang.org/rust-by-example/generics/new_types.html

```rust
// en src/vga_buffer.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}
```
La estructura `ColorCode` contiene el byte de color completo, que incluye el color de primer plano y de fondo. Como antes, derivamos los rasgos `Copy` y `Debug` para él. Para asegurar que `ColorCode` tenga el mismo diseño de datos exacto que un `u8`, usamos el atributo [`repr(transparent)`].

[`repr(transparent)`]: https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent

### Buffer de Texto
Ahora podemos agregar estructuras para representar un carácter de pantalla y el buffer de texto:

```rust
// en src/vga_buffer.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}
```
Dado que el orden de los campos en las estructuras predeterminadas no está definido en Rust, necesitamos el atributo [`repr(C)`]. Garantiza que los campos de la estructura se dispongan exactamente como en una estructura C y, por lo tanto, garantiza el orden correcto de los campos. Para la estructura `Buffer`, usamos [`repr(transparent)`] nuevamente para asegurar que tenga el mismo diseño de memoria que su único campo.

[`repr(C)`]: https://doc.rust-lang.org/nightly/nomicon/other-reprs.html#reprc

Para escribir en pantalla, ahora creamos un tipo de escritor:

```rust
// en src/vga_buffer.rs

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}
```
El escritor siempre escribirá en la última línea y desplazará las líneas hacia arriba cuando una línea esté llena (o en `\n`). El campo `column_position` lleva un seguimiento de la posición actual en la última fila. Los colores de primer plano y de fondo actuales están especificados por `color_code` y una referencia al buffer VGA está almacenada en `buffer`. Ten en cuenta que necesitamos una [vida útil explícita] aquí para decirle al compilador cuánto tiempo es válida la referencia. La vida útil [`'static`] especifica que la referencia es válida durante todo el tiempo de ejecución del programa (lo cual es cierto para el buffer de texto VGA).

[vida útil explícita]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#lifetime-annotation-syntax
[`'static`]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime

### Impresión
Ahora podemos usar el `Writer` para modificar los caracteres del buffer. Primero creamos un método para escribir un solo byte ASCII:

```rust
// en src/vga_buffer.rs

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
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {/* TODO */}
}
```
Si el byte es el byte de [nueva línea] `\n`, el escritor no imprime nada. En su lugar, llama a un método `new_line`, que implementaremos más tarde. Otros bytes se imprimen en la pantalla en el segundo caso de `match`.

[nueva línea]: https://en.wikipedia.org/wiki/Newline

Al imprimir un byte, el escritor verifica si la línea actual está llena. En ese caso, se usa una llamada a `new_line` para envolver la línea. Luego escribe un nuevo `ScreenChar` en el buffer en la posición actual. Finalmente, se avanza la posición de la columna actual.

Para imprimir cadenas completas, podemos convertirlas en bytes e imprimirlas una por una:

```rust
// en src/vga_buffer.rs

impl Writer {
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // byte ASCII imprimible o nueva línea
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // no es parte del rango ASCII imprimible
                _ => self.write_byte(0xfe),
            }

        }
    }
}
```

El buffer de texto VGA solo soporta ASCII y los bytes adicionales de [página de códigos 437]. Las cadenas de Rust son [UTF-8] por defecto, por lo que podrían contener bytes que no son soportados por el buffer de texto VGA. Usamos un `match` para diferenciar los bytes ASCII imprimibles (una nueva línea o cualquier cosa entre un carácter de espacio y un carácter `~`) y los bytes no imprimibles. Para los bytes no imprimibles, imprimimos un carácter `■`, que tiene el código hexadecimal `0xfe` en el hardware VGA.

[página de códigos 437]: https://en.wikipedia.org/wiki/Code_page_437
[UTF-8]: https://www.fileformat.info/info/unicode/utf8.htm

#### ¡Pruébalo!
Para escribir algunos caracteres en la pantalla, puedes crear una función temporal:

```rust
// en src/vga_buffer.rs

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
Primero crea un nuevo Writer que apunta al buffer VGA en `0xb8000`. La sintaxis para esto podría parecer un poco extraña: Primero, convertimos el entero `0xb8000` como un [puntero sin procesar] mutable. Luego lo convertimos en una referencia mutable al desreferenciarlo (a través de `*`) y tomarlo prestado inmediatamente (a través de `&mut`). Esta conversión requiere un [bloque `unsafe`], ya que el compilador no puede garantizar que el puntero sin procesar sea válido.

[puntero sin procesar]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#dereferencing-a-raw-pointer
[bloque `unsafe`]: https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html

Luego escribe el byte `b'H'` en él. El prefijo `b` crea un [literal de byte], que representa un carácter ASCII. Al escribir las cadenas `"ello "` y `"Wörld!"`, probamos nuestro método `write_string` y el manejo de caracteres no imprimibles. Para ver la salida, necesitamos llamar a la función `print_something` desde nuestra función `_start`:

```rust
// en src/main.rs

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    vga_buffer::print_something();

    loop {}
}
```

Cuando ejecutamos nuestro proyecto ahora, se debería imprimir un `Hello W■■rld!` en la esquina inferior izquierda de la pantalla en amarillo:

[literal de byte]: https://doc.rust-lang.org/reference/tokens.html#byte-literals

![Salida de QEMU con un `Hello W■■rld!` en amarillo en la esquina inferior izquierda](vga-hello.png)

Observa que la `ö` se imprime como dos caracteres `■`. Eso es porque `ö` está representado por dos bytes en [UTF-8], los cuales no caen en el rango ASCII imprimible. De hecho, esta es una propiedad fundamental de UTF-8: los bytes individuales de valores multibyte nunca son ASCII válidos.

### Volátil
Acabamos de ver que nuestro mensaje se imprimió correctamente. Sin embargo, podría no funcionar con futuros compiladores de Rust que optimicen más agresivamente.

El problema es que solo escribimos en el `Buffer` y nunca leemos de él nuevamente. El compilador no sabe que realmente accedemos a la memoria del buffer VGA (en lugar de la RAM normal) y no sabe nada sobre el efecto secundario de que algunos caracteres aparezcan en la pantalla. Por lo tanto, podría decidir que estas escrituras son innecesarias y pueden omitirse. Para evitar esta optimización errónea, necesitamos especificar estas escrituras como _[volátiles]_. Esto le dice al compilador que la escritura tiene efectos secundarios y no debe ser optimizada.

[volátiles]: https://en.wikipedia.org/wiki/Volatile_(computer_programming)

Para usar escrituras volátiles para el buffer VGA, usamos la biblioteca [volatile][crate volatile]. Este _crate_ (así es como se llaman los paquetes en el mundo de Rust) proporciona un tipo de envoltura `Volatile` con métodos `read` y `write`. Estos métodos usan internamente las funciones [read_volatile] y [write_volatile] de la biblioteca principal y, por lo tanto, garantizan que las lecturas/escrituras no sean optimizadas.

[crate volatile]: https://docs.rs/volatile
[read_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.read_volatile.html
[write_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.write_volatile.html

Podemos agregar una dependencia en el crate `volatile` agregándolo a la sección `dependencies` de nuestro `Cargo.toml`:

```toml
# en Cargo.toml

[dependencies]
volatile = "0.2.6"
```

Asegúrate de especificar la versión `0.2.6` de `volatile`. Las versiones más nuevas del crate no son compatibles con esta publicación.
`0.2.6` es el número de versión [semántica]. Para más información, consulta la guía [Especificar Dependencias] de la documentación de cargo.

[semántica]: https://semver.org/
[Especificar Dependencias]: https://doc.crates.io/specifying-dependencies.html

Vamos a usarlo para hacer que las escrituras al buffer VGA sean volátiles. Actualizamos nuestro tipo `Buffer` de la siguiente manera:

```rust
// en src/vga_buffer.rs

use volatile::Volatile;

struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```
En lugar de un `ScreenChar`, ahora estamos usando un `Volatile<ScreenChar>`. (El tipo `Volatile` es [genérico] y puede envolver (casi) cualquier tipo). Esto asegura que no podamos escribir accidentalmente en él “normalmente”. En su lugar, ahora tenemos que usar el método `write`.

[genérico]: https://doc.rust-lang.org/book/ch10-01-syntax.html

Esto significa que tenemos que actualizar nuestro método `Writer::write_byte`:

```rust
// en src/vga_buffer.rs

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

En lugar de una asignación típica usando `=`, ahora estamos usando el método `write`. Ahora podemos garantizar que el compilador nunca optimizará esta escritura.

### Macros de Formato
Sería bueno soportar también los macros de formato de Rust. De esa manera, podemos imprimir fácilmente diferentes tipos, como enteros o flotantes. Para soportarlos, necesitamos implementar el rasgo [`core::fmt::Write`]. El único método requerido de este rasgo es `write_str`, que se parece bastante a nuestro método `write_string`, solo que con un tipo de retorno `fmt::Result`:

[`core::fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

```rust
// en src/vga_buffer.rs

use core::fmt;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
```
El `Ok(())` es simplemente un `Result` `Ok` que contiene el tipo `()`.

Ahora podemos usar los macros de formato integrados de Rust `write!`/`writeln!`:

```rust
// en src/vga_buffer.rs

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

Ahora deberías ver un `Hello! The numbers are 42 and 0.3333333333333333` en la parte inferior de la pantalla. La llamada a `write!` devuelve un `Result` que causa una advertencia si no se usa, por lo que llamamos a la función [`unwrap`] sobre él, que entra en pánico si ocurre un error. Esto no es un problema en nuestro caso, ya que las escrituras al buffer VGA nunca fallan.

[`unwrap`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.unwrap

### Nuevas Líneas
En este momento, simplemente ignoramos las nuevas líneas y los caracteres que ya no caben en la línea. En su lugar, queremos mover cada carácter una línea hacia arriba (la línea superior se elimina) y comenzar de nuevo al inicio de la última línea. Para hacer esto, agregamos una implementación para el método `new_line` de `Writer`:

```rust
// en src/vga_buffer.rs

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
Iteramos sobre todos los caracteres de la pantalla y movemos cada carácter una fila hacia arriba. Ten en cuenta que el límite superior de la notación de rango (`..`) es exclusivo. También omitimos la fila 0 (el primer rango comienza en `1`) porque es la fila que se desplaza fuera de la pantalla.

Para terminar el código de nueva línea, agregamos el método `clear_row`:

```rust
// en src/vga_buffer.rs

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
Este método limpia una fila sobrescribiendo todos sus caracteres con un carácter de espacio.

## Una Interfaz Global
Para proporcionar un escritor global que pueda usarse como una interfaz desde otros módulos sin tener que llevar consigo una instancia de `Writer`, intentamos crear un `WRITER` estático:

```rust
// en src/vga_buffer.rs

pub static WRITER: Writer = Writer {
    column_position: 0,
    color_code: ColorCode::new(Color::Yellow, Color::Black),
    buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
};
```

Sin embargo, si intentamos compilarlo ahora, ocurren los siguientes errores:

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

Para entender lo que está sucediendo aquí, necesitamos saber que las estáticas se inicializan en tiempo de compilación, en contraste con las variables normales que se inicializan en tiempo de ejecución. El componente del compilador de Rust que evalúa tales expresiones de inicialización se llama el “[evaluador de constantes]”. Su funcionalidad todavía es limitada, pero hay trabajo en curso para expandirla, por ejemplo en el RFC “[Permitir pánico en constantes]”.

[evaluador de constantes]: https://rustc-dev-guide.rust-lang.org/const-eval.html
[Permitir pánico en constantes]: https://github.com/rust-lang/rfcs/pull/2345

El problema con `ColorCode::new` sería solucionable usando [funciones `const`], pero el problema fundamental aquí es que el evaluador de constantes de Rust no puede convertir punteros sin procesar a referencias en tiempo de compilación. Tal vez funcione algún día, pero hasta entonces, tenemos que encontrar otra solución.

[funciones `const`]: https://doc.rust-lang.org/reference/const_eval.html#const-functions

### Estáticas Perezosas
La inicialización única de estáticas con funciones no constantes es un problema común en Rust. Afortunadamente, ya existe una buena solución en un crate llamado [lazy_static]. Este crate proporciona un macro `lazy_static!` que define una `static` inicializada de forma perezosa. En lugar de calcular su valor en tiempo de compilación, la `static` se inicializa de forma perezosa cuando se accede a ella por primera vez. Así, la inicialización ocurre en tiempo de ejecución, por lo que es posible tener código de inicialización arbitrariamente complejo.

[lazy_static]: https://docs.rs/lazy_static/1.0.1/lazy_static/

Agreguemos el crate `lazy_static` a nuestro proyecto:

```toml
# en Cargo.toml

[dependencies.lazy_static]
version = "1.0"
features = ["spin_no_std"]
```

Necesitamos la característica `spin_no_std`, ya que no enlazamos la biblioteca estándar.

Con `lazy_static`, podemos definir nuestra `WRITER` estática sin problemas:

```rust
// en src/vga_buffer.rs

use lazy_static::lazy_static;

lazy_static! {
    pub static ref WRITER: Writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };
}
```

Sin embargo, esta `WRITER` es bastante inútil ya que es inmutable. Esto significa que no podemos escribir nada en ella (ya que todos los métodos de escritura toman `&mut self`). Una posible solución sería usar una [estática mutable]. Pero entonces cada lectura y escritura a ella sería insegura, ya que podría introducir fácilmente condiciones de carrera y otras cosas malas. El uso de `static mut` está altamente desaconsejado. Incluso hubo propuestas para [eliminarlo][remove static mut]. Pero, ¿cuáles son las alternativas? Podríamos intentar usar una estática inmutable con un tipo de celda como [RefCell] o incluso [UnsafeCell] que proporcione [mutabilidad interior]. Pero estos tipos no son [Sync] \(con buena razón), por lo que no podemos usarlos en estáticas.

[estática mutable]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable
[remove static mut]: https://internals.rust-lang.org/t/pre-rfc-remove-static-mut/1437
[RefCell]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html#keeping-track-of-borrows-at-runtime-with-refcellt
[UnsafeCell]: https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html
[mutabilidad interior]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
[Sync]: https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html

### Spinlocks
Para obtener mutabilidad interior sincronizada, los usuarios de la biblioteca estándar pueden usar [Mutex]. Proporciona exclusión mutua bloqueando hilos cuando el recurso ya está bloqueado. Pero nuestro kernel básico no tiene ningún soporte de bloqueo ni siquiera un concepto de hilos, por lo que tampoco podemos usarlo. Sin embargo, hay un tipo realmente básico de mutex en informática que no requiere características del sistema operativo: el [spinlock]. En lugar de bloquear, los hilos simplemente intentan bloquearlo una y otra vez en un bucle cerrado, quemando así tiempo de CPU hasta que el mutex esté libre de nuevo.

[Mutex]: https://doc.rust-lang.org/nightly/std/sync/struct.Mutex.html
[spinlock]: https://en.wikipedia.org/wiki/Spinlock

Para usar un mutex giratorio, podemos agregar el [crate spin] como una dependencia:

[crate spin]: https://crates.io/crates/spin

```toml
# en Cargo.toml
[dependencies]
spin = "0.5.2"
```

Luego podemos usar el mutex giratorio para agregar [mutabilidad interior] segura a nuestra `WRITER` estática:

```rust
// en src/vga_buffer.rs

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
Ahora podemos eliminar la función `print_something` e imprimir directamente desde nuestra función `_start`:

```rust
// en src/main.rs
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    write!(vga_buffer::WRITER.lock(), ", some numbers: {} {}", 42, 1.337).unwrap();

    loop {}
}
```
Necesitamos importar el rasgo `fmt::Write` para poder usar sus funciones.

### Seguridad
Ten en cuenta que solo tenemos un único bloque unsafe en nuestro código, que es necesario para crear una referencia `Buffer` que apunte a `0xb8000`. Después de eso, todas las operaciones son seguras. Rust usa verificación de límites para los accesos a arreglos por defecto, por lo que no podemos escribir accidentalmente fuera del buffer. Así, codificamos las condiciones requeridas en el sistema de tipos y podemos proporcionar una interfaz segura hacia el exterior.

### Un Macro println
Ahora que tenemos un escritor global, podemos agregar un macro `println` que se puede usar desde cualquier parte del código. La [sintaxis de macros] de Rust es un poco extraña, por lo que no intentaremos escribir un macro desde cero. En su lugar, veamos el código fuente del [macro `println!`] en la biblioteca estándar:

[sintaxis de macros]: https://doc.rust-lang.org/nightly/book/ch20-05-macros.html#declarative-macros-for-general-metaprogramming
[macro `println!`]: https://doc.rust-lang.org/nightly/std/macro.println!.html

```rust
#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}
```

Los macros se definen a través de una o más reglas, similares a los brazos de `match`. El macro `println` tiene dos reglas: La primera regla es para invocaciones sin argumentos, por ejemplo, `println!()`, que se expande a `print!("\n")` y, por lo tanto, solo imprime una nueva línea. La segunda regla es para invocaciones con parámetros como `println!("Hello")` o `println!("Number: {}", 4)`. También se expande a una invocación del macro `print!`, pasando todos los argumentos y una nueva línea adicional `\n` al final.

El atributo `#[macro_export]` hace que el macro esté disponible para todo el crate (no solo el módulo en el que está definido) y para crates externos. También coloca el macro en la raíz del crate, lo que significa que tenemos que importar el macro a través de `use std::println` en lugar de `std::macros::println`.

El [macro `print!`] se define como:

[macro `print!`]: https://doc.rust-lang.org/nightly/std/macro.print!.html

```rust
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
```

El macro se expande a una llamada de la [función `_print`] en el módulo `io`. La [variable `$crate`] asegura que el macro también funcione desde fuera del crate `std` expandiéndose a `std` cuando se usa en otros crates.

El [macro `format_args`] construye un tipo [fmt::Arguments] a partir de los argumentos pasados, que se pasa a `_print`. La [función `_print`] de libstd llama a `print_to`, que es bastante complicada porque soporta diferentes dispositivos `Stdout`. No necesitamos esa complejidad ya que solo queremos imprimir en el buffer VGA.

[función `_print`]: https://github.com/rust-lang/rust/blob/29f5c699b11a6a148f097f82eaa05202f8799bbc/src/libstd/io/stdio.rs#L698
[variable `$crate`]: https://doc.rust-lang.org/1.30.0/book/first-edition/macros.html#the-variable-crate
[macro `format_args`]: https://doc.rust-lang.org/nightly/std/macro.format_args.html
[fmt::Arguments]: https://doc.rust-lang.org/nightly/core/fmt/struct.Arguments.html

Para imprimir en el buffer VGA, simplemente copiamos los macros `println!` y `print!`, pero los modificamos para usar nuestra propia función `_print`:

```rust
// en src/vga_buffer.rs

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

Una cosa que cambiamos de la definición original de `println` es que también prefijamos las invocaciones del macro `print!` con `$crate`. Esto asegura que no necesitemos importar también el macro `print!` si solo queremos usar `println`.

Como en la biblioteca estándar, agregamos el atributo `#[macro_export]` a ambos macros para hacerlos disponibles en todas partes de nuestro crate. Ten en cuenta que esto coloca los macros en el espacio de nombres raíz del crate, por lo que importarlos a través de `use crate::vga_buffer::println` no funciona. En su lugar, tenemos que hacer `use crate::println`.

La función `_print` bloquea nuestra `WRITER` estática y llama al método `write_fmt` sobre ella. Este método es del rasgo `Write`, que necesitamos importar. El `unwrap()` adicional al final entra en pánico si la impresión no tiene éxito. Pero como siempre devolvemos `Ok` en `write_str`, eso no debería suceder.

Dado que los macros necesitan poder llamar a `_print` desde fuera del módulo, la función necesita ser pública. Sin embargo, como consideramos esto un detalle de implementación privado, agregamos el [atributo `doc(hidden)`] para ocultarla de la documentación generada.

[atributo `doc(hidden)`]: https://doc.rust-lang.org/nightly/rustdoc/write-documentation/the-doc-attribute.html#hidden

### Hola Mundo usando `println`
Ahora podemos usar `println` en nuestra función `_start`:

```rust
// en src/main.rs

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    loop {}
}
```

Ten en cuenta que no tenemos que importar el macro en la función main, porque ya vive en el espacio de nombres raíz.

Como se esperaba, ahora vemos un _“Hello World!”_ en la pantalla:

![QEMU imprimiendo “Hello World!”](vga-hello-world.png)

### Imprimiendo Mensajes de Pánico

Ahora que tenemos un macro `println`, podemos usarlo en nuestra función de pánico para imprimir el mensaje de pánico y la ubicación del pánico:

```rust
// en main.rs

/// Esta función se llama en caso de pánico.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
```

Cuando ahora insertamos `panic!("Some panic message");` en nuestra función `_start`, obtenemos la siguiente salida:

![QEMU imprimiendo “panicked at 'Some panic message', src/main.rs:28:5](vga-panic.png)

Así sabemos no solo que ha ocurrido un pánico, sino también el mensaje de pánico y dónde en el código sucedió.

## Resumen
En esta publicación, aprendimos sobre la estructura del buffer de texto VGA y cómo se puede escribir a través del mapeo de memoria en la dirección `0xb8000`. Creamos un módulo de Rust que encapsula la inseguridad de escribir en este buffer mapeado en memoria y presenta una interfaz segura y conveniente hacia el exterior.

Gracias a cargo, también vimos lo fácil que es agregar dependencias de bibliotecas de terceros. Las dos dependencias que agregamos, `lazy_static` y `spin`, son muy útiles en el desarrollo de sistemas operativos y las usaremos en más lugares en futuras publicaciones.

## ¿Qué sigue?
La siguiente publicación explica cómo configurar el framework de pruebas unitarias integrado de Rust. Luego crearemos algunas pruebas unitarias básicas para el módulo del buffer VGA de esta publicación.
