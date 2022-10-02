+++
title = "Un noyau Rust minimal"
weight = 2
path = "fr/minimal-rust-kernel"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "c689ecf810f8e93f6b2fb3c4e1e8b89b8a0998eb"
# GitHub usernames of the people that translated this post
translators = ["TheMimiCodes", "maximevaillancourt"]
+++

Dans cet article, nous créons un noyau Rust minimal 64-bit pour l'architecture x86. Nous continuons le travail fait dans l'article précédent [freestanding Rust binary] pour créer une image de disque amorçable qui imprime quelque chose à l'écran. 

[freestanding Rust binary]: @/edition-2/posts/01-freestanding-rust-binary/index.md

<!-- more -->

Cet article est développé ouvertement sur [GitHub]. Si vous avez des problèmes ou des questions, veuillez ouvrir une Issue sur GitHub. Vous pouvez aussi laisser un commentaire [au bas de la page]. Le code source complet pour cet article peut être trouvé dans la branche [`post-02`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[au bas de la page]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-02

<!-- toc -->

## The Boot Process
QUand vous ouvrez un ordinateur, il commence à exécuter le code du micrologiciel qui est enregistré dans la carte maîtresse[ROM]. Ce code performe un [power-on self-test], détecte la mémoire volatile disponible, et pré-initialise le CPU et le matériel. Par la suite, il recherche un disque amorçable et commence le processus d'amorçage du noyau du système d'exploitation. 

[ROM]: https://en.wikipedia.org/wiki/Read-only_memory
[power-on self-test]: https://en.wikipedia.org/wiki/Power-on_self-test

Sur x86, il y a deux standards pour les micrologiciels: le “Basic Input/Output System“ (**[BIOS]**) et le nouvel “Unified Extensible Firmware Interface” (**[UEFI]**). Le BIOS standard est vieux et dépassé, mais il est simple et bien suporté sur toutes les machines x86 depuis les années 1980. Au contraire, l'UEFI, est plutôt moderne et il offre davantage de fonctionnalités, cependant, il est plus complexe à installer (du moins, selon moi).

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[UEFI]: https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

Actuellement, nous offrons seulement un support BIOS, mais nous planifions aussi du support pour l'UEFI. Si vous aimeriez nous aider avec cela, consultez [Github issue](https://github.com/phil-opp/blog_os/issues/349).

### BIOS Boot
Presque tous les systèmes x86 ont des support pour amorcer le BIOS, incluant les récentes machine UEFI-based qui utilisent un BIOS émulé. C'est bien étant donné que vous pouvez utiliser la même logique d'armorçage sur toutes les machines du dernier siècle. De plus, cette grande compatibilité est à la fois le plus grand désavantage de l'amorçage BIOS. En effet, cela signifie que le CPU est mis dans un mode de compatibilité 16-bit appelé [real mode] avant l'amorçage afin que les bootloaders archaïques des années 1980 puissent encore fonctionner. 

Commençons par le commencement: 

Quand vous ouvrez votre ordinateur, il charge le BIOS provenant d'un emplacement de mémoire flash spéciale localisée sur la carte maîtresse. Le BIOS exécute des tests d'auto-diagnostic et es routines d'initialisation du matériel, puis il cherche des disques amorçables. S'il en trouve un, le contrôle est transféré à its _bootloader_, qui est une portion 512-byte du code exécutable enregistré au début du disque. La pluplart des bootloaders sont plus gros que 512 bytes, alors les bootloaders sont communément séparés en deux étapes. La première étape, est de 512 bytes, et la seconde étape, est chargé subséquemment à la première étapge. 

Le bootloader doit déterminé la localisation de l'image de noyau sur le disque et de la télécharger dans sa mémoire. Il doit aussi transformer le CPU 16-bit [real mode] en un 32-bit [protected mode], puis en un 64-bit [long mode], où les registres 64-bit et la mémoire maîtresse complète sont disponibles. Sa troisième tâche est de récupérer certaines informations (telle que les associations mémoires) du BIOS et les passer au noyau du système d'exploitation. 

[real mode]: https://en.wikipedia.org/wiki/Real_mode
[protected mode]: https://en.wikipedia.org/wiki/Protected_mode
[long mode]: https://en.wikipedia.org/wiki/Long_mode
[memory segmentation]: https://en.wikipedia.org/wiki/X86_memory_segmentation

Implémenter un bootloader est délicat puisque cela requiert l'écriture de code assembleur et plusieurs autres étapes particulières comme “write this magic value to this processor register”. Par conséquent, nous ne couvrons pas la création d'un bootoader dans cet article et nous fournissons plutôt un outil appelé [bootimage] qui ajoute automatiquement un bootloader à votre noyau.

[bootimage]: https://github.com/rust-osdev/bootimage

Si vous êtes intéressé à créer votre propre booloader : Gardez l'oeil ouvert, plusieurs articles sur ce sujet sont déjà prévus! <!-- , check out our “_[Writing a Bootloader]_” posts, where we explain in detail how a bootloader is built. -->

#### The Multiboot Standard
Pour éviter que chaque système d'opération implémente son propre bootloader, qui est seulement compatible avec un seul système d'exploitation, le [Free Software Foundation] à créé un bootloader public standard appelé [Multiboot] en 1995. Le standard défini une interface entre le bootloader et le système d'opération afin que n'importe quel Multiboot-compliant bootloader puisse charger n'importe quel système d'opérations Multiboot-compliant. La référence d'implementation est [GNU GRUB], qui est le bootloader le plus populaire pour les systèmes Linux. 

[Free Software Foundation]: https://en.wikipedia.org/wiki/Free_Software_Foundation
[Multiboot]: https://wiki.osdev.org/Multiboot
[GNU GRUB]: https://en.wikipedia.org/wiki/GNU_GRUB

Pour faire un noyau conforme à la spécification Multiboot, il faut seulement inséré un [Multiboot header] au début du fichier du noyau. Cela fait en sorte que c'est très simple to boot un système d'exploitation depuis GRUB. Cependant, le GRUB et le Multiboot standard présentent aussi des problèmes : 

[Multiboot header]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format

- Ils supportent seulement le mode de protection 32-bit. Cela signifie que si vous devez encore faire la configuration du CPU pour changer au 64-bit long mode.
- Ils sont désignés pour faire le bootloader simple plutôt que le noyau. Par exemple, le noyau doit être lié avec un [adjusted default page size], étant donné que le GRUB ne peut pas trouver les entêtes Multiboot autrement. Un autre exemple est que le [boot information], qui est passé au noyau, contient plusieurs structures spécifiques à l'architecture plutôt que de fournir des abstractions pures. 
- GRUB et le standard Multiboot sont peu documentés.
- GRUB doit être installé sur un système hôte pour créer une image de disque amorçable depuis le fichier du noyau. Cela rend le développement sur Windows ou sur Mac plus difficile.

[adjusted default page size]: https://wiki.osdev.org/Multiboot#Multiboot_2
[boot information]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format

En raison de ces désavantages, nous avons décidé de ne pas utiliser GRUB ou le standard Multiboot. Cependant, nous avons planifié d'ajouter un support Multiboot à notre outil [bootimage], afin qu'il soit aussi possible de charger votre noyau sur un système  GRUB. Si vous êtes interessé à écrire un noyau Multiboot conforme, consultez la [first edition] de cette série d'articles. 

[first edition]: @/edition-1/_index.md

### UEFI

(Nous ne fournissons pas le support UEFI à l'heure actuelle, mais nous aimerions bien! Si vous êtes intéressé à aider, dites-le nous dans le [Github issue](https://github.com/phil-opp/blog_os/issues/349).)

## A Minimal Kernel
Maintenant que nous savons à peu près comment un ordinateur démarre, c'est le temps de créer notre propre noyau minimal. Notre objectif est de créé une image de disque qui imprime “Hello World!” à l'écran lorsqu'il démarre. Nous faisons ceci en améliorant le [freestanding Rust binary][binaire Rust autonome] du dernier article.

Comme vous vous en rappelez peut-être, nous avons bâti un binaire autonome grâce à `cargo`, mais selon le système d'opérations, nous avions besoin de différents points d'entrée et d'options de compilation.  Ceci est dû au fait que `cargo` construit pour le the _host system_ by default, i.e., le système que vous utilisez. Ce n'est pas quelque chose que nous voulons pour notre noyau, puisqu'un noyau exécute par dessus, e.g., Windows, ce qui ne fait pas de sens. À la place, nous voulons compiler un système cible bien défini _target system_.

### Installing Rust Nightly
Rust a trois canaux de distribution :  _stable_, _beta_, et _nightly_. The Rust Book explique bien les différences entre  ces canaux, alors prenez une minute et [check it out](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html#choo-choo-release-channels-and-riding-the-trains). Pour construire un système d'opérations, nous aurons besoin de fonctionalités expérimentales qui sont disponibles uniquement sur le canal de distribution nocturne, alors nous devons installer une version nocturne de Rust. 

Pour gérer l'installation de Rust, je recommande fortement [rustup]. Cela vous permet d'installer la version nocturne, beta and stable compilers côte-à-côte et facilite leur mise à jour. Avec rustup, vous pouvez utiliser un canal de distribution nocturne pour les directives actuelles en excéutant `rustup override set nightly`. Par ailleurs, vous pouvez ajouter un fichier appelé `rust-toolchain` avec le contenu `nightly` au dossier racine du projet. Vous pouvez vérifier que vous avez une version nocturne installée en exécutant `rustc --version`: Le numéro de la version devrait comprendre `-nightly` à la fin.

[rustup]: https://www.rustup.rs/

La version nocturne du compilateur nous permet d'activer certaines fonctionnalités expérimentales en utilisant certains _drapeaux de fonctionalité_ dans le haut de notre fichier. Par exemple, nous pourrions activer [`asm!` macro][macro expérimentale `asm!`] pour écrire du code assembleur intégré en ajoutant `#![feature(asm)]` au haut de notre `main.rs`. Noter que ces fonctionnalités expérimentales sont tout à fait instables, ce qui veut dire que des versions futures de Rust pourraient les changer ou les retirer sans préavis. Pour cette raison, nous les utiliserons seulement lorsque strictement nécessaire.

[`asm!` macro]: https://doc.rust-lang.org/stable/reference/inline-assembly.html

### Spécification de cible
Cargo supporte différent systèmes cibles avec le paramètre `--target`. La cible est définie par un soi-disant _[target triple][triplet de cible]_, qui décrit l'architecteur du processeur, le fabricant, le système d'exploitation, et l'interface binaire d'application ([ABI]). Par exemple, le triplet `x86_64-unknown-linux-gnu` décrit un système avec un processeur `x86_64`, pas de fabricant défini, et un système d'exploitation Linux avec l'interface binaire d'application GNU. Rust supporte [plusieurs différents triplets de cible][platform-support], incluant `arm-linux-androideabi` pour Android ou [`wasm32-unknown-unknown` pour WebAssembly](https://www.hellorust.com/setup/wasm-target/).

[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[ABI]: https://stackoverflow.com/a/2456882
[platform-support]: https://forge.rust-lang.org/release/platform-support.html
[custom-targets]: https://doc.rust-lang.org/nightly/rustc/targets/custom.html

Pour notre système cible toutefois, nous avons besoin de paramètres de configuration spéciaux (par exemple, pas de système d'explotation sous-jacent), donc aucun des [triplets de cible existants][platform-support] ne convient. Heureusement, Rust nous permet de définir [notre propre cible][custom-targets] par l'entremise d'un fichier JSON. Par exemple, un fichier JSON qui décrit une cible `x86_64-unknown-linux-gnu` ressemble à ceci:

```json
{
    "llvm-target": "x86_64-unknown-linux-gnu",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "linux",
    "executables": true,
    "linker-flavor": "gcc",
    "pre-link-args": ["-m64"],
    "morestack": false
}
```

La plupart des champs sont requis par LLVM pour générer le code pour cette plateforme. Par exemple, le champ [`data-layout`] définit la taille de divers types d'entiers, de nombres à virgule flottante, et de pointeurs. Puis, il y a des champs que Rust utilise pour de la compilation conditionelle, comme `target-pointer-width`. Le troisième type de champ définit comme une caisse doit être construite. Par exemple, le champ `pre-link-args` spécifie les arguments fournis au [linker][lieur].

[`data-layout`]: https://llvm.org/docs/LangRef.html#data-layout
[linker]: https://en.wikipedia.org/wiki/Linker_(computing)

Nous pouvons aussi cibler les systèmes `x86_64` avec notre noyau, donc notre spécification de cible ressemblera beaucoup à celle plus haut. Commençons par créer un fichier `x86_64-blog_os.json` (utilisez le nom de votre choix) avec ce contenu commun:

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executables": true
}
```

Noter que nous avons changé le système d'exploitation dans le champs `llvm-target` et `os` pour `none`, puisque nous ferons l'exécution sur du "bare metal" (pas de système d'exploitation sous-jacent).

Nous ajoutons ensuite les champs suivants reliés à la construction:


```json
"linker-flavor": "ld.lld",
"linker": "rust-lld",
```

Plutôt que d'utiliser le lieur par défaut de la plateforme (qui pourrait ne pas supporter les cibles Linux), nous utilisons le lieur multi-plateforme [LLD] qui est inclut avec Rust pour lier notre noyau.

[LLD]: https://lld.llvm.org/

```json
"panic-strategy": "abort",
```

Ce paramètre spécifie que la cible ne permet pas le [stack unwinding][déroulement de la pile] lorsque le noyau panique, alors le système devrait plutôt s'arrêter directement. Ceci mène au même résultat que l'option `panic = "abort"` dans notre Cargo.toml, alors nous pouvons la retirer de ce fichier. (Noter que, contrairement à l'option Cargo.toml, cette option de cible s'applique aussi quand nous recompilerons la bibliothèque `core` plus loin dans cet article. Ainsi, même si vous préférez garder l'option Cargo.toml, gardez cette option.)

[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php

```json
"disable-redzone": true,
```

Nous écrivons un noyau, donc nous devrons éventuellement gérer les interruptions. Pour ce faire en toute sécurité, nous devons désactiver une optimisation de pointeur de pile nommée la _“zone rouge", puisqu'elle causerait une corruption de la pile autrement. Pour plus d'informations, lire notre article séparé à propos de la [disabling the red zone][désactivation de la zone rouge].

[disabling the red zone]: @/edition-2/posts/02-minimal-rust-kernel/disable-red-zone/index.md

```json
"features": "-mmx,-sse,+soft-float",
```

Le champ `features` active/désactive des fonctionalités de la cible. Nous désactivons les fonctionalités `mmx` et `sse` en les précédant d'un signe "moins" et activons la fonctionnalité `soft-float` en la précédant d'un signe "plus". Noter qu'il ne doit pas y avoir d'espace entre les différentes fonctionnalités, sinon LLVM n'arrive pas à analyser la chaîne de caractères des fonctionnalités.

Les fonctionnalités `mmx` et `sse` déterminent le support les instructions [Single Instruction Multiple Data (SIMD)], qui peuvent souvent significativement accélérer les programmes. Toutefois, utiliser les grands registres SIMD dans les noyaux des systèmes d'exploitation mène à des problèmes de performance. Ceci arrive puisque le noyau a besoin de restaurer tous les registres à leur état original avant de continuer un programme interrompu. Cela signifie que le noyau doit enregistrer l'état SIMD complet dans la mémoire principale à chaque appel système ou interruption matérielle. Puisque l'état SIMD est très grand (512–1600 octets) et que les interruptions peuvent survenir très fréquemment, ces opérations d'enregistrement/restauration additionnelles nuisent considérablement à la performance. Pour prévenir cela, nous désactivons SIMD pour notre noyau (pas pour les applications qui s'exécutent dessus!).

[Single Instruction Multiple Data (SIMD)]: https://en.wikipedia.org/wiki/SIMD

Un problème avec la désactivation de SIMD est que les opérations sur les nombres à virgule flottante sur `x86_64` nécessitent les registres SIMD par défaut. Pour résoudre ce problème, nous ajoutons la fonctionnalité `soft-float`, qui émule toutes les opérations à virgule flottante avec des fonctions logicielles utilisant des entiers normaux.

Pour plus d'informations, voir notre article sur la [désactivation de SIMD](@/edition-2/posts/02-minimal-rust-kernel/disable-simd/index.md).

#### Putting it Together
Our target specification file now looks like this:

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executables": true,
    "linker-flavor": "ld.lld",
    "linker": "rust-lld",
    "panic-strategy": "abort",
    "disable-redzone": true,
    "features": "-mmx,-sse,+soft-float"
}
```

### Building our Kernel
Compiling for our new target will use Linux conventions (I'm not quite sure why; I assume it's just LLVM's default). This means that we need an entry point named `_start` as described in the [previous post]:

[previous post]: @/edition-2/posts/01-freestanding-rust-binary/index.md

```rust
// src/main.rs

#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}
```

Note that the entry point needs to be called `_start` regardless of your host OS.

We can now build the kernel for our new target by passing the name of the JSON file as `--target`:

```
> cargo build --target x86_64-blog_os.json

error[E0463]: can't find crate for `core`
```

It fails! The error tells us that the Rust compiler no longer finds the [`core` library]. This library contains basic Rust types such as `Result`, `Option`, and iterators, and is implicitly linked to all `no_std` crates.

[`core` library]: https://doc.rust-lang.org/nightly/core/index.html

The problem is that the core library is distributed together with the Rust compiler as a _precompiled_ library. So it is only valid for supported host triples (e.g., `x86_64-unknown-linux-gnu`) but not for our custom target. If we want to compile code for other targets, we need to recompile `core` for these targets first.

#### The `build-std` Option

That's where the [`build-std` feature] of cargo comes in. It allows to recompile `core` and other standard library crates on demand, instead of using the precompiled versions shipped with the Rust installation. This feature is very new and still not finished, so it is marked as "unstable" and only available on [nightly Rust compilers].

[`build-std` feature]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std
[nightly Rust compilers]: #installing-rust-nightly

To use the feature, we need to create a [cargo configuration] file at `.cargo/config.toml` with the following content:

```toml
# in .cargo/config.toml

[unstable]
build-std = ["core", "compiler_builtins"]
```

This tells cargo that it should recompile the `core` and `compiler_builtins` libraries. The latter is required because it is a dependency of `core`. In order to recompile these libraries, cargo needs access to the rust source code, which we can install with `rustup component add rust-src`.

<div class="note">

**Note:** The `unstable.build-std` configuration key requires at least the Rust nightly from 2020-07-15.

</div>

After setting the `unstable.build-std` configuration key and installing the `rust-src` component, we can rerun our build command:

```
> cargo build --target x86_64-blog_os.json
   Compiling core v0.0.0 (/…/rust/src/libcore)
   Compiling rustc-std-workspace-core v1.99.0 (/…/rust/src/tools/rustc-std-workspace-core)
   Compiling compiler_builtins v0.1.32
   Compiling blog_os v0.1.0 (/…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

We see that `cargo build` now recompiles the `core`, `rustc-std-workspace-core` (a dependency of `compiler_builtins`), and `compiler_builtins` libraries for our custom target.

#### Détails reliés à la mémoire

Le compilateur Rust assume qu'un certain ensemble de fonctions intégrées sont disponibles pour tous les systèmes. La plupart de ces fonctions sont fournies par la caisse `compiler_builtins` que nous venons de recompiler. Toutefois, certaines fonctions liées à la mémoire dans cette caisse ne sont pas activées par défaut puisqu'elles sont normalement fournies par la bibliothèque C sur le système. Parmi ces fonctions, on retrouve `memset`, qui définit tous les octets dans un bloc mémoire à une certaine valeur, `memcpy`, qui copie un bloc mémoire vers un autre, et `memcmp`, qui compare deux blocs mémoire. Alors que nous n'avions pas besoin de ces fonctions pour compiler notre noyau maintenant, elles seront nécessaires aussitôt que nous lui ajouterons plus de code (par exemple, lorsque nous copierons des `struct`).

Puisque nous ne pouvons pas lier avec la bibliothèque C du système d'exploitation, nous avons besoin d'une méthode alternative de fournir ces fonctions au compilateur. Une approche possible pour ce faire serait d'implémenter nos propre fonctions `memset`, etc. et de leur appliquer l'attribut `#[no_mangle]` (pour prévenir le changement de nom automatique pendant la compilation). Or, ceci est dangereux puisque toute erreur dans l'implémentation pourrait mener à un comportement indéterminé. Par exemple, implémenter `memcpy` avec une boucle `for` pourrait mener à une recursion infinie puisque les boucles `for` invoquent implicitement la méthode caractéristique [`IntoIterator::into_iter`], qui pourrait invoquer `memcpy` de nouveau. C'est donc une bonne idée de plutôt réutiliser des implémentations existantes et éprouvées.

[`IntoIterator::into_iter`]: https://doc.rust-lang.org/stable/core/iter/trait.IntoIterator.html#tymethod.into_iter

Heureusement, la caisse `compiler_builtins` contient déjà des implémentations pour toutes les fonctions nécessaires, elles sont seulement désactivées par défaut pour ne pas interférer avec les implémentations de la bibliothèque C. Nous pouvons les activer en définissant le drapeau [`build-std-features`] de cargo à `["compiler-builtins-mem"]`. Comme pour le drapeau `build-std`, ce drapeau peut être soit fourni en ligne de commande comme drapeau `-Z` ou configuré dans la table `unstable` du fichier `.cargo/config.toml`. Puisque nous voulons toujours construire avec ce drapeau, l'option du fichier de configuration fait plus de sens pour nous:

[`build-std-features`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features

```toml
# dans .cargo/config.toml

[unstable]
build-std-features = ["compiler-builtins-mem"]
build-std = ["core", "compiler_builtins"]
```

(Le support pour la fonctionnalité `compiler-builtins-mem` a [été ajouté assez récemment](https://github.com/rust-lang/rust/pull/77284), donc vous aurez besoin de la version nocturne `2020-09-30` de Rust ou plus récent pour l'utiliser.)

Dans les coulisses, ce drapeau active la [`mem` feature][fonctionnalité `mem`] de la caisse `compiler_builtins`. Le résultat est que l'attribut `#[no_mangle]` est appliqué aux [`memcpy` etc. implementations][implémentations `memcpy` et autres] de la caise, ce qui les rend disponible au lieur.

[`mem` feature]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/Cargo.toml#L54-L55
[`memcpy` etc. implementations]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/src/mem.rs#L12-L69

Avec ce changement, notre noyau a des implémentations valides pour toutes les fonctions requises par le compilateur, donc il peut continuer à compiler même si notre code devient plus complexe.

#### Définir une cible par défaut

Pour ne pas avoir à fournir le paramètre `--target` à chaque invocation de `cargo build`, nous pouvons définir la cible par défaut. Pour ce faire, nous ajoutons le code suivant à notre fichier de [cargo configuration][configuration Cargo] dans `.cargo/config.toml`:

[cargo configuration]: https://doc.rust-lang.org/cargo/reference/config.html

```toml
# dans .cargo/config.toml

[build]
target = "x86_64-blog_os.json"
```

Ceci indique à `cargo` d'utiliser notre cible `x86_64-blog_os.json` quand il n'y a pas d'argument de cible `--target` explicitement fourni. Ceci veut dire que nous pouvons maintenant construire notre noyau avec un simple `cargo build`. Pour plus d'informations sur les options de configuration cargo, jetez un coup d'oeil à la [official documentation][documentation officielle de cargo].

Nous pouvons maintenant construire notre noyau pour une cible "bare metal" avec un simple `cargo build`. Toutefois, notre point d'entrée `_start`, qui sera appelé par le bootloader, est encore vide. Il est temps de lui faire imprimer quelque chose à l'écran.

### Imprimer à l'écran
La façon la plus facile d'imprimer à l'écran à ce stade est grâce au tampon texte VGA. C'est un emplacement mémoire spécial associé au matériel VGA qui contient le contenu affiché à l'écran. Il consiste normalement en 25 lines qui contiennent chacune 80 cellules de caractère. Chaque cellule de caractère affiche un caractère ASCII avec des couleurs d'avant-plan et d'arrière-plan. Le résultat à l'écran ressemble à ceci:

[VGA text buffer]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode

![sortie à l'écran pour des caractères ASCII ordinaires](https://upload.wikimedia.org/wikipedia/commons/f/f8/Codepage-437.png)

Nous discuterons de la disposition exacte du tampon VGA dans le prochain article, où nous lui écrirons un premier petit pilote. Pour afficher “Hello World!”, nous devons seulement savoir que le tampon est situé à l'adresse `0xb8000` et que chaque cellule de caractère consiste en un octet ASCII et un octet de couleur.

L'implémentation ressemble à ceci :

```rust
static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}
```

D'abord, nous transformons l'entier `0xb8000` en un [raw pointer][pointeur brut]. Puis nous [iterate][parcourons] les octets de la [byte string][chaîne d'octets] [static][statique] `HELLO`. Nous utilisons la méthode [`enumerate`] pour aussi obtenir une variable `i`. Dans le corps de la boucle `for`, nous utilisons la méthode [`offset`] pour écrire la chaîne d'octets et l'octet de couleur correspondant(`0xb` est un cyan pâle).

[iterate]: https://doc.rust-lang.org/stable/book/ch13-02-iterators.html
[static]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate
[byte string]: https://doc.rust-lang.org/reference/tokens.html#byte-string-literals
[raw pointer]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

Noter qu'il y a un bloc [`unsafe`] qui enveloppe les écritures mémoire. La raison est que le compilateur Rust ne peut pas prouver que les pointeurs bruts que nous créons sont valides. Ils pourraient pointer n'importe où et mener à une corruption de données. En les mettant dans un bloc `unsafe`, nous disons fondamentalement au compilateur que nous sommes absolument certains que les opérations sont valides. Noter qu'un bloc `unsafe` ne désactive pas les contrôles de sécurité de Rust. Il permet seulement de faire [five additional things][cinq choses supplémentaires].

[`unsafe`]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html
[five additional things]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html#unsafe-superpowers

Je veux souligner que **ceci n'est pas la façon de faire les choses en Rust!** Il est très facile de faire un gâchis en travaillant avec des pointeurs bruts à l'intérieur de blocs `unsafe`. Par exemple, nous pourrions facilement écrire au-delà de la fin du tampon si nous ne sommes pas prudents.

Alors nous voulons minimiser l'utilisation de `unsafe` autant que possible. Rust nous offre la possibilité de faire cela en créant des abstractions sécuritaires. Par exemple, nous pourrions créer un type tampon VGA qui encapsule les risques et qui s'assure qu'il est impossible de faire quoi que ce soit d'incorrect à l'extérieur de ce type. Ainsi, nous aurions besoin de très peu de code `unsafe` and nous serions certains que nous ne violons pas la [memory safety][sécurité de mémoire]. Nous allons créer une telle abstraction de tampon VGA buffer dans le prochain article.

[memory safety]: https://en.wikipedia.org/wiki/Memory_safety

## Exécuter notre noyau

Maintenant que nous avons un exécutable qui fait quelque chose de perceptible, il est temps de l'exécuter. D'abord, nous devons transformer notre noyau compilé en une image de disque amorçable en le liant à un bootloader. Ensuite, nous pourrons exécuter l'image de disque dans une machine virtuelle [QEMU] ou l'amorcer sur du véritable matériel en utilisant une clé USB.

### Créer une image d'amorçage

Pour transformer notre noyau compilé en image de disque amorçable, nous devons le lier avec un bootloader. Comme nous l'avons appris dans la [section about booting][section à propos du lancement], le bootloader est responsable de lancer le CPU et de charger notre noyau.

[section about booting]: #the-boot-process

Plutôt que d'écrire notre propre bootloader, ce qui est un projet en soi, nous utilisons la caisse [`bootloader`]. Cette caisse propose un bootloader BIOS de base sans dépendance C, seulement du code Rust et de l'assembleur intégré. Pour l'utiliser pour lancer notre noyau, nous devons ajouter une dépendance pour cette caisse:

[`bootloader`]: https://crates.io/crates/bootloader

```toml
# dans Cargo.toml

[dependencies]
bootloader = "0.9.8"
```

Ajouter le bootloader comme dépendance n'est pas suffisant pour réellement créer une image de disque amorçable. Le problème est que nous devons lier notre noyau avec le bootloader après la compilation, mais cargo ne supporte pas les [post-build scripts][scripts post-build].

[post-build scripts]: https://github.com/rust-lang/cargo/issues/545

Pour résoudre ce problème, nous avons créé un outil nommé `bootimage` qui compile d'abord le noyau et le bootloader, et les lie ensuite ensemble pour créer une image de disque amorçable. Pour installer cet outil, exécutez la commande suivante dans votre terminal:

```
cargo install bootimage
```

Pour exécuter `bootimage` et construire le bootloader, vous devez avoir la composante rustup `llvm-tools-preview` installée. Vous pouvez l'installer en exécutant `rustup component add llvm-tools-preview`.

Après avoir installé `bootimage` and ajouté la composante `llvm-tools-preview`, nous pouvons créer une image de disque amorçable en exécutant:

```
> cargo bootimage
```

Nous voyons que l'outil recompile notre noyau en utilisant `cargo build`, alors il utilisera automatiquement tout changement que vous faites. Ensuite, il compile le bootloader, ce qui peut prendre un certain temps. Comme toutes les dépendances de caisses, il est seulement construit une fois puis il est mis en cache, donc les builds subséquentes seront beaucoup plus rapides. Enfin, `bootimage` combine le bootloader et le noyau en une image de disque amorçable.

Après avoir exécuté la commande, vous devriez voir une image de disque amorçable nommée `bootimage-blog_os.bin` dans votre dossier `target/x86_64-blog_os/debug`. Vous pouvez la lancer dans une machine virtuelle ou la copier sur une clé USB pour la lancer sur du véritable matériel. (Notez que ceci n'est pas une image CD, qui est un format différent, donc la graver sur un CD ne fonctionne pas).

#### Comment cela fonctionne-t-il?

L'outil `bootimage` effectue les étapes suivantes en arrière-plan:

- Il compile notre noyau en un fichier [ELF].
- Il compile notre dépendance bootloader en exécutable autonome.
- Il lie les octets du fichier ELF noyau au bootloader.

[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
[rust-osdev/bootloader]: https://github.com/rust-osdev/bootloader

Lorsque lancé, le bootloader lit et analyse le fichier ELF ajouté. Il associe ensuite les segments du programme aux adresses virtuelles dans les tables de pages, réinitialise la section `.bss`, puis met en place une pile. Finalement, il lit le point d'entrée (notre fonction `_start`) et s'y rend.

### Amorçage dans QEMU

Nous pouvons maintenant lancer l'image disque dans une machine virtuelle. Pour la démarrer dans [QEMU], exécutez la commande suivante :

[QEMU]: https://www.qemu.org/

```
> qemu-system-x86_64 -drive format=raw,file=target/x86_64-blog_os/debug/bootimage-blog_os.bin
warning: TCG doesn't support requested feature: CPUID.01H:ECX.vmx [bit 5]
```

Ceci ouvre une fenêtre séparée qui devrait ressembler à cela:

![QEMU showing "Hello World!"](qemu.png)

Nous voyoons que notre "Hello World!" est visible à l'écran.

### Véritable ordinateur

Il est aussi possible d'écrire l'image disque sur une clé USB et de le lancer sur un véritable ordinateur, **mais soyez prudent** et choisissez le bon nom de périphérique, parce que **tout sur ce périphérique sera écrasé**:

```
> dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

Où `sdX` est le nom du périphérique de votre clé USB.

Après l'écriture de l'image sur votre clé USB, vous pouvez l'exécuter sur du véritable matériel en l'amorçant à partir de la clé USB. Vous devrez probablement utiliser un menu d'amorçage spécial ou changer l'ordre d'amorçage dans votre configuration BIOS pour amorcer à partir de la clé USB. Notez que cela ne fonctionne actuellement pas avec des ordinateurs UEFI, puisque la caisse `bootloader` ne supporte pas encore UEFI.

### Utilisation de `cargo run`

Pour faciliter l'exécution de notre noyau dans QEMU, nous pouvons définir la clé de configuration `runner` pour cargo:

```toml
# dans .cargo/config.toml

[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```

La table `target.'cfg(target_os = "none")'` s'applique à toutes les cibles dont le champ `"os"` dans le fichier de configuration est défini à `"none"`. Ceci inclut notre cible `x86_64-blog_os.json`. La clé `runner` key spécifie la commande qui doit être invoquée pour `cargo run`. La commande est exécutée après une build réussie avec le chemin de l'exécutable comme premier argument. Voir la [configuration cargo][cargo configuration] pour plus de détails.

La commande `bootimage runner` est spécifiquement conçue pour être utilisable comme un exécutable `runner`. Elle lie l'exécutable fourni  avec le bootloader duquel le projet dépend et lance ensuite QEMU. Voir le [Readme of `bootimage`][README de `bootimage`] pour plus de détails et les options de configuration possibles.

[Readme of `bootimage`]: https://github.com/rust-osdev/bootimage

Nous pouvons maintenant utiliser `cargo run` pour compiler notre noyau et le lancer dans QEMU.

## Et ensuite?

Dans le prochain article, nous explorerons le tampon texte VGA plus en détails et nous écrirons une interface sécuritaire pour l'utiliser. Nous allons aussi mettre en place la macro `println`.
