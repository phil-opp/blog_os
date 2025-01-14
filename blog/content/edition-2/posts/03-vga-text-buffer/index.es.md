+++
title = "Modo de Texto VGA"
weight = 3
path = "modo-texto-vga"
date  = 2018-02-26

[extra]
chapter = "Fundamentos"
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

Bit(s) | Valor
------ | ----------------
0-7    | Código de punto ASCII
8-11   | Color de primer plano
12-14  | Color de fondo
15     | Parpadeo

El primer byte representa el carácter que debe imprimirse en la [codificación ASCII]. Para ser más específicos, no es exactamente ASCII, sino un conjunto de caracteres llamado [_página de códigos 437_] con algunos caracteres adicionales y ligeras modificaciones. Para simplificar, procederemos a llamarlo un carácter ASCII en esta publicación.

[codificación ASCII]: https://en.wikipedia.org/wiki/ASCII
[_página de códigos 437_]: https://en.wikipedia.org/wiki/Code_page_437

El segundo byte define cómo se muestra el carácter. Los primeros cuatro bits definen el color de primer plano, los siguientes tres bits el color de fondo, y el último bit si el carácter debe parpadear. Los siguientes colores están disponibles:

Número | Color      | Número + Bit de Brillo | Color Brillante
------ | ---------- | ---------------------- | -------------
0x0    | Negro      | 0x8                    | Gris Oscuro
0x1    | Azul       | 0x9                    | Azul Claro
0x2    | Verde      | 0xa                    | Verde Claro
0x3    | Cian       | 0xb                    | Cian Claro
0x4    | Rojo       | 0xc                    | Rojo Claro
0x5    | Magenta    | 0xd                    | Magenta Claro
0x6    | Marrón     | 0xe                    | Amarillo
0x7    | Gris Claro | 0xf                    | Blanco

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

[puntero sin procesar]: https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
[bloque `unsafe`]: https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html

Luego escribe el byte `b'H'` en él. El prefijo `b` crea un [literal de byte], que representa un carácter ASCII. Al escribir las cadenas `"ello "` y `"Wörld!"`, probamos nuestro método `write_string` y el manejo de caracteres no imprimibles. Para ver la salida, necesitamos llamar a la función `print_something` desde nuestra función `_start`:

```rust
// en src/main.rs

#[no_mangle]
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
