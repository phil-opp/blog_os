+++
title = "A Freestanding Rust Binary"
weight = 1
path = "freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
+++

La première étape pour créer notre propre noyeau de système d'exploitation est de créer un exécutable Rust qui ne relie pas la bibliothèque standard. Cela rend possible l'exécution du code Rust sur la [machine nue] sans système d'exploitation sous-jacent.

[machine nue]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

Ce blog est développé sur [GitHub]. Si vous avez un problème ou une question, veuillez ouvrir une issue. Vous pouvez aussi laisser un commentaire [en bas de page]. Le code source complet de cet article est disponible sur la branche [`post-01`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[en bas de page]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## Introduction
Pour écrire un noyau de système d'exploitation, nous avons besoin d'un code qui ne dépend pas de fonctionnalités de système d'exploitation. Cela signifie que nous ne pouvons pas utiliser les fils d'exécution, les fichiers, la mémoire sur le tas, le réseau, les nombres aléatoires, la sortie standard ou tout autre fonctionnalité nécessitant une abstraction du système d'exploitation ou un matériel spécifique. Cela a du sens, étant donné que nous essayons d'écrire notre propre OS et nos propres pilotes.
Cela signifie que nous ne pouvons pas utiliser la majeure partie de la [bibliothèque standard de Rust]. Il y a néanmoins beaucoup de fonctionnalités de Rust que nous _pouvons_ utiliser. Par exemple, nous pouvons utiliser les [iterators], les [closures], le [pattern matching], l'[option] et le [result], le [string formatting], et bien-sûr l'[ownership system]. Ces fonctionnalités permettent l'écriture d'un noyeau d'une façon expressive et haut-niveau sans se soucier du [comportement non-défini] ou de la [sécurité de la mémoire].

[option]: https://doc.rust-lang.org/core/option/
[result]:https://doc.rust-lang.org/core/result/
[bibliothèque standard de Rust]: https://doc.rust-lang.org/std/
[iterators]: https://doc.rust-lang.org/book/ch13-02-iterators.html
[closures]: https://doc.rust-lang.org/book/ch13-01-closures.html
[pattern matching]: https://doc.rust-lang.org/book/ch06-00-enums.html
[string formatting]: https://doc.rust-lang.org/core/macro.write.html
[ownership system]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
[comportement non-défini]: https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs
[sécurité de la mémoire]: https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention

Pour créer un noyau d'OS en Rust, nous devons créer un exécutable qui peut tourner sans système d'exploitation sous-jacent. Un tel exécutable est appelé “freestanding” (autoporté) ou “bare-metal”.
Cet article décrit les étapes nécessaires pour créer un exécutable Rust autoporté et explique pourquoi ces étapes sont importantes. Si vous n'êtes intéressé que par un example minimal, vous pouvez **[aller au résumé](#summary)**.

## Désactiver la Bibliothèque Standard

Par défaut, toutes les crates Rust relient la [bibliothèque standard], qui dépend du système d'exploitation pour les fonctionnalités telles que les fils d'exécution, les fichiers ou le réseau. Elle dépend aussi de la bibliothèque standard de C `libc`, qui intéragit de près avec les services de l'OS. Comme notre plan est d'écrire un système d'exploitation, nous ne pouvons pas utiliser des bibliothèques dépendant de l'OS. Nous devons donc désactiver l'inclusion automatique de la bibliothèque standard en utilisant l'[attribut `no std`].

[bibliothèque standard]: https://doc.rust-lang.org/std/
[attribut `no std`]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

Nous commencons par créer un nouveau projet d'application cargo. La manière la plus simple de faire est avec la ligne de commande : 

```
cargo new blog_os --bin --edition 2018
```

J'ai nommé le projet `blog_os`, mais vous pouvez bien-sûr choisir le nom qu'il vous convient. Le flag `--bin` indique que nous voulons créer un exécutable (contrairement à une bibliothèque) et le flag `--edition 2018` indique que nous voulons utiliser l'[édition 2018] de Rust pour notre crate. Quand nous lançons la commande, cargo crée la structure de répertoire suivante pour nous :

[édition 2018]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

Le fichier `Cargo.toml` contient la configuration de la crate, par exemple le nom de la crate, l'auteur, le numéro de [versionnage sémantique] et les dépendances. Le fichier `src/main.rs` contient le module racine de notre crate et notre fonction `main`. Vous pouvez compiler votre crate avec `cargo build` et ensuite exécuter l'exécutable compilé `blog_os` dans le sous-dossier `target/debug`.

[versionnage sémantique]: https://semver.org/

### L'Attribut `no_std`

Pour l'instant, notre crate relie la bilbiothèque standard implicitement. Désactivons cela en ajoutant l'[attribut `no std`] : 

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

Quand nous essayons maintenant de compiler (avec `cargo build)`, l'erreur suivante se produit :

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

La raison est que la [macro `println`] fait partie de la bibliothèque standard, que nous ne pouvons plus utiliser. Nous ne pouvons donc plus afficher des choses. Cela est logique, car `println` écrit dans la [sortie standard], qui est un descripteur de fichier spécial fourni par le système d'eploitation.

[macro `println`]: https://doc.rust-lang.org/std/macro.println.html
[sortie standard]: https://fr.wikipedia.org/wiki/Flux_standard#Sortie_standard

Supprimons l'affichage et essayons à nouveau avec une fonction main vide :

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

Maintenant le compilateur a besoin d'une fonction `#[panic_handler]` et d'un _objet de langage_.

## Implémentation de Panic

L'attribut `panic_handler` définit la fonction que le compilateur doit appeler lorsqu'un [panic] arrive. La bibliothèque standard fournit sa propre fonction de gestion de panic mais dans un environnement `no_std`, nous avons besoin de le définir nous-mêmes : 

[panic]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// dans main.rs

use core::panic::PanicInfo;

/// Cette fonction est appelée à chaque panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

Le [paramètre `PanicInfo`][PanicInfo] contient le fichier et la ligne où le panic a eu lieu et le message optionnel de panic. La fonction ne devrait jamais retourner quoi que ce soit, elle est donc marquée comme [fonction divergente] en retournant le [type “never”] `!`. Nous ne pouvons pas faire grand chose dans cette fonction pour le moment, nous bouclons donc indéfiniment.

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[fonction divergente]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[type “never”]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## L'Objet de Langage `eh_personality`

Les objets de langage sont des fonctions et des types spéciaux qui sont requis par le compilateur de manière interne. Par example, le trait [`Copy`] est un objet de langage qui indique au compilateur quels types possèdent la [sémantique copy][`Copy`]. Quand nous regardons l'[implémantation][copy code] du code, nous pouvons voir qu'il possède l'attribut spécial `#[lang = copy]` qui le définit comme étant un objet de langage.

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

Bien qu'il soit possible de fourning des implémentations personnalisées des objets de langage, cela ne devrait être fait qu'en dernier recours. La raison est que les objets de langages sont des détails d'implémentation très instables et qui ne sont même pas vérifiés au niveau de leur type (donc le compilateur ne vérifie même pas qu'une fonction possède les bons types d'arguments). Heureusement, il y a une manière plus stable de corriger l'erreur d'object de langage ci-dessus.

L'[objet de langage `eh_personality`] marque une fonction qui est utilisée pour l'implémentation du [déroulement de pile]. Par défaut, Rust utilise le déroulement de pule pour exécuter les destructeurs de chaque variables vivante sur le stack en cas de [panic]. Cela assure que toute la mémoire utilisée est libérée et permet au fil dexécution parent d'attraper le panix et de continuer l'exécution. Le déroulement toutefois est un processus compliqué et nécessite des bibliothèques spécifiques à l'OS ([libunwind] pour Linux ou [gestion structurée des erreurs] pour Windows), nous ne voulons donc pas l'utiliser pour notre sustème d'exploitation.

[objet de langage `eh_personality`]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[déroulement de pile]: https://docs.microsoft.com/fr-fr/cpp/cpp/exceptions-and-stack-unwinding-in-cpp?view=msvc-160
[libunwind]: https://www.nongnu.org/libunwind/
[gestion structurée des erreurs]: https://docs.microsoft.com/fr-fr/windows/win32/debug/structured-exception-handling

### Désactiver le Déroulement

Il y a d'autres cas d'utilisation pour lesquels le déroulement n'est pas souhaité. Rust offre donc une option pour [interrompre après un panic]. Cela désactive la génération de symboles de déroulement et ainsi reduit considérablement la taille de l'exécutable. Il y a de multiples endroit où nous pouvons désactiver le déroulement. Le plus simple est d'ajouter les lignes suivantes dans notre `Cargo.toml` :

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

Cela configure la stratégie de panic à `abort` pour le profile `dev` (utilisé pour `cargo build`) et le profil `release` (utilisé pour `cargo build --release`). Maintenant l'objet de langage `eh_personality` ne devrait plus être rquis. 

[interrompre après un panic]: https://github.com/rust-lang/rust/pull/32900

Nous avons dorénavant corrigé les deux erreurs ci-dessus. Toutefois, si nous essayons de compiler, une autre erreur apparaît :

```
> cargo build
error: requires `start` lang_item
```

L'objet de langage `start` manque à notre programme. Il définit le point d'entrée.

## L'attribut `start`

On pourrait penser que la fonction `main` est la première fonction appelée lorsqu'un programme est exécuté. Toutefois, la plupart des langage a un [environnement d'exécution] qui est responsables des tâches telles que le ramassage des miettes (ex: dans Java) ou les fils d'exécution logiciel (ex: les goroutines dans Go). Cet environnement doit être appelé avant `main` puisqu'il a besoin de s'initialiser.

[environnement d'exécution]: https://fr.wikipedia.org/wiki/Environnement_d%27ex%C3%A9cution

Dans un exécutable Rust classique qui relie la bibliothèque standard, l'exécution commence dans une bibliothèque d'environnement d'exécution C appelé `crt0` (“C runtime zero”). Elle configure l'environnement pour une application C. Cela comprend la création d'une pile et le placement des arguments dans les bons registres. L'environnement d'exécution C appelle ensuite [le point d'entrée de l'environnement d'exécution de Rust][rt::lang_start], qui est marqué par l'objet de langage `start`. Rust possède un environnement d'exécution très minime, qui se charge de petites tâches telles que la configuration des guardes de dépassement de pile ou l'affichage de la trace d'appels lors d'un panic. L'environnement d'exécution finit par appeler la fonction `main`.

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

Notre exécutable autoporté n'a pas accès à l'environnement d'exécution de Rust ni à `crt0`. Nous avons donc besion de définir notre propre point d'entrée. Implémenter l'objet de langage `start` n'aiderait pas car nous aurions toujours besoin de `crt0`. Nous avons plutôt besoin de réécrire le point d'entrée de `crt0` directement.

### Réécrire le Point d'Entrée

Pour indiquer au compilateur que nous ne voulons pas utiliser la chaîne de point d'entrée normale, nous ajoutons l'attribut `#![no_main]`. 

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// Cette fonction est appelée à chaque panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

Vous remarquerez peut-être que nous avons retiré la fonction `main`. La raison est que la présence de cette fonction n'a pas de sens sans un environnement d'exécution sous-jacent qui l'appelle. À la place, nous réécrivons le point d'entrée du système d'exploitation avec notre propre fonction `_start` :

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

En utilisant l'attribut `#[no_mangle]`, nous désactivons la [décoration de nom] pour assurer que le compilateur Rust crée une fonction avec le nom `_start`. Sans cet attribut, le compilateur génèrerait un symbol obscure `_ZN3blog_os4_start7hb173fedf945531caE` pour donner un nom unique à chaque fonction. L'attribut est nécessaire car nous avons besoin d'indiquer le nom de la fonction de point d'entrée à l'éditeur de lien dans l'étape suivante.

Nous devons aussi marquer la fonction avec `extern C` pour indiquer au compilateur qu'il devrait utiliser la [convention de nommage] de C pour cette fonction (au lieu de la convention de nommage de Rust non-spécifiée). Cette fonction se nomme `_start` car c'est le nom par défaut des points d'entrée pour la plupart des systèmes.

[décoration de nom]: https://fr.wikipedia.org/wiki/D%C3%A9coration_de_nom
[convention de nommage]: https://fr.wikipedia.org/wiki/Convention_de_nommage

Le type de retour `!` signifie que la fonction est divergente, c-à-d qu'elle n'a pas le droit de retourner quoi que ce soit. Cela est nécessaire car le point d'entrée n'est pas appelé par une fonction, mais invoqué directement par le système d'exploitation ou par le chargeur d'amorçage. Donc au lieu de retourner une valeur, le point d'entrée doit invoquer l'[appel système `exit`] du système d'exploitation. Dans notre cas, arrêter la machine pourrait être une action convenable, puisqu'il ne reste rien d'autre à faire si un exécutable autoporté s'arrête. Pour l'instant, nous remplissons le condition en bouclant indéfiniement.

[appel système `exit`]: https://en.wikipedia.org/wiki/Exit_(system_call)

Quand nous lançons `cargo build`, nous obtenons une erreur de l'_éditeur de liens_.

## Linker Errors

The linker is a program that combines the generated code into an executable. Since the executable format differs between Linux, Windows, and macOS, each system has its own linker that throws a different error. The fundamental cause of the errors is the same: the default configuration of the linker assumes that our program depends on the C runtime, which it does not.

To solve the errors, we need to tell the linker that it should not include the C runtime. We can do this either by passing a certain set of arguments to the linker or by building for a bare metal target.

### Building for a Bare Metal Target

By default Rust tries to build an executable that is able to run in your current system environment. For example, if you're using Windows on `x86_64`, Rust tries to build a `.exe` Windows executable that uses `x86_64` instructions. This environment is called your "host" system.

To describe different environments, Rust uses a string called [_target triple_]. You can see the target triple for your host system by running `rustc --version --verbose`:

[_target triple_]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple

```
rustc 1.35.0-nightly (474e7a648 2019-04-07)
binary: rustc
commit-hash: 474e7a6486758ea6fc761893b1a49cd9076fb0ab
commit-date: 2019-04-07
host: x86_64-unknown-linux-gnu
release: 1.35.0-nightly
LLVM version: 8.0
```

The above output is from a `x86_64` Linux system. We see that the `host` triple is `x86_64-unknown-linux-gnu`, which includes the CPU architecture (`x86_64`), the vendor (`unknown`), the operating system (`linux`), and the [ABI] (`gnu`).

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

By compiling for our host triple, the Rust compiler and the linker assume that there is an underlying operating system such as Linux or Windows that use the C runtime by default, which causes the linker errors. So to avoid the linker errors, we can compile for a different environment with no underlying operating system.

An example for such a bare metal environment is the `thumbv7em-none-eabihf` target triple, which describes an [embedded] [ARM] system. The details are not important, all that matters is that the target triple has no underlying operating system, which is indicated by the `none` in the target triple. To be able to compile for this target, we need to add it in rustup:

[embedded]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture

```
rustup target add thumbv7em-none-eabihf
```

This downloads a copy of the standard (and core) library for the system. Now we can build our freestanding executable for this target:

```
cargo build --target thumbv7em-none-eabihf
```

By passing a `--target` argument we [cross compile] our executable for a bare metal target system. Since the target system has no operating system, the linker does not try to link the C runtime and our build succeeds without any linker errors.

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

This is the approach that we will use for building our OS kernel. Instead of `thumbv7em-none-eabihf`, we will use a [custom target] that describes a `x86_64` bare metal environment. The details will be explained in the next post.

[custom target]: https://doc.rust-lang.org/rustc/targets/custom.html

### Linker Arguments

Instead of compiling for a bare metal system, it is also possible to resolve the linker errors by passing a certain set of arguments to the linker. This isn't the approach that we will use for our kernel, therefore this section is optional and only provided for completeness. Click on _"Linker Arguments"_ below to show the optional content.

<details>

<summary>Linker Arguments</summary>

In this section we discuss the linker errors that occur on Linux, Windows, and macOS, and explain how to solve them by passing additional arguments to the linker. Note that the executable format and the linker differ between operating systems, so that a different set of arguments is required for each system.

#### Linux

On Linux the following linker error occurs (shortened):

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

The problem is that the linker includes the startup routine of the C runtime by default, which is also called `_start`. It requires some symbols of the C standard library `libc` that we don't include due to the `no_std` attribute, therefore the linker can't resolve these references. To solve this, we can tell the linker that it should not link the C startup routine by passing the `-nostartfiles` flag.

One way to pass linker attributes via cargo is the `cargo rustc` command. The command behaves exactly like `cargo build`, but allows to pass options to `rustc`, the underlying Rust compiler. `rustc` has the `-C link-arg` flag, which passes an argument to the linker. Combined, our new build command looks like this:

```
cargo rustc -- -C link-arg=-nostartfiles
```

Now our crate builds as a freestanding executable on Linux!

We didn't need to specify the name of our entry point function explicitly since the linker looks for a function with the name `_start` by default.

#### Windows

On Windows, a different linker error occurs (shortened):

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

The "entry point must be defined" error means that the linker can't find the entry point. On Windows, the default entry point name [depends on the used subsystem][windows-subsystems]. For the `CONSOLE` subsystem the linker looks for a function named `mainCRTStartup` and for the `WINDOWS` subsystem it looks for a function named `WinMainCRTStartup`. To override the default and tell the linker to look for our `_start` function instead, we can pass an `/ENTRY` argument to the linker:

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

From the different argument format we clearly see that the Windows linker is a completely different program than the Linux linker.

Now a different linker error occurs:

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

This error occurs because Windows executables can use different [subsystems][windows-subsystems]. For normal programs they are inferred depending on the entry point name: If the entry point is named `main`, the `CONSOLE` subsystem is used, and if the entry point is named `WinMain`, the `WINDOWS` subsystem is used. Since our `_start` function has a different name, we need to specify the subsystem explicitly:

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

We use the `CONSOLE` subsystem here, but the `WINDOWS` subsystem would work too. Instead of passing `-C link-arg` multiple times, we use `-C link-args` which takes a space separated list of arguments.

With this command, our executable should build successfully on Windows.

#### macOS

On macOS, the following linker error occurs (shortened):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

This error message tells us that the linker can't find an entry point function with the default name `main` (for some reason all functions are prefixed with a `_` on macOS). To set the entry point to our `_start` function, we pass the `-e` linker argument:

```
cargo rustc -- -C link-args="-e __start"
```

The `-e` flag specifies the name of the entry point function. Since all functions have an additional `_` prefix on macOS, we need to set the entry point to `__start` instead of `_start`.

Now the following linker error occurs:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS [does not officially support statically linked binaries] and requires programs to link the `libSystem` library by default. To override this and link a static binary, we pass the `-static` flag to the linker:

[does not officially support statically linked binaries]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

This still does not suffice, as a third linker error occurs:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

This error occurs because programs on macOS link to `crt0` (“C runtime zero”) by default. This is similar to the error we had on Linux and can be also solved by adding the `-nostartfiles` linker argument:

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Now our program should build successfully on macOS.

#### Unifying the Build Commands

Right now we have different build commands depending on the host platform, which is not ideal. To avoid this, we can create a file named `.cargo/config.toml` that contains the platform specific arguments:

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

The `rustflags` key contains arguments that are automatically added to every invocation of `rustc`. For more information on the `.cargo/config.toml` file check out the [official documentation](https://doc.rust-lang.org/cargo/reference/config.html).

Now our program should be buildable on all three platforms with a simple `cargo build`.

#### Should You Do This?

While it's possible to build a freestanding executable for Linux, Windows, and macOS, it's probably not a good idea. The reason is that our executable still expects various things, for example that a stack is initialized when the `_start` function is called. Without the C runtime, some of these requirements might not be fulfilled, which might cause our program to fail, e.g. through a segmentation fault.

If you want to create a minimal binary that runs on top of an existing operating system, including `libc` and setting the `#[start]` attribute as described [here](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html) is probably a better idea.

</details>

## Summary

A minimal freestanding Rust binary looks like this:

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

To build this binary, we need to compile for a bare metal target such as `thumbv7em-none-eabihf`:

```
cargo build --target thumbv7em-none-eabihf
```

Alternatively, we can compile it for the host system by passing additional linker arguments:

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Note that this is just a minimal example of a freestanding Rust binary. This binary expects various things, for example that a stack is initialized when the `_start` function is called. **So for any real use of such a binary, more steps are required**.

## What's next?

The [next post] explains the steps needed for turning our freestanding binary into a minimal operating system kernel. This includes creating a custom target, combining our executable with a bootloader, and learning how to print something to the screen.

[next post]: @/edition-2/posts/02-minimal-rust-kernel/index.md
