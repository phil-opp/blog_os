+++
title = "A Freestanding Rust Binary"
weight = 1
path = "fr/freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "3e87916b6c2ed792d1bdb8c0947906aef9013ac1"
# GitHub usernames of the people that translated this post
translators = ["Alekzus"]
+++
+++

La première étape pour créer notre propre noyau de système d'exploitation est de créer un exécutable Rust qui ne relie pas la bibliothèque standard. Cela rend possible l'exécution du code Rust sur la ["bare machine"][machine nue] sans système d'exploitation sous-jacent.

[machine nue]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

Ce blog est développé sur [GitHub]. Si vous avez un problème ou une question, veuillez ouvrir une issue. Vous pouvez aussi laisser un commentaire [en bas de page]. Le code source complet de cet article est disponible sur la branche [`post-01`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[en bas de page]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## Introduction
Pour écrire un noyau de système d'exploitation, nous avons besoin d'un code qui ne dépend pas de fonctionnalités de système d'exploitation. Cela signifie que nous ne pouvons pas utiliser les fils d'exécution, les fichiers, la mémoire sur le tas, le réseau, les nombres aléatoires, la sortie standard ou tout autre fonctionnalité nécessitant une abstraction du système d'exploitation ou un matériel spécifique. Cela a du sens, étant donné que nous essayons d'écrire notre propre OS et nos propres pilotes.
Cela signifie que nous ne pouvons pas utiliser la majeure partie de la [bibliothèque standard de Rust]. Il y a néanmoins beaucoup de fonctionnalités de Rust que nous _pouvons_ utiliser. Par exemple, nous pouvons utiliser les [iterators], les [closures], le [pattern matching], l'[option] et le [result], le [string formatting], et bien-sûr l'[ownership system]. Ces fonctionnalités permettent l'écriture d'un noyeau d'une façon expressive et haut-niveau sans se soucier des [comportements indéfinis] ou de la [sécurité de la mémoire].

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
Cet article décrit les étapes nécessaires pour créer un exécutable Rust autoporté et explique pourquoi ces étapes sont importantes. Si vous n'êtes intéressé que par un exemple minimal, vous pouvez **[aller au résumé](#résumé)**.

## Désactiver la Bibliothèque Standard

Par défaut, tous les crates Rust relient la [bibliothèque standard], qui dépend du système d'exploitation pour les fonctionnalités telles que les fils d'exécution, les fichiers ou le réseau. Elle dépend aussi de la bibliothèque standard de C `libc`, qui intéragit de près avec les services de l'OS. Comme notre plan est d'écrire un système d'exploitation, nous ne pouvons pas utiliser des bibliothèques dépendant de l'OS. Nous devons donc désactiver l'inclusion automatique de la bibliothèque standard en utilisant l'[attribut `no std`].

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

La raison est que la [macro `println`] fait partie de la bibliothèque standard, que nous ne pouvons plus utiliser. Nous ne pouvons donc plus afficher de texte avec. Cela est logique, car `println` écrit dans la [sortie standard], qui est un descripteur de fichier spécial fourni par le système d'eploitation.

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

Les objets de langage sont des fonctions et des types spéciaux qui sont requis par le compilateur de manière interne. Par exemple, le trait [`Copy`] est un objet de langage qui indique au compilateur quels types possèdent la [sémantique copy][`Copy`]. Quand nous regardons l'[implémentation][copy code] du code, nous pouvons voir qu'il possède l'attribut spécial `#[lang = copy]` qui le définit comme étant un objet de langage.

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

Bien qu'il soit possible de fournir des implémentations personnalisées des objets de langage, cela ne devrait être fait qu'en dernier recours. La raison est que les objets de langages sont des détails d'implémentation très instables et qui ne sont même pas vérifiés au niveau de leur type (donc le compilateur ne vérifie même pas qu'une fonction possède les bons types d'arguments). Heureusement, il y a une manière plus robuste de corriger l'erreur d'objet de langage ci-dessus.

L'[objet de langage `eh_personality`] marque une fonction qui est utilisée pour l'implémentation du [déroulement de pile]. Par défaut, Rust utilise le déroulement de pile pour exécuter les destructeurs de chaque variable vivante sur la pile en cas de [panic]. Cela assure que toute la mémoire utilisée est libérée et permet au fil d'exécution parent d'attraper la panic et de continuer l'exécution. Le déroulement toutefois est un processus compliqué et nécessite des bibliothèques spécifiques à l'OS ([libunwind] pour Linux ou [gestion structurée des erreurs] pour Windows), nous ne voulons donc pas l'utiliser pour notre système d'exploitation.

[objet de langage `eh_personality`]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[déroulement de pile]: https://docs.microsoft.com/fr-fr/cpp/cpp/exceptions-and-stack-unwinding-in-cpp?view=msvc-160
[libunwind]: https://www.nongnu.org/libunwind/
[gestion structurée des erreurs]: https://docs.microsoft.com/fr-fr/windows/win32/debug/structured-exception-handling

### Désactiver le Déroulement

Il y a d'autres cas d'utilisation pour lesquels le déroulement n'est pas souhaité. Rust offre donc une option pour [interrompre après un panic]. Cela désactive la génération de symboles de déroulement et ainsi réduit considérablement la taille de l'exécutable. Il y a de multiples endroit où nous pouvons désactiver le déroulement. Le plus simple est d'ajouter les lignes suivantes dans notre `Cargo.toml` :

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

Cela configure la stratégie de panic à `abort` pour le profil `dev` (utilisé pour `cargo build`) et le profil `release` (utilisé pour `cargo build --release`). Maintenant l'objet de langage `eh_personality` ne devrait plus être requis. 

[interrompre après un panic]: https://github.com/rust-lang/rust/pull/32900

Nous avons dorénavant corrigé les deux erreurs ci-dessus. Toutefois, si nous essayons de compiler, une autre erreur apparaît :

```
> cargo build
error: requires `start` lang_item
```

L'objet de langage `start` manque à notre programme. Il définit le point d'entrée.

## L'attribut `start`

On pourrait penser que la fonction `main` est la première fonction appelée lorsqu'un programme est exécuté. Toutefois, la plupart des langages ont un [environnement d'exécution] qui est responsable des tâches telles que le ramassage des miettes (ex: dans Java) ou les fils d'exécution logiciel (ex: les goroutines dans Go). Cet environnement doit être appelé avant `main` puisqu'il a besoin de s'initialiser.

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

En utilisant l'attribut `#[no_mangle]`, nous désactivons la [décoration de nom] pour assurer que le compilateur Rust crée une fonction avec le nom `_start`. Sans cet attribut, le compilateur génèrerait un symbol obscure `_ZN3blog_os4_start7hb173fedf945531caE` pour donner un nom unique à chaque fonction. L'attribut est nécessaire car nous avons besoin d'indiquer le nom de la fonction de point d'entrée à l'éditeur de lien (*linker*) dans l'étape suivante.

Nous devons aussi marquer la fonction avec `extern C` pour indiquer au compilateur qu'il devrait utiliser la [convention de nommage] de C pour cette fonction (au lieu de la convention de nommage de Rust non-spécifiée). Cette fonction se nomme `_start` car c'est le nom par défaut des points d'entrée pour la plupart des systèmes.

[décoration de nom]: https://fr.wikipedia.org/wiki/D%C3%A9coration_de_nom
[convention de nommage]: https://fr.wikipedia.org/wiki/Convention_de_nommage

Le type de retour `!` signifie que la fonction est divergente, c-à-d qu'elle n'a pas le droit de retourner quoi que ce soit. Cela est nécessaire car le point d'entrée n'est pas appelé par une fonction, mais invoqué directement par le système d'exploitation ou par le chargeur d'amorçage. Donc au lieu de retourner une valeur, le point d'entrée doit invoquer l'[appel système `exit`] du système d'exploitation. Dans notre cas, arrêter la machine pourrait être une action convenable, puisqu'il ne reste rien d'autre à faire si un exécutable autoporté s'arrête. Pour l'instant, nous remplissons la condition en bouclant indéfiniement.

[appel système `exit`]: https://fr.wikipedia.org/wiki/Appel_syst%C3%A8me

Quand nous lançons `cargo build`, nous obtenons une erreur de _linker_.

## Erreurs de Linker

Le linker est un programme qui va transformer le code généré en exécutable. Comme le format de l'exécutable differt entre Linux, Windows et macOS, chaque système possède son propre linker qui lève une erreur différente. La cause fondamentale de cette erreur est la même : la configuration par défaut du linker part du principe que notre programme dépend de l'environnement d'exécution de C, ce qui n'est pas le cas.

Pour résoudre les erreurs, nous devons indiquer au linker qu'il ne doit pas inclure l'environnement d'exécution de C. Nous pouvons faire cela soit en passant un ensemble précis d'arguments, soit en compilant pour une cible bare metal.

### Compiler pour une Cible Bare Metal

Par défaut Rust essaie de compiler un exécutable qui est compatible avec l'environnment du système actuel. Par exemple, si vous utilisez Windows avec `x86_64`, Rust essaie de compiler un exécutable Windows `.exe` qui utilises des instructions `x86_64`. Cet environnement est appelé système "hôte".

Pour décrire plusieurs environnements, Rust utilise une chaîne de caractères appelée [_triplé cible_]. Vous pouvez voir le triplé cible de votre système hôte en lançant la commande `rustc --version --verbose` :

[_triplé cible_]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple

```
rustc 1.35.0-nightly (474e7a648 2019-04-07)
binary: rustc
commit-hash: 474e7a6486758ea6fc761893b1a49cd9076fb0ab
commit-date: 2019-04-07
host: x86_64-unknown-linux-gnu
release: 1.35.0-nightly
LLVM version: 8.0
```

La sortie ci-dessus provient d'un système Linux `x86_64`. Nous pouvons voir que le triplé `host` est `x86_64-unknown-linux-gnu`, qui inclut l'architecture du CPU (`x86_64`), le vendeur (`unknown`), le système d'exploitation (`linux`) et l'[ABI] (`gnu`).

[ABI]: https://fr.wikipedia.org/wiki/Application_binary_interface

En compilant pour notre triplé hôte, le compilateur Rust ainsi que le linker supposent qu'il y a un système d'exploitation sous-jacent comme Linux ou Windows qui utilise l'environnement d'exécution C par défaut, ce qui cause les erreurs de linker. Donc pour éviter ces erreurs, nous pouvons compiler pour un environnement différent sans système d'exploitation sous-jacent.

Un exemple d'un tel envrironnement est le triplé cible `thumbv7em-none-eabihf`, qui décrit un système [ARM] [embarqué]. Les détails ne sont pas importants, tout ce qui compte est que le triplé cible n'a pas de système d'exploitation sous-jacent, ce qui est indiqué par le `none` dans le triplé cible. Pour pouvoir compilé pour cette cible, nous avons besoin de l'ajouter dans rustup :

[embarqué]: https://fr.wikipedia.org/wiki/Syst%C3%A8me_embarqu%C3%A9
[ARM]: https://fr.wikipedia.org/wiki/Architecture_ARM

```
rustup target add thumbv7em-none-eabihf
```

Cela télécharge une copie de la bibliothèque standard (et core) pour le système. Maintenant nous pouvons compiler notre exécutable autoporté pour cette cible :

```
cargo build --target thumbv7em-none-eabihf
```

En donnant un argument `--target`, nous effectuons une [compilation croisée][cross_compile] de notre exécutable pour un système bare metal. Comme le système cible n'a pas de système d'exploitation, le linker n'essaie pas de lier l'environnement d'exécution C et notre compilation réussit sans erreur de linker.

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

C'est l'approche que nous allons utiliser pour construire notre noyau d'OS. Plutôt que `thumbv7em-none-eabihf`, nous allons utiliser une [cible personnalisée][custom target] qui décrit un environnement bare metal `x86_64`. Les détails seront expliqués dans le prochain article.

[custom target]: https://doc.rust-lang.org/rustc/targets/custom.html

### Arguments du Linker

Au lieu de compiler pour un système bare metal, il est aussi possible de résoudre les erreurs de linker en passant un ensemble précis d'arguments au linker. Ce n'est pas l'approche que nous allons utiliser pour notre noyau. Cette section est donc optionnelle et fournis uniquement à titre de complétude. Cliquez sur _"Arguments du Linker"_ ci-dessous pour montrer le contenu optionel.

<details>

<summary>Arguments du Linker</summary>

Dans cette section nous allons parler des erreurs de linker qui se produisent sur Linux, Windows et macOS. Nous allons aussi apprendre à résoudre ces erreurs en passant des arguments complémentaires au linker. À noter que le format de l'exécutable et le linker diffèrent entre les systèmes d'exploitation. Il faut donc un ensemble d'arguments différent pour chaque système.

#### Linux

Sur Linux, voici l'erreur de linker qui se produit (raccourcie) :

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

Le problème est que le linker inclut par défaut la routine de démarrage de l'environnement d'exécution de C, qui est aussi appelée `_start`. Elle requiert des symboles de la bibliothèque standard de C `libc` que nous n'incluons pas à cause de l'attribut `no_std`. Le linker ne peut donc pas résoudre ces références. Pour résoudre cela, nous pouvons indiquer au linker qu'il ne devrait pas lier la routine de démarrage de C en passant l'argument `-nostartfiles`.

Une façon de passer des attributs au linker via cargo est la commande `cargo rustc`. Cette commande se comporte exactement comme `cargo build`, mais permet aussi de donner des options à `rustc`, le compilateur Rust sous-jacent. `rustc` possède le flag `-C link-arg`, qui donne un argument au linker. Combinés, notre nouvelle commande ressemble à ceci :
    
```
cargo rustc -- -C link-arg=-nostartfiles
```

Dorénavant notre crate compile en tant qu'exécutable Linux autoporté !

Nous n'avions pas besoin de spécifier le nom de notre point d'entrée de façon explicite car le linker cherche par défaut une fonction nommée `_start`. 
    
#### Windows

Sur Windows, une erreur de linker différente se produit (raccourcie) :

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

Cette erreur signifie que le linker ne peut pas trouver le point d'entrée. Sur Windows, le nom par défaut du point d'entrée [dépend du sous-système utilisé][windows-subsystems]. Pour le sous-système `CONSOLE`, le linker cherche une fonction nommée `mainCRTStartup` et pour le sous-système `WINDOWS`, il cherche une fonction nomée `WinMainCRTStartup`. Pour réécrire la valeur par défaut et indiquer au linker de chercher notre fonction `_start` à la place, nous pouvons donner l'argument `/ENTRY` au linker :

[windows-subsystems]: https://docs.microsoft.com/fr-fr/cpp/build/reference/entry-entry-point-symbol?view=msvc-160

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

Vu le format d'argument différent nous pouvons clairement voir que le linker Windows est un programme totalement différent du linker Linux.

Maintenant une erreur de linker différente se produit :
    
```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

Cette erreur se produit car les exécutables Windows peuvent utiliser différents [sous-systèmes][windows-subsystems]. Pour les programmes normaux, ils sont inférés en fonction du nom du point d'entrée : s'il est nommé `main`, le sous-système `CONSOLE` est utilisé. Si le point d'entrée est nommé `WinMain`, alors le sous-sytème `WINDOWS` est utilisé.  Comme notre fonction `_start` possède un nom différent, nous devons préciser le sous-système explicitement :
    
```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

Ici nous utilisons le sous-système `CONSOLE`, mais le sous-système `WINDOWS` pourrait fonctionner aussi. Au lieu de donner `-C link-arg` plusieurs fois, nous utilisons `-C link-args` qui utilise des arguments séparés par des espaces.
    
Avec cette commande, notre exécutable devrait compiler avec succès sous Windows.
    
#### macOS

Sur macOS, voici l'erreur de linker qui se produit (raccourcie) :

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

Cette erreur nous indique que le linker ne peut pas trouver une fonction de point d'entrée avec le nom par défaut `main` (pour une quelconque raison, toutes les fonctions sur macOS sont précédées de `_`). Pour configurer le point d'entrée sur notre fonction `_start`, nous donnons l'argument `-e` au linker :

```
cargo rustc -- -C link-args="-e __start"
```

L'argument `-e` spécifie le nom de la fonction de point d'entrée. Comme toutes les fonctions ont un préfixe supplémentaire `_` sur macOS, nous devons configurer le point d'entrée comme étant `__start` au lieu de `_start`.
    
Maintenant l'erreur de linker suivante se produit :

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

macOS [ne supporte pas officiellement les bibliothèques liées de façon statique] et necéessite que les programmes lient la bibliothèque `libSystem` par défaut. Pour réécrire ceci et lier une bibliothèque statique, nous donnons l'argument `-static` au linker :

[ne supporte pas officiellement les bibliothèques liées de façon statique]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

Cela ne suffit toujours pas, une troisième erreur de linker se produit :

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

Cette erreur se produit car les programmes sous macOS lient `crt0` (“C runtime zero”) par défaut. Ceci est similaire à l'erreur que nous avions eu sous Linux et peut aussi être résolue en ajoutant l'argument `-nostartfiles` au linker :

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

Maintenant notre programme compile avec succès sous macOS.
    
#### Unifier les Commandes de Compilation

À cet instant nous avons différentes commandes de compilation en fonction de la plateforme hôte, ce qui n'est pas idéal. Pour éviter cela, nous pouvons créer un ficher nommé `.cargo/config.toml` qui contient les arguments spécifiques aux plateformes :

```toml
# dans .cargo/config.toml

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

La clé `rustflags` contient des arguments qui sont automatiquement ajoutés à chaque appel de `rustc`. Pour plus d'informations sur le fichier `.cargo/config.toml`, allez voir la [documentation officielle](https://doc.rust-lang.org/cargo/reference/config.html)
    
Maintenant notre programme devrait être compilable sur les trois plateformes avec un simple `cargo build`.
    
#### Devriez-vous Faire Ça ?

Bien qu'il soit possible de compiler un exécutable autoporté pour Linux, Windows et macOS, ce n'est probablement pas une bonne idée. La raison est que notre exécutable s'attend toujours à trouver certaines choses, par exemple une pile initialisée lorsque la fonction `_start` est appelée. Sans l'environnement d'exécution C, certains de ces conditions peuvent ne pas être remplies, ce qui pourrait faire planter notre programme, avec par exemple une erreur de segmentation.

Si vous voulez créer un exécutable minimal qui tourne sur un système d'exploitation existant, include `libc` et mettre l'attribut `#[start]` come décrit [ici](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html) semble être une meilleure idée.
    
</details>

## Résumé

Un exécutable Rust autoporté minimal ressemble à ceci :

`src/main.rs`:

```rust
#![no_std] // ne pas lier la bibliothèque standard Rust
#![no_main] // désactiver tous les points d'entrée au niveau de Rust

use core::panic::PanicInfo;

#[no_mangle] // ne pas décorer le nom de cette fonction
pub extern "C" fn _start() -> ! {
    // cette fonction est le point d'entrée, comme le linker cherche une fonction
    // nomée `_start` par défaut
    loop {}
}

/// Cette fonction est appelée à chaque panic.
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

# le profile utilisé pour `cargo build`
[profile.dev]
panic = "abort" # désactive le déroulement de la pile lors d'un panic

# le profile utilisé pour `cargo build --release`
[profile.release]
panic = "abort" # désactive le déroulement de la pile lors d'un panic
```

Pour compiler cet exécutable, nous devons compiler pour une cible bare metal telle que `thumbv7em-none-eabihf` :

```
cargo build --target thumbv7em-none-eabihf
```

Sinon, nous pouvons aussi compiler pour le système hôte en donnant des arguments supplémentaires pour le linker :

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

À noter que ceci est juste un exemple minimal d'un exécutable Rust autoporté. Cet exécutable s'attend à de nombreuses choses, comme par exemple le fait qu'une pile soit initialisée lorsque la fonction `_start` est appelée. **Donc pour une réelle utilisation d'un tel exécutable, davantages d'étapes sont requises.**

## Et ensuite ?

Le [poste suivant][next post] explique les étapes nécessaires pour transformer notre exécutable autoporté minimal en noyau de système d'opération. Cela comprend la création d'une cible personnalisée, l'intégration de notre exécutable avec un chargeur d'amorçage et l'apprentissage de comment imprimer quelque chose sur l'écran.

[next post]: @/edition-2/posts/02-minimal-rust-kernel/index.md
