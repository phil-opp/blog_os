+++
title = "Mode Texte VGA"
weight = 3
path = "fr/vga-text-mode"
date = 2018-02-26

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "211f460251cd332905225c93eb66b1aff9f4aefd"
# GitHub usernames of the people that translated this post
translators = ["YaogoGerard"]
+++

Le [mode texte VGA] est une manière simple d'afficher du texte à l'écran. Dans cet article, nous créons une interface qui rend son utilisation sûre et simple en encapsulant toutes les parties non sûres dans un module séparé. Nous implémentons également le support des [macros de formatage] de Rust.

[mode texte VGA]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode
[macros de formatage]: https://doc.rust-lang.org/std/fmt/#related-macros

<!-- more -->

Ce blog est développé ouvertement sur [GitHub]. Si vous avez des problèmes ou des questions, veuillez ouvrir un ticket là-bas. Vous pouvez également laisser des commentaires [en bas de page]. Le code source complet de cet article se trouve dans la branche [`post-03`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[en bas de page]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-03

<!-- toc -->

## Le tampon de texte VGA

Pour afficher un caractère à l'écran en mode texte VGA, il faut l'écrire dans le tampon de texte du matériel VGA. Le tampon de texte VGA est un tableau à deux dimensions typiquement de 25 lignes et 80 colonnes, qui est directement rendu à l'écran. Chaque entrée du tableau décrit un caractère à l'écran via le format suivant :

| Bit(s) | Valeur                      |
| ------ | --------------------------- |
| 0-7    | Point de code ASCII         |
| 8-11   | Couleur de premier plan     |
| 12-14  | Couleur d'arrière-plan      |
| 15     | Clignotement                |

Le premier octet représente le caractère qui doit être affiché dans l'[encodage ASCII]. Pour être plus précis, ce n'est pas exactement l'ASCII, mais un jeu de caractères nommé [_page de codes 437_] avec quelques caractères supplémentaires et de légères modifications. Par souci de simplicité, nous continuerons à l'appeler caractère ASCII dans cet article.

[encodage ASCII]: https://en.wikipedia.org/wiki/ASCII
[_page de codes 437_]: https://en.wikipedia.org/wiki/Code_page_437

Le deuxième octet définit comment le caractère est affiché. Les quatre premiers bits définissent la couleur de premier plan, les trois bits suivants la couleur d'arrière-plan, et le dernier bit si le caractère doit clignoter. Les couleurs suivantes sont disponibles :

| Nombre | Couleur          | Nombre + Bit de Luminosité | Couleur Claire |
| ------ | ---------------- | -------------------------- | -------------- |
| 0x0    | Noir             | 0x8                        | Gris Foncé     |
| 0x1    | Bleu             | 0x9                        | Bleu Clair     |
| 0x2    | Vert             | 0xa                        | Vert Clair     |
| 0x3    | Cyan             | 0xb                        | Cyan Clair     |
| 0x4    | Rouge            | 0xc                        | Rouge Clair    |
| 0x5    | Magenta          | 0xd                        | Rose           |
| 0x6    | Marron           | 0xe                        | Jaune          |
| 0x7    | Gris Clair       | 0xf                        | Blanc          |

Le bit 4 est le _bit de luminosité_, qui transforme, par exemple, le bleu en bleu clair. Pour la couleur d'arrière-plan, ce bit est réutilisé comme bit de clignotement.

Le tampon de texte VGA est accessible via une [entrée-sortie mappée en mémoire] à l'adresse `0xb8000`. Cela signifie que les lectures et écritures à cette adresse n'accèdent pas à la RAM mais accèdent directement au tampon de texte sur le matériel VGA. Cela signifie que nous pouvons le lire et l'écrire via des opérations mémoire normales à cette adresse.

[entrée-sortie mappée en mémoire]: https://en.wikipedia.org/wiki/Memory-mapped_I/O

Notez que le matériel mappé en mémoire peut ne pas supporter toutes les opérations RAM normales. Par exemple, un périphérique pourrait ne supporter que des lectures octet par octet et renvoyer des données incohérentes si un `u64` est lu. Heureusement, le tampon de texte [supporte les lectures et écritures normales], nous n'avons donc pas à le traiter de manière spéciale.

[supporte les lectures et écritures normales]: https://web.stanford.edu/class/cs140/projects/pintos/specs/freevga/vga/vgamem.htm#manip

## Un module Rust

Maintenant que nous savons comment fonctionne le tampon VGA, nous pouvons créer un module Rust pour gérer l'affichage :

```rust
// dans src/main.rs
mod vga_buffer;
```

Pour le contenu de ce module, nous créons un nouveau fichier `src/vga_buffer.rs`. Tout le code ci-dessous va dans notre nouveau module (sauf indication contraire).

### Couleurs

Tout d'abord, nous représentons les différentes couleurs à l'aide d'une énumération :

```rust
// dans src/vga_buffer.rs

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

Nous utilisons ici une [énumération de style C] pour spécifier explicitement le numéro de chaque couleur. Grâce à l'attribut `repr(u8)`, chaque variante de l'énumération est stockée sous forme de `u8`. En réalité, 4 bits seraient suffisants, mais Rust n'a pas de type `u4`.

[énumération de style C]: https://doc.rust-lang.org/rust-by-example/custom_types/enum/c_like.html

Normalement, le compilateur émettrait un avertissement pour chaque variante inutilisée. En utilisant l'attribut `#[allow(dead_code)]`, nous désactivons ces avertissements pour l'énumération `Color`.

En [dérivant] les traits [`Copy`], [`Clone`], [`Debug`], [`PartialEq`] et [`Eq`], nous activons la [sémantique de copie] pour le type et le rendons imprimable et comparable.

[dérivant]: https://doc.rust-lang.org/rust-by-example/trait/derive.html
[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[`Clone`]: https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html
[`Debug`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html
[`PartialEq`]: https://doc.rust-lang.org/nightly/core/cmp/trait.PartialEq.html
[`Eq`]: https://doc.rust-lang.org/nightly/core/cmp/trait.Eq.html
[sémantique de copie]: https://doc.rust-lang.org/1.30.0/book/first-edition/ownership.html#copy-types

Pour représenter un code couleur complet qui spécifie les couleurs de premier plan et d'arrière-plan, nous créons un [newtype] au-dessus de `u8` :

[newtype]: https://doc.rust-lang.org/rust-by-example/generics/new_types.html

```rust
// dans src/vga_buffer.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}
```

La structure `ColorCode` contient l'octet de couleur complet, contenant les couleurs de premier plan et d'arrière-plan. Comme précédemment, nous dérivons les traits `Copy` et `Debug` pour celle-ci. Pour garantir que `ColorCode` a exactement la même disposition de données qu'un `u8`, nous utilisons l'attribut [`repr(transparent)`].

[`repr(transparent)`]: https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent

### Tampon de texte

Nous pouvons maintenant ajouter des structures pour représenter un caractère d'écran et le tampon de texte :

```rust
// dans src/vga_buffer.rs

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

Étant donné que l'ordre des champs dans les structures par défaut est indéfini en Rust, nous avons besoin de l'attribut [`repr(C)`]. Il garantit que les champs de la structure sont disposés exactement comme dans une structure C et garantit ainsi l'ordre correct des champs. Pour la structure `Buffer`, nous utilisons à nouveau [`repr(transparent)`] pour nous assurer qu'elle a la même disposition en mémoire que son champ unique.

[`repr(C)`]: https://doc.rust-lang.org/nightly/nomicon/other-reprs.html#reprc

Pour écrire réellement à l'écran, nous créons maintenant un type writer :

```rust
// dans src/vga_buffer.rs

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}
```

Le writer écrira toujours sur la dernière ligne et décalera les lignes vers le haut lorsqu'une ligne est pleine (ou sur `\n`). Le champ `column_position` garde la trace de la position actuelle dans la dernière ligne. Les couleurs actuelles de premier plan et d'arrière-plan sont spécifiées par `color_code` et une référence au tampon VGA est stockée dans `buffer`. Notez que nous avons besoin d'une [durée de vie explicite] ici pour indiquer au compilateur combien de temps la référence est valide. La durée de vie [`'static`] spécifie que la référence est valide pendant toute la durée d'exécution du programme (ce qui est vrai pour le tampon de texte VGA).

[durée de vie explicite]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#lifetime-annotation-syntax
[`'static`]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime

### Affichage

Nous pouvons maintenant utiliser le `Writer` pour modifier les caractères du tampon. Tout d'abord, nous créons une méthode pour écrire un seul octet ASCII :

```rust
// dans src/vga_buffer.rs

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

Si l'octet est l'octet de [nouvelle ligne] `\n`, le writer n'affiche rien. Au lieu de cela, il appelle une méthode `new_line`, que nous implémenterons plus tard. Les autres octets sont affichés à l'écran dans le deuxième cas `match`.

[nouvelle ligne]: https://en.wikipedia.org/wiki/Newline

Lors de l'affichage d'un octet, le writer vérifie si la ligne actuelle est pleine. Dans ce cas, un appel à `new_line` est utilisé pour passer à la ligne suivante. Ensuite, il écrit un nouveau `ScreenChar` dans le tampon à la position actuelle. Enfin, la position de colonne actuelle est avancée.

Pour afficher des chaînes entières, nous pouvons les convertir en octets et les afficher un par un :

```rust
// dans src/vga_buffer.rs

impl Writer {
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // octet ASCII imprimable ou nouvelle ligne
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // ne fait pas partie de la plage ASCII imprimable
                _ => self.write_byte(0xfe),
            }

        }
    }
}
```

Le tampon de texte VGA ne prend en charge que l'ASCII et les octets supplémentaires de la [page de codes 437]. Les chaînes Rust sont en [UTF-8] par défaut, elles peuvent donc contenir des octets qui ne sont pas pris en charge par le tampon de texte VGA. Nous utilisons un `match` pour différencier les octets ASCII imprimables (une nouvelle ligne ou tout ce qui se trouve entre un caractère espace et un caractère `~`) et les octets non imprimables. Pour les octets non imprimables, nous affichons un caractère `■`, qui a le code hexadécimal `0xfe` sur le matériel VGA.

[page de codes 437]: https://en.wikipedia.org/wiki/Code_page_437
[UTF-8]: https://www.fileformat.info/info/unicode/utf8.htm

#### Essayons !

Pour écrire quelques caractères à l'écran, vous pouvez créer une fonction temporaire :

```rust
// dans src/vga_buffer.rs

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

Elle crée d'abord un nouveau Writer qui pointe vers le tampon VGA à `0xb8000`. La syntaxe pour cela peut sembler un peu étrange : D'abord, nous convertissons l'entier `0xb8000` en [pointeur brut] mutable. Ensuite, nous le convertissons en référence mutable en le déréférençant (via `*`) et en l'empruntant à nouveau immédiatement (via `&mut`). Cette conversion nécessite un [bloc `unsafe`], car le compilateur ne peut pas garantir que le pointeur brut est valide.

[pointeur brut]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#dereferencing-a-raw-pointer
[bloc `unsafe`]: https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html

Ensuite, elle écrit l'octet `b'H'`. Le préfixe `b` crée un [littéral d'octet], qui représente un caractère ASCII. En écrivant les chaînes `"ello "` et `"Wörld!"`, nous testons notre méthode `write_string` et la gestion des caractères non imprimables. Pour voir la sortie, nous devons appeler la fonction `print_something` depuis notre fonction `_start` :

```rust
// dans src/main.rs

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    vga_buffer::print_something();

    loop {}
}
```

Lorsque nous exécutons notre projet maintenant, un `Hello W■■rld!` devrait être affiché dans le coin _inférieur_ gauche de l'écran en jaune :

[littéral d'octet]: https://doc.rust-lang.org/reference/tokens.html#byte-literals

![Sortie QEMU avec un `Hello W■■rld!` jaune dans le coin inférieur gauche](vga-hello.png)

Remarquez que le `ö` est affiché sous forme de deux caractères `■`. C'est parce que `ö` est représenté par deux octets en [UTF-8], qui ne se trouvent pas tous les deux dans la plage ASCII imprimable. En fait, c'est une propriété fondamentale de l'UTF-8 : les octets individuels des valeurs multi-octets ne sont jamais de l'ASCII valide.

### Volatile

Nous venons de voir que notre message a été affiché correctement. Cependant, cela pourrait ne pas fonctionner avec les futurs compilateurs Rust qui optimisent de manière plus agressive.

Le problème est que nous écrivons uniquement dans le `Buffer` et ne le lisons plus jamais. Le compilateur ne sait pas que nous accédons réellement à la mémoire du tampon VGA (au lieu de la RAM normale) et ne sait rien de l'effet secondaire selon lequel certains caractères apparaissent à l'écran. Il pourrait donc décider que ces écritures sont inutiles et peuvent être omises. Pour éviter cette optimisation erronée, nous devons spécifier que ces écritures sont _[volatile]_. Cela indique au compilateur que l'écriture a des effets secondaires et ne doit pas être optimisée.

[volatile]: https://en.wikipedia.org/wiki/Volatile_(computer_programming)

Afin d'utiliser des écritures volatiles pour le tampon VGA, nous utilisons la bibliothèque [volatile][volatile crate]. Cette _crate_ (c'est ainsi que les paquets sont appelés dans le monde Rust) fournit un type wrapper `Volatile` avec des méthodes `read` et `write`. Ces méthodes utilisent en interne les fonctions [read_volatile] et [write_volatile] de la bibliothèque core et garantissent ainsi que les lectures/écritures ne sont pas optimisées.

[volatile crate]: https://docs.rs/volatile
[read_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.read_volatile.html
[write_volatile]: https://doc.rust-lang.org/nightly/core/ptr/fn.write_volatile.html

Nous pouvons ajouter une dépendance à la crate `volatile` en l'ajoutant à la section `dependencies` de notre `Cargo.toml` :

```toml
# dans Cargo.toml

[dependencies]
volatile = "0.2.6"
```

Assurez-vous de spécifier la version `0.2.6` de `volatile`. Les versions plus récentes de la crate ne sont pas compatibles avec cet article.
`0.2.6` est le numéro de version [sémantique]. Pour plus d'informations, consultez le guide [Specifying Dependencies] de la documentation cargo.

[sémantique]: https://semver.org/
[Specifying Dependencies]: https://doc.crates.io/specifying-dependencies.html

Utilisons-la pour rendre les écritures dans le tampon VGA volatiles. Nous mettons à jour notre type `Buffer` comme suit :

```rust
// dans src/vga_buffer.rs

use volatile::Volatile;

struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```

Au lieu d'un `ScreenChar`, nous utilisons maintenant un `Volatile<ScreenChar>`. (Le type `Volatile` est [générique] et peut envelopper (presque) n'importe quel type). Cela garantit que nous ne pouvons pas écrire dedans accidentellement de manière "normale". Au lieu de cela, nous devons maintenant utiliser la méthode `write`.

[générique]: https://doc.rust-lang.org/book/ch10-01-syntax.html

Cela signifie que nous devons mettre à jour notre méthode `Writer::write_byte` :

```rust
// dans src/vga_buffer.rs

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

Au lieu d'une affectation typique utilisant `=`, nous utilisons maintenant la méthode `write`. Maintenant, nous pouvons garantir que le compilateur n'optimisera jamais cette écriture.

### Macros de formatage

Il serait agréable de prendre en charge les macros de formatage de Rust également. De cette façon, nous pouvons facilement afficher différents types, comme des entiers ou des flottants. Pour les prendre en charge, nous devons implémenter le trait [`core::fmt::Write`]. La seule méthode requise de ce trait est `write_str`, qui ressemble beaucoup à notre méthode `write_string`, juste avec un type de retour `fmt::Result` :

[`core::fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

```rust
// dans src/vga_buffer.rs

use core::fmt;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
```

Le `Ok(())` est juste un Result `Ok` contenant le type `()`.

Maintenant, nous pouvons utiliser les macros de formatage intégrées de Rust `write!`/`writeln!` :

```rust
// dans src/vga_buffer.rs

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

Maintenant, vous devriez voir un `Hello! The numbers are 42 and 0.3333333333333333` en bas de l'écran. L'appel à `write!` renvoie un `Result` qui provoque un avertissement s'il n'est pas utilisé, nous appelons donc la fonction [`unwrap`] dessus, qui panique si une erreur se produit. Ce n'est pas un problème dans notre cas, car les écritures dans le tampon VGA n'échouent jamais.

[`unwrap`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.unwrap

### Nouvelles lignes

Pour le moment, nous ignorons simplement les nouvelles lignes et les caractères qui ne rentrent plus dans la ligne. Au lieu de cela, nous voulons déplacer chaque caractère d'une ligne vers le haut (la ligne supérieure est supprimée) et recommencer au début de la dernière ligne. Pour ce faire, nous ajoutons une implémentation pour la méthode `new_line` de `Writer` :

```rust
// dans src/vga_buffer.rs

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

Nous itérons sur tous les caractères de l'écran et déplaçons chaque caractère d'une ligne vers le haut. Notez que la borne supérieure de la notation de plage (`..`) est exclusive. Nous omettons également la 0ème ligne (la première plage commence à `1`) car c'est la ligne qui est décalée hors de l'écran.

Pour terminer le code de nouvelle ligne, nous ajoutons la méthode `clear_row` :

```rust
// dans src/vga_buffer.rs

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

Cette méthode efface une ligne en écrasant tous ses caractères par un caractère espace.

## Une interface globale

Pour fournir un writer global qui peut être utilisé comme interface depuis d'autres modules sans transporter une instance `Writer`, nous essayons de créer un `WRITER` statique :

```rust
// dans src/vga_buffer.rs

pub static WRITER: Writer = Writer {
    column_position: 0,
    color_code: ColorCode::new(Color::Yellow, Color::Black),
    buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
};
```

Cependant, si nous essayons de le compiler maintenant, les erreurs suivantes se produisent :

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

Pour comprendre ce qui se passe ici, nous devons savoir que les statiques sont initialisés au moment de la compilation, contrairement aux variables normales qui sont initialisées au moment de l'exécution. Le composant du compilateur Rust qui évalue ces expressions d'initialisation est appelé le "[const evaluator]". Sa fonctionnalité est encore limitée, mais il y a un travail en cours pour l'étendre, par exemple dans la RFC "[Allow panicking in constants]".

[const evaluator]: https://rustc-dev-guide.rust-lang.org/const-eval.html
[Allow panicking in constants]: https://github.com/rust-lang/rfcs/pull/2345

Le problème avec `ColorCode::new` serait résoluble en utilisant des [fonctions `const`], mais le problème fondamental ici est que l'évaluateur const de Rust n'est pas capable de convertir les pointeurs bruts en références au moment de la compilation. Peut-être que cela fonctionnera un jour, mais d'ici là, nous devons trouver une autre solution.

[fonctions `const`]: https://doc.rust-lang.org/reference/const_eval.html#const-functions

### Lazy Statics

L'initialisation unique de statiques avec des fonctions non-const est un problème courant en Rust. Heureusement, il existe déjà une bonne solution dans une crate nommée [lazy_static]. Cette crate fournit une macro `lazy_static!` qui définit un `static` initialisé paresseusement. Au lieu de calculer sa valeur au moment de la compilation, le `static` s'initialise paresseusement lorsqu'il est accédé pour la première fois. Ainsi, l'initialisation se produit au moment de l'exécution, de sorte qu'un code d'initialisation arbitrairement complexe est possible.

[lazy_static]: https://docs.rs/lazy_static/1.0.1/lazy_static/

Ajoutons la crate `lazy_static` à notre projet :

```toml
# dans Cargo.toml

[dependencies.lazy_static]
version = "1.0"
features = ["spin_no_std"]
```

Nous avons besoin de la fonctionnalité `spin_no_std`, car nous ne lions pas la bibliothèque standard.

Avec `lazy_static`, nous pouvons définir notre `WRITER` statique sans problème :

```rust
// dans src/vga_buffer.rs

use lazy_static::lazy_static;

lazy_static! {
    pub static ref WRITER: Writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };
}
```

Cependant, ce `WRITER` est assez inutile car il est immuable. Cela signifie que nous ne pouvons rien y écrire (puisque toutes les méthodes d'écriture prennent `&mut self`). Une solution possible serait d'utiliser un [static mutable]. Mais alors chaque lecture et écriture serait unsafe car cela pourrait facilement introduire des courses de données et d'autres mauvaises choses. L'utilisation de `static mut` est fortement déconseillée. Il y a même eu des propositions pour [le supprimer][remove static mut]. Mais quelles sont les alternatives ? Nous pourrions essayer d'utiliser un static immuable avec un type de cellule comme [RefCell] ou même [UnsafeCell] qui fournit une [mutabilité intérieure]. Mais ces types ne sont pas [Sync] (pour de bonnes raisons), nous ne pouvons donc pas les utiliser dans des statiques.

[static mutable]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable
[remove static mut]: https://internals.rust-lang.org/t/pre-rfc-remove-static-mut/1437
[RefCell]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html#keeping-track-of-borrows-at-runtime-with-refcellt
[UnsafeCell]: https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html
[mutabilité intérieure]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
[Sync]: https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html

### Spinlocks

Pour obtenir une mutabilité intérieure synchronisée, les utilisateurs de la bibliothèque standard peuvent utiliser [Mutex]. Il fournit une exclusion mutuelle en bloquant les threads lorsque la ressource est déjà verrouillée. Mais notre noyau de base n'a aucun support de blocage ni même de concept de threads, nous ne pouvons donc pas l'utiliser non plus. Cependant, il existe un type de mutex très basique en informatique qui ne nécessite aucune fonctionnalité du système d'exploitation : le [spinlock]. Au lieu de bloquer, les threads essaient simplement de le verrouiller encore et encore dans une boucle serrée, brûlant ainsi du temps CPU jusqu'à ce que le mutex soit à nouveau libre.

[Mutex]: https://doc.rust-lang.org/nightly/std/sync/struct.Mutex.html
[spinlock]: https://en.wikipedia.org/wiki/Spinlock

Pour utiliser un mutex tournant, nous pouvons ajouter la [crate spin] comme dépendance :

[crate spin]: https://crates.io/crates/spin

```toml
# dans Cargo.toml
[dependencies]
spin = "0.5.2"
```

Ensuite, nous pouvons utiliser le mutex tournant pour ajouter une [mutabilité intérieure] sûre à notre `WRITER` statique :

```rust
// dans src/vga_buffer.rs

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

Maintenant, nous pouvons supprimer la fonction `print_something` et afficher directement depuis notre fonction `_start` :

```rust
// dans src/main.rs
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    write!(vga_buffer::WRITER.lock(), ", some numbers: {} {}", 42, 1.337).unwrap();

    loop {}
}
```

Nous devons importer le trait `fmt::Write` pour pouvoir utiliser ses fonctions.

### Sécurité

Notez que nous n'avons qu'un seul bloc unsafe dans notre code, qui est nécessaire pour créer une référence `Buffer` pointant vers `0xb8000`. Ensuite, toutes les opérations sont sûres. Rust utilise la vérification des limites pour les accès aux tableaux par défaut, nous ne pouvons donc pas écrire accidentellement en dehors du tampon. Ainsi, nous avons encodé les conditions requises dans le système de types et sommes capables de fournir une interface sûre vers l'extérieur.

### Une macro println

Maintenant que nous avons un writer global, nous pouvons ajouter une macro `println` qui peut être utilisée n'importe où dans la base de code. La [syntaxe de macro] de Rust est un peu étrange, nous n'essaierons donc pas d'écrire une macro à partir de zéro. Au lieu de cela, nous regardons la source de la [macro `println!`] dans la bibliothèque standard :

[syntaxe de macro]: https://doc.rust-lang.org/nightly/book/ch20-05-macros.html#declarative-macros-for-general-metaprogramming
[macro `println!`]: https://doc.rust-lang.org/nightly/std/macro.println!.html

```rust
#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}
```

Les macros sont définies par une ou plusieurs règles, similaires aux branches `match`. La macro `println` a deux règles : La première règle est pour les invocations sans arguments, par exemple `println!()`, qui est développée en `print!("\n")` et affiche donc juste une nouvelle ligne. La deuxième règle est pour les invocations avec des paramètres tels que `println!("Hello")` ou `println!("Number: {}", 4)`. Elle est également développée en une invocation de la macro `print!`, passant tous les arguments et une nouvelle ligne supplémentaire `\n` à la fin.

L'attribut `#[macro_export]` rend la macro disponible pour toute la crate (pas seulement le module dans lequel elle est définie) et les crates externes. Il place également la macro à la racine de la crate, ce qui signifie que nous devons importer la macro via `use std::println` au lieu de `std::macros::println`.

La [macro `print!`] est définie comme :

[macro `print!`]: https://doc.rust-lang.org/nightly/std/macro.print!.html

```rust
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
```

La macro se développe en un appel de la [fonction `_print`] dans le module `io`. La [variable `$crate`] garantit que la macro fonctionne également en dehors de la crate `std` en se développant en `std` lorsqu'elle est utilisée dans d'autres crates.

La [macro `format_args`] construit un type [fmt::Arguments] à partir des arguments passés, qui est transmis à `_print`. La [fonction `_print`] de libstd appelle `print_to`, qui est assez compliquée car elle prend en charge différents périphériques `Stdout`. Nous n'avons pas besoin de cette complexité car nous voulons simplement afficher sur le tampon VGA.

[fonction `_print`]: https://github.com/rust-lang/rust/blob/29f5c699b11a6a148f097f82eaa05202f8799bbc/src/libstd/io/stdio.rs#L698
[variable `$crate`]: https://doc.rust-lang.org/1.30.0/book/first-edition/macros.html#the-variable-crate
[macro `format_args`]: https://doc.rust-lang.org/nightly/std/macro.format_args.html
[fmt::Arguments]: https://doc.rust-lang.org/nightly/core/fmt/struct.Arguments.html

Pour afficher sur le tampon VGA, nous copions simplement les macros `println!` et `print!`, mais les modifions pour utiliser notre propre fonction `_print` :

```rust
// dans src/vga_buffer.rs

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

Une chose que nous avons changée par rapport à la définition originale de `println` est que nous avons préfixé les invocations de la macro `print!` avec `$crate` également. Cela garantit que nous n'avons pas besoin d'importer la macro `print!` aussi si nous voulons seulement utiliser `println`.

Comme dans la bibliothèque standard, nous ajoutons l'attribut `#[macro_export]` aux deux macros pour les rendre disponibles partout dans notre crate. Notez que cela place les macros dans l'espace de noms racine de la crate, donc les importer via `use crate::vga_buffer::println` ne fonctionne pas. Au lieu de cela, nous devons faire `use crate::println`.

La fonction `_print` verrouille notre `WRITER` statique et appelle la méthode `write_fmt` dessus. Cette méthode provient du trait `Write`, que nous devons importer. Le `unwrap()` supplémentaire à la fin panique si l'affichage n'est pas réussi. Mais puisque nous retournons toujours `Ok` dans `write_str`, cela ne devrait pas se produire.

Comme les macros doivent pouvoir appeler `_print` depuis l'extérieur du module, la fonction doit être publique. Cependant, puisque nous considérons cela comme un détail d'implémentation privé, nous ajoutons l'[attribut `doc(hidden)`] pour le masquer de la documentation générée.

[attribut `doc(hidden)`]: https://doc.rust-lang.org/nightly/rustdoc/write-documentation/the-doc-attribute.html#hidden

### Hello World en utilisant `println`

Maintenant, nous pouvons utiliser `println` dans notre fonction `_start` :

```rust
// dans src/main.rs

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    loop {}
}
```

Notez que nous n'avons pas besoin d'importer la macro dans la fonction main, car elle vit déjà dans l'espace de noms racine.

Comme prévu, nous voyons maintenant un _"Hello World!"_ à l'écran :

![QEMU affichant "Hello World!"](vga-hello-world.png)

### Affichage des messages de panique

Maintenant que nous avons une macro `println`, nous pouvons l'utiliser dans notre fonction de panique pour afficher le message de panique et l'emplacement de la panique :

```rust
// dans main.rs

/// Cette fonction est appelée en cas de panique.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
```

Lorsque nous insérons maintenant `panic!("Some panic message");` dans notre fonction `_start`, nous obtenons la sortie suivante :

![QEMU affichant "panicked at 'Some panic message', src/main.rs:28:5"](vga-panic.png)

Nous savons donc non seulement qu'une panique s'est produite, mais aussi le message de panique et où dans le code cela s'est produit.

## Résumé

Dans cet article, nous avons appris la structure du tampon de texte VGA et comment il peut être écrit via le mappage mémoire à l'adresse `0xb8000`. Nous avons créé un module Rust qui encapsule le caractère unsafe de l'écriture dans ce tampon mappé en mémoire et présente une interface sûre et pratique vers l'extérieur.

Grâce à cargo, nous avons également vu à quel point il est facile d'ajouter des dépendances à des bibliothèques tierces. Les deux dépendances que nous avons ajoutées, `lazy_static` et `spin`, sont très utiles dans le développement d'OS et nous les utiliserons dans plus d'endroits dans les futurs articles.

## Et ensuite ?

Le prochain article explique comment configurer le framework de tests unitaires intégré de Rust. Nous créerons ensuite quelques tests unitaires de base pour le module de tampon VGA de cet article.