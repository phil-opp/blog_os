+++
title = "Tests"
weight = 4
path = "fr/testing"
date = 2019-04-27

[extra]
chapter = "Bare Bones"
comments_search_term = 1009
+++

Cet article explore les tests unitaires et les tests d'intégration dans les exécutables `no_std`. Nous utiliserons le support de Rust pour les frameworks de test personnalisés afin d'exécuter des fonctions de test à l'intérieur de notre noyau. Pour transmettre les résultats hors de QEMU, nous utiliserons différentes fonctionnalités de QEMU ainsi que l'outil `bootimage`.

<!-- more -->

Ce blog est développé de manière ouverte sur [GitHub]. Si vous rencontrez des problèmes ou avez des questions, n'hésitez pas à ouvrir une issue à cet endroit. Vous pouvez également laisser des commentaires [en bas de page]. Le code source complet de cet article se trouve dans la branche [`post-04`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[en bas de page]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-04

<!-- toc -->

## Prérequis

Cet article remplace les articles (désormais obsolètes) [_Tests unitaires_] et [_Tests d'intégration_]. Il suppose que vous avez suivi l'article [_Un noyau Rust minimal_] publié après le 27/04/2019. Concrètement, vous devez disposer d'un fichier `.cargo/config.toml` qui [définit une cible par défaut] et [définit un exécutable d'exécution (runner)].

[_Tests unitaires_]: @/edition-2/posts/deprecated/04-unit-testing/index.md
[_Tests d'intégration_]: @/edition-2/posts/deprecated/05-integration-tests/index.md
[_Un noyau Rust minimal_]: @/edition-2/posts/02-minimal-rust-kernel/index.md
[définit une cible par défaut]: @/edition-2/posts/02-minimal-rust-kernel/index.md#set-a-default-target
[définit un exécutable d'exécution (runner)]: @/edition-2/posts/02-minimal-rust-kernel/index.md#using-cargo-run

## Les tests en Rust

Rust dispose d'un [framework de test intégré] capable d'exécuter des tests unitaires sans aucune configuration préalable. Il suffit de créer une fonction qui vérifie certains résultats à l'aide d'assertions et d'ajouter l'attribut `#[test]` à l'en-tête de la fonction. Ensuite, `cargo test` trouvera et exécutera automatiquement toutes les fonctions de test de votre crate.

[framework de test intégré]: https://doc.rust-lang.org/book/ch11-00-testing.html

Pour activer les tests sur notre binaire noyau, nous pouvons définir le champ `test` à `true` dans le Cargo.toml :

```toml
# in Cargo.toml

[[bin]]
name = "blog_os"
test = true
bench = false
```

Cette [section `[[bin]]`](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#configuring-a-target) indique à `cargo` comment compiler notre exécutable `blog_os`.
Le champ `test` précise si les tests sont pris en charge pour cet exécutable.
Nous avions défini `test = false` dans le premier article afin de [satisfaire `rust-analyzer`](@/edition-2/posts/01-freestanding-rust-binary/index.md#making-rust-analyzer-happy), mais nous souhaitons maintenant activer les tests, nous le remettons donc à `true`.

Malheureusement, les tests sont un peu plus compliqués pour les applications `no_std` comme notre noyau. Le problème vient du fait que le framework de test de Rust utilise implicitement la bibliothèque intégrée [`test`], laquelle dépend de la bibliothèque standard. Cela signifie que nous ne pouvons pas utiliser le framework de test par défaut pour notre noyau `#[no_std]`.

[`test`]: https://doc.rust-lang.org/test/index.html

On peut le constater en essayant d'exécuter `cargo test` dans notre projet :

```
> cargo test
   Compiling blog_os v0.1.0 (/…/blog_os)
error[E0463]: can't find crate for `test`
```

Comme la crate `test` dépend de la bibliothèque standard, elle n'est pas disponible pour notre cible bare metal. Bien qu'il soit [possible][utest] de porter la crate `test` dans un contexte `#[no_std]`, cela reste très instable et nécessite des bidouillages, comme la redéfinition de la macro `panic`.

[utest]: https://github.com/japaric/utest

### Frameworks de test personnalisés

Heureusement, Rust permet de remplacer le framework de test par défaut grâce à la fonctionnalité instable [`custom_test_frameworks`]. Cette fonctionnalité ne nécessite aucune bibliothèque externe et fonctionne donc aussi dans les environnements `#[no_std]`. Son principe est de collecter toutes les fonctions annotées avec l'attribut `#[test_case]`, puis d'appeler une fonction runner définie par l'utilisateur en lui passant la liste des tests en argument. Cela donne à l'implémentation un contrôle maximal sur le processus de test.

[`custom_test_frameworks`]: https://doc.rust-lang.org/unstable-book/language-features/custom-test-frameworks.html

L'inconvénient par rapport au framework de test par défaut est que de nombreuses fonctionnalités avancées, comme les [tests `should_panic`], ne sont pas disponibles. C'est donc à l'implémentation de fournir elle-même ce type de fonctionnalité si besoin. Cela nous convient parfaitement puisque nous évoluons dans un environnement d'exécution très particulier, où les implémentations par défaut de ces fonctionnalités avancées ne fonctionneraient probablement pas de toute façon. Par exemple, l'attribut `#[should_panic]` s'appuie sur le déroulement de la pile (stack unwinding) pour intercepter les panics, fonctionnalité que nous avons désactivée pour notre noyau.

[tests `should_panic`]: https://doc.rust-lang.org/book/ch11-01-writing-tests.html#checking-for-panics-with-should_panic

Pour mettre en place un framework de test personnalisé dans notre noyau, nous ajoutons ce qui suit à notre `main.rs` :

```rust
// in src/main.rs

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

Notre runner se contente d'afficher un court message de débogage, puis appelle chaque fonction de test de la liste. Le type d'argument `&[&dyn Fn()]` est une [_slice_] de références à des [_objets trait_] du trait [_Fn()_]. Il s'agit essentiellement d'une liste de références vers des types pouvant être appelés comme des fonctions. Comme cette fonction est inutile en dehors des tests, nous utilisons l'attribut `#[cfg(test)]` pour ne l'inclure que dans ce contexte.

[_slice_]: https://doc.rust-lang.org/std/primitive.slice.html
[_objets trait_]: https://doc.rust-lang.org/1.30.0/book/first-edition/trait-objects.html
[_Fn()_]: https://doc.rust-lang.org/std/ops/trait.Fn.html

Si nous exécutons `cargo test` maintenant, nous constatons que la compilation réussit (si ce n'est pas le cas, voir la remarque ci-dessous). Cependant, nous voyons toujours notre « Hello World » au lieu du message de notre `test_runner`. La raison est que notre fonction `_start` reste utilisée comme point d'entrée. La fonctionnalité des frameworks de test personnalisés génère une fonction `main` qui appelle `test_runner`, mais cette fonction est ignorée car nous utilisons l'attribut `#[no_main]` et fournissons notre propre point d'entrée.

<div class = "warning">

**Remarque :** Il existe actuellement un bug dans cargo qui provoque des erreurs de type « duplicate lang item » lors de l'exécution de `cargo test` dans certains cas. Cela se produit lorsque vous avez défini `panic = "abort"` pour un profil dans votre `Cargo.toml`. Essayez de le supprimer, `cargo test` devrait alors fonctionner. Si cela ne suffit pas, ajoutez `panic-abort-tests = true` à la section `[unstable]` de votre fichier `.cargo/config.toml`. Consultez [l'issue cargo correspondante](https://github.com/rust-lang/cargo/issues/7359) pour plus de détails.

</div>

Pour corriger cela, nous devons d'abord renommer la fonction générée en autre chose que `main` grâce à l'attribut `reexport_test_harness_main`. Nous pouvons ensuite appeler cette fonction renommée depuis notre fonction `_start` :

```rust
// in src/main.rs

#![reexport_test_harness_main = "test_main"]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}
```

Nous donnons le nom `test_main` à la fonction d'entrée du framework de test et l'appelons depuis notre point d'entrée `_start`. Nous utilisons la [compilation conditionnelle] pour n'ajouter l'appel à `test_main` que dans un contexte de test, puisque cette fonction n'est pas générée lors d'une exécution normale.

Lorsque nous exécutons maintenant `cargo test`, le message « Running 0 tests » de notre `test_runner` s'affiche à l'écran. Nous sommes maintenant prêts à créer notre première fonction de test :

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}
```

Lorsque nous exécutons `cargo test` maintenant, nous obtenons la sortie suivante :

![QEMU affichant "Hello World!", "Running 1 tests" et "trivial assertion... [ok]"](qemu-test-runner-output.png)

La slice `tests` passée à notre fonction `test_runner` contient désormais une référence à la fonction `trivial_assertion`. Le message `trivial assertion... [ok]` affiché à l'écran nous indique que le test a bien été appelé et qu'il a réussi.

Après l'exécution des tests, notre `test_runner` retourne à la fonction `test_main`, qui elle-même retourne à notre fonction de point d'entrée `_start`. À la fin de `_start`, nous entrons dans une boucle infinie, car la fonction de point d'entrée n'est pas autorisée à retourner. Cela pose problème, car nous voulons que `cargo test` se termine une fois tous les tests exécutés.

## Quitter QEMU

Pour l'instant, nous avons une boucle infinie à la fin de notre fonction `_start` et devons fermer QEMU manuellement à chaque exécution de `cargo test`. C'est problématique, car nous souhaitons également pouvoir exécuter `cargo test` dans des scripts, sans intervention de l'utilisateur. La solution propre serait d'implémenter un véritable mécanisme d'arrêt de notre système d'exploitation. Malheureusement, cela reste relativement complexe, car cela nécessite d'implémenter le support du standard de gestion de l'alimentation [APM] ou [ACPI].

[APM]: https://wiki.osdev.org/APM
[ACPI]: https://wiki.osdev.org/ACPI

Heureusement, il existe une échappatoire : QEMU propose un dispositif spécial appelé `isa-debug-exit`, qui permet de quitter facilement QEMU depuis le système invité. Pour l'activer, nous devons passer un argument `-device` à QEMU. Nous pouvons le faire en ajoutant une clé de configuration `package.metadata.bootimage.test-args` dans notre `Cargo.toml` :

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
```

Le `bootimage runner` ajoute les `test-args` à la commande QEMU par défaut pour tous les exécutables de test. Pour un `cargo run` normal, ces arguments sont ignorés.

En plus du nom du dispositif (`isa-debug-exit`), nous passons deux paramètres, `iobase` et `iosize`, qui précisent le _port d'E/S_ par lequel le dispositif peut être atteint depuis notre noyau.

### Les ports d'E/S

Il existe deux approches différentes pour la communication entre le CPU et les périphériques matériels sur x86 : les **E/S mappées en mémoire** (memory-mapped I/O) et les **E/S mappées en ports** (port-mapped I/O). Nous avons déjà utilisé les E/S mappées en mémoire pour accéder au [tampon texte VGA] via l'adresse mémoire `0xb8000`. Cette adresse n'est pas associée à la RAM, mais à une mémoire du périphérique VGA.

[tampon texte VGA]: @/edition-2/posts/03-vga-text-buffer/index.md

À l'inverse, les E/S mappées en ports utilisent un bus d'E/S distinct pour la communication. Chaque périphérique connecté possède un ou plusieurs numéros de port. Pour communiquer avec un tel port d'E/S, il existe des instructions CPU spéciales appelées `in` et `out`, qui prennent un numéro de port et un octet de données en paramètres (il existe aussi des variantes de ces instructions permettant d'envoyer un `u16` ou un `u32`).

Le dispositif `isa-debug-exit` utilise les E/S mappées en ports. Le paramètre `iobase` précise sur quelle adresse de port le dispositif doit se trouver (`0xf4` est un port [généralement inutilisé][list of x86 I/O ports] du bus d'E/S x86), et `iosize` précise la taille du port (`0x04` signifie quatre octets).

[list of x86 I/O ports]: https://wiki.osdev.org/I/O_Ports#The_list

### Utiliser le dispositif de sortie

Le fonctionnement du dispositif `isa-debug-exit` est très simple. Lorsqu'une `valeur` est écrite sur le port d'E/S désigné par `iobase`, cela provoque la fermeture de QEMU avec le [code de sortie] `(valeur << 1) | 1`. Ainsi, lorsque nous écrivons `0` sur le port, QEMU se ferme avec le code de sortie `(0 << 1) | 1 = 1`, et lorsque nous écrivons `1`, il se ferme avec le code `(1 << 1) | 1 = 3`.

[code de sortie]: https://en.wikipedia.org/wiki/Exit_status

Plutôt que d'invoquer manuellement les instructions assembleur `in` et `out`, nous utilisons les abstractions fournies par la crate [`x86_64`]. Pour ajouter une dépendance à cette crate, nous l'ajoutons à la section `dependencies` de notre `Cargo.toml` :

[`x86_64`]: https://docs.rs/x86_64/0.14.2/x86_64/

```toml
# in Cargo.toml

[dependencies]
x86_64 = "0.14.2"
```

Nous pouvons maintenant utiliser le type [`Port`] fourni par la crate pour créer une fonction `exit_qemu` :

[`Port`]: https://docs.rs/x86_64/0.14.2/x86_64/instructions/port/struct.Port.html

```rust
// in src/main.rs

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

La fonction crée un nouveau [`Port`] à l'adresse `0xf4`, qui correspond à l'`iobase` du dispositif `isa-debug-exit`. Elle écrit ensuite le code de sortie passé en paramètre sur le port. Nous utilisons un `u32` car nous avons défini l'`iosize` du dispositif `isa-debug-exit` à 4 octets. Les deux opérations sont marquées `unsafe`, car écrire sur un port d'E/S peut, de manière générale, entraîner un comportement arbitraire.

Pour préciser le statut de sortie, nous créons une énumération `QemuExitCode`. L'idée est de quitter avec le code de succès si tous les tests ont réussi, et avec le code d'échec sinon. L'énumération est marquée `#[repr(u32)]` afin que chaque variante soit représentée par un entier `u32`. Nous utilisons le code de sortie `0x10` pour le succès et `0x11` pour l'échec. Les valeurs exactes des codes de sortie importent peu, tant qu'elles n'entrent pas en conflit avec les codes de sortie par défaut de QEMU. Par exemple, utiliser le code `0` pour le succès ne serait pas une bonne idée, car il deviendrait `(0 << 1) | 1 = 1` après transformation, ce qui correspond au code de sortie par défaut lorsque QEMU échoue à démarrer. Nous ne pourrions alors plus distinguer une erreur QEMU d'une exécution de test réussie.

Nous pouvons maintenant mettre à jour notre `test_runner` pour qu'il quitte QEMU une fois tous les tests exécutés :

```rust
// in src/main.rs

fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    /// new
    exit_qemu(QemuExitCode::Success);
}
```

Lorsque nous exécutons `cargo test` maintenant, nous constatons que QEMU se ferme immédiatement après l'exécution des tests. Le problème est que `cargo test` interprète le test comme un échec, même si nous avons transmis notre code de sortie `Success` :

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

Le problème est que `cargo test` considère tout code d'erreur différent de `0` comme un échec.

### Code de sortie de succès

Pour contourner ce problème, `bootimage` fournit une clé de configuration `test-success-exit-code` qui permet d'associer un code de sortie donné au code de sortie `0` :

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = […]
test-success-exit-code = 33         # (0x10 << 1) | 1
```

Avec cette configuration, `bootimage` fait correspondre notre code de sortie de succès au code de sortie 0, de sorte que `cargo test` reconnaît correctement le cas de succès et ne compte plus le test comme un échec.

Notre test runner ferme désormais automatiquement QEMU et rapporte correctement les résultats des tests. La fenêtre QEMU reste visible très brièvement, mais pas assez longtemps pour en lire les résultats. Il serait donc utile de pouvoir afficher les résultats des tests directement dans la console, afin de pouvoir les consulter même après la fermeture de QEMU.

## Afficher les résultats dans la console

Pour voir la sortie des tests dans la console, nous devons trouver un moyen de transmettre les données de notre noyau vers le système hôte. Il existe plusieurs façons d'y parvenir, par exemple en envoyant les données via une interface réseau TCP. Cependant, la mise en place d'une pile réseau est une tâche assez complexe, nous allons donc opter pour une solution plus simple.

### Le port série

Une manière simple d'envoyer les données consiste à utiliser le [port série], une interface ancienne que l'on ne trouve plus sur les ordinateurs modernes. Il est facile à programmer, et QEMU peut rediriger les octets envoyés via le port série vers la sortie standard de l'hôte ou vers un fichier.

[port série]: https://en.wikipedia.org/wiki/Serial_port

Les puces qui implémentent une interface série sont appelées [UART]. Il existe [de nombreux modèles d'UART] sur x86, mais heureusement, les différences entre eux se limitent à quelques fonctionnalités avancées dont nous n'avons pas besoin. Les UART courants aujourd'hui sont tous compatibles avec l'[UART 16550], nous utiliserons donc ce modèle pour notre framework de test.

[UART]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter
[de nombreux modèles d'UART]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter#Models
[UART 16550]: https://en.wikipedia.org/wiki/16550_UART

Nous utiliserons la crate [`uart_16550`] pour initialiser l'UART et envoyer des données via le port série. Pour l'ajouter en tant que dépendance, nous mettons à jour notre `Cargo.toml` et notre `main.rs` :

[`uart_16550`]: https://docs.rs/uart_16550

```toml
# in Cargo.toml

[dependencies]
uart_16550 = "0.6.0"
```

La crate `uart_16550` contient un type [`Uart16550Tty`](https://docs.rs/uart_16550/latest/uart_16550/struct.Uart16550Tty.html) qui initialise l'UART en mode [TTY](https://en.wikipedia.org/wiki/Teleprinter), ce qui nous permet d'envoyer facilement du texte.

Utilisons ce type dans un nouveau module `serial` :

```rust
// in src/main.rs

mod serial;
```

```rust
// in src/serial.rs

use uart_16550::{Config, Uart16550Tty, backend::PioBackend};
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL1: Mutex<Uart16550Tty<PioBackend>> = Mutex::new(unsafe {
        Uart16550Tty::new_port(0x3F8, Config::default())
            .expect("failed to initialize UART")
    });
}
```

Comme pour le [tampon texte VGA][vga lazy-static], nous utilisons `lazy_static` ainsi qu'un spinlock pour créer une instance `static` de notre writer. L'utilisation de `lazy_static` nous garantit que l'UART n'est initialisé qu'une seule fois, lors de sa première utilisation.

Comme le dispositif `isa-debug-exit`, l'UART est programmé via des E/S mappées en ports, ce qu'indique le paramètre [`PioBackend`](https://docs.rs/uart_16550/latest/uart_16550/backend/struct.PioBackend.html). L'UART étant plus complexe, il utilise plusieurs ports d'E/S pour programmer différents registres du périphérique. La fonction `unsafe` `Uart16550Tty::new_port` attend en argument l'adresse du premier port d'E/S de l'UART, à partir de laquelle elle peut calculer les adresses de tous les ports nécessaires. Nous lui passons l'adresse de port `0x3F8`, qui est le numéro de port standard pour la première interface série.

[vga lazy-static]: @/edition-2/posts/03-vga-text-buffer/index.md#lazy-statics

Pour rendre le port série facilement utilisable, nous ajoutons les macros `serial_print!` et `serial_println!` :

```rust
// in src/serial.rs

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
```

Cette implémentation est très similaire à celle de nos macros `print` et `println`. Comme le type `Uart16550Tty` implémente déjà le trait [`fmt::Write`], nous n'avons pas besoin de fournir notre propre implémentation.

[`fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

Nous pouvons maintenant afficher les résultats sur l'interface série plutôt que sur le tampon texte VGA dans notre code de test :

```rust
// in src/main.rs

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

Notez que la macro `serial_println` se trouve directement à la racine de l'espace de noms, car nous avons utilisé l'attribut `#[macro_export]` ; l'importer via `use crate::serial::serial_println` ne fonctionnera donc pas.

### Arguments QEMU

Pour voir la sortie série de QEMU, nous devons utiliser l'argument `-serial` afin de rediriger la sortie vers stdout :

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio"
]
```

Lorsque nous exécutons `cargo test` maintenant, nous voyons directement la sortie des tests dans la console :

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

Cependant, lorsqu'un test échoue, nous voyons toujours la sortie dans la fenêtre QEMU, car notre gestionnaire de panic utilise toujours `println`. Pour simuler cela, nous pouvons remplacer l'assertion de notre test `trivial_assertion` par `assert_eq!(0, 1)` :

![QEMU affichant "Hello World!" puis "panicked at 'assertion failed: `(left == right)`
    left: `0`, right: `1`', src/main.rs:55:5"](qemu-failed-test.png)

Nous constatons que le message de panic s'affiche toujours dans le tampon VGA, tandis que le reste de la sortie des tests s'affiche sur le port série. Le message de panic étant très utile, il serait intéressant de pouvoir le voir également dans la console.

### Afficher un message d'erreur en cas de panic

Pour quitter QEMU avec un message d'erreur en cas de panic, nous pouvons utiliser la [compilation conditionnelle] afin d'utiliser un gestionnaire de panic différent en mode test :

[compilation conditionnelle]: https://doc.rust-lang.org/1.30.0/book/first-edition/conditional-compilation.html

```rust
// in src/main.rs

// our existing panic handler
#[cfg(not(test))] // nouvel attribut
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

// our panic handler in test mode
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}
```

Pour notre gestionnaire de panic en mode test, nous utilisons `serial_println` au lieu de `println`, puis nous quittons QEMU avec un code de sortie d'échec. Notez que nous avons toujours besoin d'une boucle infinie après l'appel à `exit_qemu`, car le compilateur ignore que le dispositif `isa-debug-exit` provoque la fermeture du programme.

Désormais, QEMU se ferme également en cas d'échec des tests et affiche un message d'erreur utile dans la console :

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

Comme tous les résultats des tests s'affichent désormais dans la console, nous n'avons plus besoin de la fenêtre QEMU qui s'ouvre brièvement. Nous pouvons donc la masquer entièrement.

### Masquer QEMU

Puisque nous obtenons désormais l'ensemble des résultats de test grâce au dispositif `isa-debug-exit` et au port série, la fenêtre QEMU ne nous est plus utile. Nous pouvons la masquer en passant l'argument `-display none` à QEMU :

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio",
    "-display", "none"
]
```

Désormais, QEMU s'exécute entièrement en arrière-plan et aucune fenêtre ne s'ouvre plus. C'est non seulement moins gênant, mais cela permet aussi à notre framework de test de fonctionner dans des environnements dépourvus d'interface graphique, comme les services d'intégration continue (CI) ou les connexions [SSH].

[SSH]: https://en.wikipedia.org/wiki/Secure_Shell

### Délais d'expiration (timeouts)

Comme `cargo test` attend que le test runner se termine, un test qui ne retourne jamais peut bloquer le test runner indéfiniment. C'est regrettable, mais cela ne pose pas de problème majeur en pratique, car il est généralement facile d'éviter les boucles infinies. Dans notre cas, cependant, des boucles infinies peuvent survenir dans différentes situations :

- Le bootloader échoue à charger notre noyau, ce qui provoque un redémarrage infini du système.
- Le firmware BIOS/UEFI échoue à charger le bootloader, ce qui provoque le même redémarrage infini.
- Le CPU entre dans une instruction `loop {}` à la fin de l'une de nos fonctions, par exemple parce que le dispositif de sortie de QEMU ne fonctionne pas correctement.
- Le matériel provoque une réinitialisation du système, par exemple lorsqu'une exception CPU n'est pas interceptée (nous expliquerons cela dans un prochain article).

Comme les boucles infinies peuvent survenir dans de nombreuses situations, l'outil `bootimage` applique par défaut un délai d'expiration de 5 minutes pour chaque exécutable de test. Si le test ne se termine pas dans ce délai, il est marqué comme échoué et une erreur « Timed Out » s'affiche dans la console. Cette fonctionnalité garantit que les tests bloqués dans une boucle infinie ne paralysent pas `cargo test` indéfiniment.

Vous pouvez le vérifier vous-même en ajoutant une instruction `loop {}` dans le test `trivial_assertion`. Lorsque vous exécutez `cargo test`, vous constatez que le test est marqué comme expiré au bout de 5 minutes. La durée du délai d'expiration est [configurable][bootimage config] via une clé `test-timeout` dans le Cargo.toml :

[bootimage config]: https://github.com/rust-osdev/bootimage#configuration

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-timeout = 300          # (in seconds)
```

Si vous ne souhaitez pas attendre 5 minutes pour voir le test `trivial_assertion` expirer, vous pouvez temporairement réduire cette valeur.

### Ajouter automatiquement l'affichage

Notre test `trivial_assertion` doit actuellement afficher lui-même son statut à l'aide de `serial_print!`/`serial_println!` :

```rust
#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

Ajouter manuellement ces instructions d'affichage pour chaque test que nous écrivons est fastidieux. Mettons donc à jour notre `test_runner` afin qu'il affiche automatiquement ces messages. Pour cela, nous devons créer un nouveau trait `Testable` :

```rust
// in src/main.rs

pub trait Testable {
    fn run(&self) -> ();
}
```

L'astuce consiste maintenant à implémenter ce trait pour tous les types `T` qui implémentent le [trait `Fn()`] :

[trait `Fn()`]: https://doc.rust-lang.org/stable/core/ops/trait.Fn.html

```rust
// in src/main.rs

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

Nous implémentons la fonction `run` en commençant par afficher le nom de la fonction à l'aide de la fonction [`any::type_name`]. Cette fonction est implémentée directement dans le compilateur et renvoie une description textuelle de tout type. Pour les fonctions, ce type correspond à leur nom, ce qui est exactement ce que nous recherchons ici. Le caractère `\t` est la [tabulation], qui apporte un léger alignement aux messages `[ok]`.

[`any::type_name`]: https://doc.rust-lang.org/stable/core/any/fn.type_name.html
[tabulation]: https://en.wikipedia.org/wiki/Tab_character

Après avoir affiché le nom de la fonction, nous invoquons la fonction de test via `self()`. Cela ne fonctionne que parce que nous exigeons que `self` implémente le trait `Fn()`. Une fois que la fonction de test retourne, nous affichons `[ok]` pour indiquer qu'elle n'a pas paniqué.

La dernière étape consiste à mettre à jour notre `test_runner` pour qu'il utilise le nouveau trait `Testable` :

```rust
// in src/main.rs

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) { // new
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run(); // new
    }
    exit_qemu(QemuExitCode::Success);
}
```

Les deux seuls changements sont le type de l'argument `tests`, qui passe de `&[&dyn Fn()]` à `&[&dyn Testable]`, ainsi que le fait que nous appelons désormais `test.run()` au lieu de `test()`.

Nous pouvons maintenant retirer les instructions d'affichage de notre test `trivial_assertion`, puisqu'elles sont désormais générées automatiquement :

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
```

La sortie de `cargo test` ressemble désormais à ceci :

```
Running 1 tests
blog_os::trivial_assertion...	[ok]
```

Le nom de la fonction inclut désormais le chemin complet vers celle-ci, ce qui est utile lorsque des fonctions de test portant le même nom existent dans différents modules. Le reste de la sortie est identique à ce que nous avions auparavant, mais nous n'avons plus besoin d'ajouter manuellement des instructions d'affichage à nos tests.

## Tester le tampon VGA

Maintenant que nous disposons d'un framework de test fonctionnel, nous pouvons créer quelques tests pour notre implémentation du tampon VGA. Commençons par un test très simple pour vérifier que `println` fonctionne sans paniquer :

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}
```

Ce test se contente d'afficher quelque chose dans le tampon VGA. S'il se termine sans paniquer, cela signifie que l'appel à `println` n'a pas non plus paniqué.

Pour s'assurer qu'aucun panic ne survient même lorsque de nombreuses lignes sont affichées et que des lignes défilent hors de l'écran, nous pouvons créer un autre test :

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}
```

Nous pouvons également créer une fonction de test pour vérifier que les lignes affichées apparaissent réellement à l'écran :

```rust
// in src/vga_buffer.rs

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

Cette fonction définit une chaîne de test, l'affiche à l'aide de `println`, puis parcourt les caractères écran du `WRITER` statique, qui représente le tampon texte VGA. Comme `println` affiche sur la dernière ligne de l'écran puis ajoute immédiatement un retour à la ligne, la chaîne devrait apparaître sur la ligne `BUFFER_HEIGHT - 2`.

En utilisant [`enumerate`], nous comptons le nombre d'itérations dans la variable `i`, que nous utilisons ensuite pour charger le caractère écran correspondant à `c`. En comparant le champ `ascii_character` du caractère écran avec `c`, nous vérifions que chaque caractère de la chaîne apparaît bien dans le tampon texte VGA.

[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate

Comme vous pouvez l'imaginer, nous pourrions créer bien d'autres fonctions de test. Par exemple, une fonction vérifiant qu'aucun panic ne survient lors de l'affichage de lignes très longues et qu'elles sont correctement renvoyées à la ligne, ou une fonction testant que les retours à la ligne, les caractères non imprimables et les caractères non-Unicode sont gérés correctement.

Pour le reste de cet article, nous allons cependant expliquer comment créer des _tests d'intégration_ afin de tester l'interaction entre différents composants.

## Tests d'intégration

La convention pour les [tests d'intégration] en Rust consiste à les placer dans un répertoire `tests` à la racine du projet (c'est-à-dire à côté du répertoire `src`). Le framework de test par défaut ainsi que les frameworks de test personnalisés détecteront et exécuteront automatiquement tous les tests présents dans ce répertoire.

[tests d'intégration]: https://doc.rust-lang.org/book/ch11-03-test-organization.html#integration-tests

Tous les tests d'intégration constituent des exécutables à part entière, complètement séparés de notre `main.rs`. Cela signifie que chaque test doit définir sa propre fonction de point d'entrée. Créons un exemple de test d'intégration nommé `basic_boot` pour voir en détail comment cela fonctionne :

```rust
// in tests/basic_boot.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

#[unsafe(no_mangle)] // don't mangle the name of this function
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

Puisque les tests d'intégration sont des exécutables séparés, nous devons redéfinir tous les attributs de crate (`no_std`, `no_main`, `test_runner`, etc.). Nous devons également créer une nouvelle fonction de point d'entrée `_start`, qui appelle la fonction d'entrée des tests `test_main`. Nous n'avons besoin d'aucun attribut `cfg(test)`, car les exécutables de tests d'intégration ne sont jamais compilés en mode non-test.

Nous utilisons la macro [`unimplemented`], qui panique systématiquement, comme espace réservé pour la fonction `test_runner`, et nous nous contentons pour l'instant d'une simple `loop` dans le gestionnaire de `panic`. Idéalement, nous souhaiterions implémenter ces fonctions exactement comme nous l'avons fait dans notre `main.rs`, en utilisant la macro `serial_println` et la fonction `exit_qemu`. Le problème est que nous n'avons pas accès à ces fonctions, puisque les tests sont compilés de manière totalement séparée de notre exécutable `main.rs`.

[`unimplemented`]: https://doc.rust-lang.org/core/macro.unimplemented.html

Si vous exécutez `cargo test` à ce stade, vous obtiendrez une boucle infinie, car le gestionnaire de panic boucle indéfiniment. Vous devrez utiliser le raccourci clavier `ctrl+c` pour quitter QEMU.

### Créer une bibliothèque

Pour rendre les fonctions nécessaires disponibles à notre test d'intégration, nous devons extraire une bibliothèque de notre `main.rs`, qui pourra être incluse par d'autres crates et par les exécutables de test d'intégration. Pour cela, nous créons un nouveau fichier `src/lib.rs` :

```rust
// src/lib.rs

#![no_std]

```

Comme `main.rs`, le fichier `lib.rs` est un fichier spécial automatiquement reconnu par cargo. La bibliothèque constitue une unité de compilation distincte, nous devons donc à nouveau préciser l'attribut `#![no_std]`.

Pour que notre bibliothèque fonctionne avec `cargo test`, nous devons également déplacer les fonctions et attributs de test de `main.rs` vers `lib.rs` :

```rust
// in src/lib.rs

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

/// Entry point for `cargo test`
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

Pour rendre notre `test_runner` disponible aux exécutables et aux tests d'intégration, nous le rendons public et ne lui appliquons pas l'attribut `cfg(test)`. Nous extrayons également l'implémentation de notre gestionnaire de panic dans une fonction publique `test_panic_handler`, afin qu'elle soit aussi disponible pour les exécutables.

Comme notre `lib.rs` est testé indépendamment de notre `main.rs`, nous devons ajouter un point d'entrée `_start` et un gestionnaire de panic lorsque la bibliothèque est compilée en mode test. Grâce à l'attribut de crate [`cfg_attr`], nous activons conditionnellement l'attribut `no_main` dans ce cas.

[`cfg_attr`]: https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute

Nous déplaçons également l'énumération `QemuExitCode` et la fonction `exit_qemu`, en les rendant publiques :

```rust
// in src/lib.rs

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

Les exécutables et les tests d'intégration peuvent désormais importer ces fonctions depuis la bibliothèque, sans avoir à fournir leur propre implémentation. Pour rendre également `println` et `serial_println` disponibles, nous déplaçons aussi les déclarations de modules :

```rust
// in src/lib.rs

pub mod serial;
pub mod vga_buffer;
```

Nous rendons ces modules publics afin qu'ils soient utilisables en dehors de notre bibliothèque. Cela est également nécessaire pour rendre nos macros `println` et `serial_println` utilisables, puisqu'elles font appel aux fonctions `_print` de ces modules.

Nous pouvons maintenant mettre à jour notre `main.rs` pour qu'il utilise la bibliothèque :

```rust
// in src/main.rs

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

/// This function is called on panic.
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

La bibliothèque s'utilise comme une crate externe normale. Elle s'appelle `blog_os`, comme notre crate. Le code ci-dessus utilise la fonction `blog_os::test_runner` dans l'attribut `test_runner`, ainsi que la fonction `blog_os::test_panic_handler` dans notre gestionnaire de panic `cfg(test)`. Il importe également la macro `println` pour la rendre disponible à nos fonctions `_start` et `panic`.

À ce stade, `cargo run` et `cargo test` devraient à nouveau fonctionner. Bien sûr, `cargo test` boucle toujours indéfiniment (vous pouvez quitter avec `ctrl+c`). Corrigeons cela en utilisant les fonctions de bibliothèque nécessaires dans notre test d'intégration.

### Compléter le test d'intégration

Comme notre `src/main.rs`, notre exécutable `tests/basic_boot.rs` peut importer des types depuis notre nouvelle bibliothèque. Cela nous permet d'importer les éléments manquants pour compléter notre test :

```rust
// in tests/basic_boot.rs

#![test_runner(blog_os::test_runner)]

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

Plutôt que de réimplémenter le test runner, nous utilisons la fonction `test_runner` de notre bibliothèque en remplaçant l'attribut `#![test_runner(crate::test_runner)]` par `#![test_runner(blog_os::test_runner)]`. Nous n'avons alors plus besoin de la fonction `test_runner` provisoire dans `basic_boot.rs`, et pouvons donc la supprimer. Pour notre gestionnaire `panic`, nous appelons la fonction `blog_os::test_panic_handler`, comme nous l'avons fait dans notre `main.rs`.

Désormais, `cargo test` se termine normalement à nouveau. Lorsque vous l'exécutez, vous constatez qu'il compile et exécute successivement les tests de `lib.rs`, `main.rs` et `basic_boot.rs`. Pour les tests d'intégration `main.rs` et `basic_boot`, il indique « Running 0 tests », car ces fichiers ne contiennent aucune fonction annotée avec `#[test_case]`.

Nous pouvons maintenant ajouter des tests à notre `basic_boot.rs`. Par exemple, nous pouvons tester que `println` fonctionne sans paniquer, comme nous l'avons fait pour les tests du tampon VGA :

```rust
// in tests/basic_boot.rs

use blog_os::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}
```

Lorsque nous exécutons `cargo test` maintenant, nous constatons qu'il trouve et exécute la fonction de test.

Ce test peut sembler quelque peu inutile pour l'instant, puisqu'il est presque identique à l'un des tests du tampon VGA. Cependant, à l'avenir, les fonctions `_start` de notre `main.rs` et de notre `lib.rs` pourraient s'étoffer et appeler diverses routines d'initialisation avant d'exécuter la fonction `test_main`, de sorte que les deux tests s'exécuteraient dans des environnements très différents.

En testant `println` dans un environnement `basic_boot`, sans appeler la moindre routine d'initialisation dans `_start`, nous pouvons nous assurer que `println` fonctionne dès le démarrage. C'est important, car nous en dépendons notamment pour l'affichage des messages de panic.

### Tests futurs

La force des tests d'intégration réside dans le fait qu'ils sont traités comme des exécutables totalement séparés. Cela leur confère un contrôle total sur l'environnement, ce qui permet de tester que le code interagit correctement avec le CPU ou les périphériques matériels.

Notre test `basic_boot` est un exemple très simple de test d'intégration. À l'avenir, notre noyau deviendra beaucoup plus riche en fonctionnalités et interagira avec le matériel de diverses façons. En ajoutant des tests d'intégration, nous pouvons nous assurer que ces interactions fonctionnent (et continuent de fonctionner) comme prévu. Voici quelques idées de tests futurs possibles :

- **Exceptions CPU** : lorsque le code effectue des opérations invalides (par exemple, une division par zéro), le CPU déclenche une exception. Le noyau peut enregistrer des fonctions de gestion pour de telles exceptions. Un test d'intégration pourrait vérifier que le bon gestionnaire d'exception est appelé lors d'une exception CPU, ou que l'exécution se poursuit correctement après une exception résoluble.
- **Tables de pages** : les tables de pages définissent quelles régions mémoire sont valides et accessibles. En modifiant les tables de pages, il est possible d'allouer de nouvelles régions mémoire, par exemple lors du lancement de programmes. Un test d'intégration pourrait modifier les tables de pages dans la fonction `_start` et vérifier que ces modifications produisent les effets attendus dans des fonctions `#[test_case]`.
- **Programmes en espace utilisateur** : les programmes en espace utilisateur sont des programmes disposant d'un accès limité aux ressources du système. Par exemple, ils n'ont pas accès aux structures de données du noyau ni à la mémoire d'autres programmes. Un test d'intégration pourrait lancer des programmes en espace utilisateur effectuant des opérations interdites et vérifier que le noyau les bloque toutes.

Comme vous pouvez l'imaginer, bien d'autres tests sont possibles. En ajoutant de tels tests, nous nous assurons de ne pas les casser accidentellement lorsque nous ajoutons de nouvelles fonctionnalités à notre noyau ou que nous refactorisons notre code. Cela devient particulièrement important à mesure que notre noyau grossit et se complexifie.

### Tests censés paniquer

Le framework de test de la bibliothèque standard prend en charge un attribut [`#[should_panic]`][should_panic] permettant de construire des tests censés échouer. Cela est utile, par exemple, pour vérifier qu'une fonction échoue bien lorsqu'un argument invalide lui est transmis. Malheureusement, cet attribut n'est pas pris en charge dans les crates `#[no_std]`, car il nécessite le support de la bibliothèque standard.

[should_panic]: https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html#testing-panics

Bien que nous ne puissions pas utiliser l'attribut `#[should_panic]` dans notre noyau, nous pouvons obtenir un comportement similaire en créant un test d'intégration qui quitte avec un code d'erreur de succès depuis le gestionnaire de panic. Commençons par créer un tel test, nommé `should_panic` :

```rust
// in tests/should_panic.rs

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

Ce test reste incomplet, car il ne définit encore ni fonction `_start` ni aucun des attributs propres au framework de test personnalisé. Ajoutons les éléments manquants :

```rust
// in tests/should_panic.rs

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

Plutôt que de réutiliser le `test_runner` de notre `lib.rs`, ce test définit sa propre fonction `test_runner`, qui quitte avec un code de sortie d'échec lorsqu'un test retourne sans paniquer (puisque nous voulons justement que nos tests paniquent). Si aucune fonction de test n'est définie, le runner quitte avec un code de sortie de succès. Comme le runner quitte systématiquement après l'exécution d'un seul test, il n'est pas pertinent de définir plus d'une fonction `#[test_case]`.

Nous pouvons maintenant créer un test censé échouer :

```rust
// in tests/should_panic.rs

use blog_os::serial_print;

#[test_case]
fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}
```

Ce test utilise `assert_eq` pour vérifier que `0` et `1` sont égaux. Bien entendu, cela échoue, et notre test panique donc comme prévu. Notez que nous devons afficher manuellement le nom de la fonction à l'aide de `serial_print!` ici, car nous n'utilisons pas le trait `Testable`.

Lorsque nous exécutons le test via `cargo test --test should_panic`, nous constatons qu'il réussit, puisque le test a paniqué comme attendu. Si nous commentons l'assertion et relançons le test, nous constatons qu'il échoue bien, avec le message _« test did not panic »_.

Un inconvénient majeur de cette approche est qu'elle ne fonctionne que pour une seule fonction de test. Avec plusieurs fonctions `#[test_case]`, seule la première est exécutée, car l'exécution ne peut pas se poursuivre une fois le gestionnaire de panic appelé. Je ne connais actuellement pas de bonne solution à ce problème ; n'hésitez pas à me faire part de vos idées !

### Tests sans harnais (No Harness Tests)

Pour les tests d'intégration ne comportant qu'une seule fonction de test (comme notre test `should_panic`), le test runner n'est pas vraiment nécessaire. Dans ce genre de cas, nous pouvons désactiver complètement le test runner et exécuter notre test directement dans la fonction `_start`.

La clé de cette approche consiste à désactiver le drapeau `harness` pour le test dans le `Cargo.toml`, qui détermine si un test runner est utilisé pour un test d'intégration. Lorsqu'il est défini à `false`, à la fois le test runner par défaut et la fonctionnalité de framework de test personnalisé sont désactivés, de sorte que le test est traité comme un exécutable normal.

Désactivons le drapeau `harness` pour notre test `should_panic` :

```toml
# in Cargo.toml

[[test]]
name = "should_panic"
harness = false
```

Nous pouvons maintenant grandement simplifier notre test `should_panic` en supprimant le code lié au `test_runner`. Le résultat ressemble à ceci :

```rust
// in tests/should_panic.rs

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

Nous appelons maintenant directement la fonction `should_fail` depuis notre fonction `_start`, et quittons avec un code de sortie d'échec si elle retourne. Lorsque nous exécutons `cargo test --test should_panic` maintenant, nous constatons que le test se comporte exactement comme avant.

Au-delà de la création de tests `should_panic`, la désactivation de l'attribut `harness` peut également s'avérer utile pour des tests d'intégration complexes, par exemple lorsque les différentes fonctions de test ont des effets de bord et doivent être exécutées dans un ordre précis.

## Résumé

Les tests constituent une technique très utile pour s'assurer que certains composants se comportent comme prévu. Même s'ils ne peuvent pas démontrer l'absence de bugs, ils restent un outil précieux pour les détecter, et surtout pour éviter les régressions.

Cet article a expliqué comment mettre en place un framework de test pour notre noyau Rust. Nous avons utilisé la fonctionnalité des frameworks de test personnalisés de Rust afin d'implémenter le support d'un simple attribut `#[test_case]` dans notre environnement bare-metal. Grâce au dispositif `isa-debug-exit` de QEMU, notre test runner peut quitter QEMU une fois les tests exécutés et rapporter leur statut. Pour afficher les messages d'erreur dans la console plutôt que dans le tampon VGA, nous avons créé un pilote basique pour le port série.

Après avoir créé quelques tests pour notre macro `println`, nous avons exploré les tests d'intégration dans la seconde moitié de cet article. Nous avons appris qu'ils se trouvent dans le répertoire `tests` et sont traités comme des exécutables complètement séparés. Pour leur donner accès à la fonction `exit_qemu` et à la macro `serial_println`, nous avons déplacé l'essentiel de notre code dans une bibliothèque pouvant être importée par tous les exécutables et tests d'intégration. Comme les tests d'intégration s'exécutent dans leur propre environnement séparé, ils permettent de tester les interactions avec le matériel ou de créer des tests censés paniquer.

Nous disposons désormais d'un framework de test s'exécutant dans un environnement réaliste au sein de QEMU. En créant davantage de tests dans les prochains articles, nous pourrons maintenir la maintenabilité de notre noyau à mesure qu'il se complexifiera.

## Et ensuite ?

Dans le prochain article, nous explorerons les _exceptions CPU_. Ces exceptions sont déclenchées par le CPU lorsqu'une opération illégale se produit, comme une division par zéro ou un accès à une page mémoire non mappée (ce que l'on appelle une « page fault »). Être capable d'intercepter et d'examiner ces exceptions est essentiel pour déboguer les futures erreurs. La gestion des exceptions est également très proche de la gestion des interruptions matérielles, nécessaire pour la prise en charge du clavier.