+++
title = "Un noyau Rust minimal"
weight = 2
path = "fr/minimal-rust-kernel"
date = 2018-02-10

[extra]
# Please update this when updating the translation
translation_based_on_commit = "c689ecf810f8e93f6b2fb3c4e1e8b89b8a0998eb"
# GitHub usernames of the people that translated this post
translators = ["TheMimiCodes", "maximevaillancourt"]
# GitHub usernames of the people that contributed to this translation
translation_contributors = ["alaincao"]
+++

Dans cet article, nous créons un noyau Rust 64-bit minimal pour l'architecture x86. Nous continuons le travail fait dans l'article précédent “[Un binaire Rust autonome][freestanding Rust binary]” pour créer une image de disque amorçable qui affiche quelque chose à l'écran. 

[freestanding Rust binary]: @/edition-2/posts/01-freestanding-rust-binary/index.fr.md

<!-- more -->

Cet article est développé de manière ouverte sur [GitHub]. Si vous avez des problèmes ou des questions, veuillez ouvrir une _Issue_ sur GitHub. Vous pouvez aussi laisser un commentaire [au bas de la page]. Le code source complet pour cet article se trouve dans la branche [`post-02`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[au bas de la page]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-02

<!-- toc -->

## Le processus d'amorçage
Quand vous allumez un ordinateur, il commence par exécuter le code du micrologiciel qui est enregistré dans la carte mère ([ROM]). Ce code performe un [test d'auto-diagnostic de démarrage][power-on self-test], détecte la mémoire volatile disponible, et pré-initialise le processeur et le matériel. Par la suite, il recherche un disque amorçable et commence le processus d'amorçage du noyau du système d'exploitation.

[ROM]: https://fr.wikipedia.org/wiki/M%C3%A9moire_morte
[power-on self-test]: https://fr.wikipedia.org/wiki/Power-on_self-test_(informatique)

Sur x86, il existe deux standards pour les micrologiciels : le “Basic Input/Output System“ (**[BIOS]**) et le nouvel “Unified Extensible Firmware Interface” (**[UEFI]**). Le BIOS standard est vieux et dépassé, mais il est simple et bien supporté sur toutes les machines x86 depuis les années 1980. Au contraire, l'UEFI est moderne et offre davantage de fonctionnalités. Cependant, il est plus complexe à installer (du moins, selon moi).

[BIOS]: https://fr.wikipedia.org/wiki/BIOS_(informatique)
[UEFI]: https://fr.wikipedia.org/wiki/UEFI

Actuellement, nous offrons seulement un support BIOS, mais nous planifions aussi du support pour l'UEFI. Si vous aimeriez nous aider avec cela, consultez l'[_issue_ sur GitHub](https://github.com/phil-opp/blog_os/issues/349).

### Amorçage BIOS
Presque tous les systèmes x86 peuvent amorcer le BIOS, y compris les nouvelles machines UEFI qui utilisent un BIOS émulé. C'est une bonne chose car cela permet d'utiliser la même logique d'amorçage sur toutes les machines du dernier siècle. Cependant, cette grande compatibilité est aussi le plus grand inconvénient de l'amorçage BIOS, car cela signifie que le CPU est mis dans un mode de compatibilité 16-bit appelé _[real mode]_ avant l'amorçage afin que les bootloaders archaïques des années 1980 puissent encore fonctionner. 

Mais commençons par le commencement :

Quand vous allumez votre ordinateur, il charge le BIOS provenant d'un emplacement de mémoire flash spéciale localisée sur la carte mère. Le BIOS exécute des tests d'auto-diagnostic et des routines d'initialisation du matériel, puis il cherche des disques amorçables. S'il en trouve un, le contrôle est transféré à son _bootloader_, qui est une portion de 512 octets de code exécutable enregistré au début du disque. Vu que la plupart des bootloaders dépassent 512 octets, ils sont généralement divisés en deux phases: la première, plus petite, tient dans ces 512 octets, tandis que la seconde phase est chargée subséquemment.

Le bootloader doit déterminer l'emplacement de l'image de noyau sur le disque et la charger en mémoire. Il doit aussi passer le processeur de 16-bit ([real mode]) à 32-bit ([protected mode]), puis à 64-bit ([long mode]), dans lequel les registres 64-bit et la totalité de la mémoire principale sont disponibles. Sa troisième responsabilité est de récupérer certaines informations (telle que les associations mémoires) du BIOS et de les transférer au noyau du système d'exploitation. 

[real mode]: https://fr.wikipedia.org/wiki/Mode_r%C3%A9el
[protected mode]: https://fr.wikipedia.org/wiki/Mode_prot%C3%A9g%C3%A9
[long mode]: https://en.wikipedia.org/wiki/Long_mode
[memory segmentation]: https://fr.wikipedia.org/wiki/Segmentation_(informatique)

Implémenter un bootloader est fastidieux car cela requiert l'écriture en language assembleur ainsi que plusieurs autres étapes particulières comme “écrire une valeur magique dans un registre du processeur". Par conséquent, nous ne couvrons pas la création d'un bootloader dans cet article et fournissons plutôt un outil appelé [bootimage] qui ajoute automatiquement un bootloader au noyau.

[bootimage]: https://github.com/rust-osdev/bootimage

Si vous êtes intéressé par la création de votre propre booloader : restez dans le coin, plusieurs articles sur ce sujet sont déjà prévus à ce sujet! <!-- , jetez un coup d'oeil à nos articles “_[Writing a Bootloader]_”, où nous expliquons en détails comment écrire un bootloader. -->

#### Le standard Multiboot
Pour éviter que chaque système d'exploitation implémente son propre bootloader, qui est seulement compatible avec un seul système d'exploitation, la [Free Software Foundation] a créé en 1995 un bootloader standard public appelé [Multiboot]. Le standard définit une interface entre le bootloader et le système d'exploitation afin que n'importe quel bootloader compatible Multiboot puisse charger n'importe quel système d'exploitation compatible Multiboot. L'implémentation de référence est [GNU GRUB], qui est le bootloader le plus populaire pour les systèmes Linux. 

[Free Software Foundation]: https://fr.wikipedia.org/wiki/Free_Software_Foundation
[Multiboot]: https://wiki.osdev.org/Multiboot
[GNU GRUB]: https://fr.wikipedia.org/wiki/GNU_GRUB

Pour créer un noyau compatible Multiboot, il suffit d'insérer une [en-tête Multiboot][Multiboot header] au début du fichier du noyau. Cela rend très simple l'amorçage d'un système d'exploitation depuis GRUB. Cependant, GRUB et le standard Multiboot présentent aussi quelques problèmes : 

[Multiboot header]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format

- Ils supportent seulement le "protected mode" 32-bit. Cela signifie que vous devez encore effectuer la configuration du processeur pour passer au "long mode" 64-bit.
- Ils sont conçus pour simplifier le bootloader plutôt que le noyau. Par exemple, le noyau doit être lié avec une [taille de page prédéfinie][adjusted default page size], étant donné que GRUB ne peut pas trouver les entêtes Multiboot autrement. Un autre exemple est que l'[information de boot][boot information], qui est fournies au noyau, contient plusieurs structures spécifiques à l'architecture au lieu de fournir des abstractions pures. 
- GRUB et le standard Multiboot sont peu documentés.
- GRUB doit être installé sur un système hôte pour créer une image de disque amorçable depuis le fichier du noyau. Cela rend le développement sur Windows ou sur Mac plus difficile.

[adjusted default page size]: https://wiki.osdev.org/Multiboot#Multiboot_2
[boot information]: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format

En raison de ces désavantages, nous avons décidé de ne pas utiliser GRUB ou le standard Multiboot. Cependant, nous avons l'intention d'ajouter le support Multiboot à notre outil [bootimage], afin qu'il soit aussi possible de charger le noyau sur un système GRUB. Si vous êtes interessé par l'écriture d'un noyau Multiboot conforme, consultez la [première édition][first edition] de cette série d'articles. 

[first edition]: @/edition-1/_index.md

### UEFI

(Nous ne fournissons pas le support UEFI à l'heure actuelle, mais nous aimerions bien! Si vous voulez aider, dites-le nous dans cette [_issue_ GitHub](https://github.com/phil-opp/blog_os/issues/349).)

## Un noyau minimal
Maintenant que nous savons à peu près comment un ordinateur démarre, il est temps de créer notre propre noyau minimal. Notre objectif est de créer une image de disque qui affiche “Hello World!” à l'écran lorsqu'il démarre. Nous ferons ceci en améliorant le [binaire Rust autonome][freestanding Rust binary] du dernier article.

Comme vous vous en rappelez peut-être, nous avons créé un binaire autonome grâce à `cargo`, mais selon le système d'exploitation, nous avions besoin de différents points d'entrée et d'options de compilation. C'est dû au fait que `cargo` construit pour _système hôte_ par défaut, c'est-à-dire le système que vous utilisez. Ce n'est pas ce que nous voulons pour notre noyau, car un noyau qui s'exécute, par exemple, sur Windows n'a pas de sens. Nous voulons plutôt compiler pour un _système cible_ bien défini.

### Installer une version nocturne de Rust
Rust a trois canaux de distribution : _stable_, _beta_, et _nightly_. Le Livre de Rust explique bien les différences entre ces canaux, alors prenez une minute et [jetez y un coup d'oeil](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html#choo-choo-release-channels-and-riding-the-trains). Pour construire un système d'exploitation, nous aurons besoin de fonctionalités expérimentales qui sont disponibles uniquement sur le canal de distribution nocturne. Donc nous devons installer une version nocturne de Rust.

Pour gérer l'installation de Rust, je recommande fortement [rustup]. Il vous permet d'installer les versions nocturne, beta et stable du compilateur côte-à-côte et facilite leurs mises à jour. Avec rustup, vous pouvez utiliser un canal de distribution nocturne pour le dossier actuel en exécutant `rustup override set nightly`. Par ailleurs, vous pouvez ajouter un fichier appelé `rust-toolchain` avec le contenu `nightly` au dossier racine du projet. Vous pouvez vérifier que vous avez une version nocturne installée en exécutant `rustc --version`: Le numéro de la version devrait comprendre `-nightly` à la fin.

[rustup]: https://www.rustup.rs/

La version nocturne du compilateur nous permet d'activer certaines fonctionnalités expérimentales en utilisant certains _drapeaux de fonctionalité_ dans le haut de notre fichier. Par exemple, nous pourrions activer [macro expérimentale `asm!`][`asm!` macro] pour écrire du code assembleur intégré en ajoutant `#![feature(asm)]` au haut de notre `main.rs`. Notez que ces fonctionnalités expérimentales sont tout à fait instables, ce qui veut dire que des versions futures de Rust pourraient les changer ou les retirer sans préavis. Pour cette raison, nous les utiliserons seulement lorsque strictement nécessaire.

[`asm!` macro]: https://doc.rust-lang.org/stable/reference/inline-assembly.html

### Spécification de la cible
Cargo supporte différent systèmes cibles avec le paramètre `--target`. La cible est définie par un soi-disant _[triplet de cible][target triple]_, qui décrit l'architecteur du processeur, le fabricant, le système d'exploitation, et l'interface binaire d'application ([ABI]). Par exemple, le triplet `x86_64-unknown-linux-gnu` décrit un système avec un processeur `x86_64`, sans fabricant défini, et un système d'exploitation Linux avec l'interface binaire d'application GNU. Rust supporte [plusieurs différents triplets de cible][platform-support], incluant `arm-linux-androideabi` pour Android ou [`wasm32-unknown-unknown` pour WebAssembly](https://www.hellorust.com/setup/wasm-target/).

[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[ABI]: https://stackoverflow.com/a/2456882
[platform-support]: https://forge.rust-lang.org/release/platform-support.html
[custom-targets]: https://doc.rust-lang.org/nightly/rustc/targets/custom.html

Pour notre système cible toutefois, nous avons besoin de paramètres de configuration spéciaux (par exemple, pas de système d'explotation sous-jacent), donc aucun des [triplets de cible existants][platform-support] ne convient. Heureusement, Rust nous permet de définir [notre propre cible][custom-targets] par l'entremise d'un fichier JSON. Par exemple, un fichier JSON qui décrit une cible `x86_64-unknown-linux-gnu` ressemble à ceci:

```json
{
    "llvm-target": "x86_64-unknown-linux-gnu",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
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

La plupart des champs sont requis par LLVM pour générer le code pour cette plateforme. Par exemple, le champ [`data-layout`] définit la taille de divers types d'entiers, de nombres à virgule flottante, et de pointeurs. Puis, il y a des champs que Rust utilise pour de la compilation conditionelle, comme `target-pointer-width`. Le troisième type de champ définit comment la crate doit être construite. Par exemple, le champ `pre-link-args` spécifie les arguments fournis au [lieur][linker].

[`data-layout`]: https://llvm.org/docs/LangRef.html#data-layout
[linker]: https://en.wikipedia.org/wiki/Linker_(computing)

Nous pouvons aussi cibler les systèmes `x86_64` avec notre noyau, donc notre spécification de cible ressemblera beaucoup à celle plus haut. Commençons par créer un fichier `x86_64-blog_os.json` (utilisez le nom de votre choix) avec ce contenu commun:

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executables": true
}
```

Notez que nous avons changé le système d'exploitation dans le champs `llvm-target` et `os` en `none`, puisque nous ferons l'exécution sur du "bare metal" (donc, sans système d'exploitation sous-jacent).

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

Ce paramètre spécifie que la cible ne permet pas le [déroulement de la pile][stack unwinding] lorsque le noyau panique, alors le système devrait plutôt s'arrêter directement. Ceci mène au même résultat que l'option `panic = "abort"` dans notre Cargo.toml, alors nous pouvons la retirer de ce fichier. (Notez que, contrairement à l'option Cargo.toml, cette option de cible s'applique aussi quand nous recompilerons la bibliothèque `core` plus loin dans cet article. Ainsi, même si vous préférez garder l'option Cargo.toml, gardez cette option.)

[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php

```json
"disable-redzone": true,
```

Nous écrivons un noyau, donc nous devrons éventuellement gérer les interruptions. Pour ce faire en toute sécurité, nous devons désactiver une optimisation de pointeur de pile nommée la _“zone rouge"_, puisqu'elle causerait une corruption de la pile autrement. Pour plus d'informations, lire notre article séparé à propos de la [désactivation de la zone rouge][disabling the red zone].

[disabling the red zone]: @/edition-2/posts/02-minimal-rust-kernel/disable-red-zone/index.md

```json
"features": "-mmx,-sse,+soft-float",
```

Le champ `features` active/désactive des fonctionalités de la cible. Nous désactivons les fonctionalités `mmx` et `sse` en les précédant d'un signe "moins" et activons la fonctionnalité `soft-float` en la précédant d'un signe "plus". Notez qu'il ne doit pas y avoir d'espace entre les différentes fonctionnalités, sinon LLVM n'arrive pas à analyser la chaîne de caractères des fonctionnalités.

Les fonctionnalités `mmx` et `sse` déterminent le support les instructions [Single Instruction Multiple Data (SIMD)], qui peuvent souvent significativement accélérer les programmes. Toutefois, utiliser les grands registres SIMD dans les noyaux des systèmes d'exploitation mène à des problèmes de performance. Ceci parce que le noyau a besoin de restaurer tous les registres à leur état original avant de continuer un programme interrompu. Cela signifie que le noyau doit enregistrer l'état SIMD complet dans la mémoire principale à chaque appel système ou interruption matérielle. Puisque l'état SIMD est très grand (512–1600 octets) et que les interruptions peuvent survenir très fréquemment, ces opérations d'enregistrement/restauration additionnelles nuisent considérablement à la performance. Pour prévenir cela, nous désactivons SIMD pour notre noyau (pas pour les applications qui s'exécutent dessus!).

[Single Instruction Multiple Data (SIMD)]: https://fr.wikipedia.org/wiki/Single_instruction_multiple_data

Un problème avec la désactivation de SIMD est que les opérations sur les nombres à virgule flottante sur `x86_64` nécessitent les registres SIMD par défaut. Pour résoudre ce problème, nous ajoutons la fonctionnalité `soft-float`, qui émule toutes les opérations à virgule flottante avec des fonctions logicielles utilisant des entiers normaux.

Pour plus d'informations, voir notre article sur la [désactivation de SIMD](@/edition-2/posts/02-minimal-rust-kernel/disable-simd/index.md).

#### Assembler le tout
Notre fichier de spécification de cible ressemble maintenant à ceci :

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
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

### Construction de notre noyau
Compiler pour notre nouvelle cible utilisera les conventions Linux (je ne suis pas trop certain pourquoi; j'assume que c'est simplement le comportement par défaut de LLVM). Cela signifie que nos avons besoin d'un point d'entrée nommé `_start` comme décrit dans [l'article précédent][previous post]:

[previous post]: @/edition-2/posts/01-freestanding-rust-binary/index.fr.md

```rust
// src/main.rs

#![no_std] // ne pas lier la bibliothèque standard Rust
#![no_main] // désactiver tous les points d'entrée Rust

use core::panic::PanicInfo;

/// Cette fonction est invoquée lorsque le système panique
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle] // ne pas massacrer le nom de cette fonction
pub extern "C" fn _start() -> ! {
    // cette fonction est le point d'entrée, puisque le lieur cherche une fonction
    // nommée `_start` par défaut
    loop {}
}
```

Notez que le point d'entrée doit être appelé `_start` indépendamment du système d'exploitation hôte.

Nous pouvons maintenant construire le noyau pour notre nouvelle cible en fournissant le nom du fichier JSON comme `--target`:

```
> cargo build --target x86_64-blog_os.json

error[E0463]: can't find crate for `core`
```

Cela échoue! L'erreur nous dit que le compilateur ne trouve plus la [bibliothèque `core`][`core` library]. Cette bibliothèque contient les types de base Rust comme `Result`, `Option`, les itérateurs, et est implicitement liée à toutes les crates `no_std`.

[`core` library]: https://doc.rust-lang.org/nightly/core/index.html

Le problème est que la bibliothèque `core` est distribuée avec le compilateur Rust comme biliothèque _precompilée_. Donc, elle est seulement valide pour les triplets d'hôtes supportés (par exemple, `x86_64-unknown-linux-gnu`) mais pas pour notre cible personnalisée. Si nous voulons compiler du code pour d'autres cibles, nous devons d'abord recompiler `core` pour ces cibles.

#### L'option `build-std`

C'est ici que la [fonctionnalité `build-std`][`build-std` feature] de cargo entre en jeu. Elle permet de recompiler `core` et d'autres crates de la bibliothèque standard sur demande, plutôt que d'utiliser des versions précompilées incluses avec l'installation de Rust. Cette fonctionnalité est très récente et n'est pas encore complète, donc elle est définie comme instable et est seulement disponible avec les [versions nocturnes du compilateur Rust][nightly Rust compilers].

[`build-std` feature]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std
[nightly Rust compilers]: #installer-une-version-nocturne-de-rust

Pour utiliser cette fonctionnalité, nous devons créer un fichier de [configuration cargo][cargo configuration] dans `.cargo/config.toml` avec le contenu suivant:

```toml
# dans .cargo/config.toml

[unstable]
build-std = ["core", "compiler_builtins"]
```

Ceci indique à cargo qu'il doit recompiler les bibliothèques `core` et `compiler_builtins`. Celle-ci est nécessaire pour qu'elle ait une dépendance de `core`. Afin de recompiler ces bibliothèques, cargo doit avoir accès au code source de Rust, que nous pouvons installer avec `rustup component add rust-src`.

<div class="note">

**Note:** La clé de configuration `unstable.build-std` nécessite une version nocturne de Rust plus récente que 2020-07-15.

</div>

Après avoir défini la clé de configuration `unstable.build-std` et installé la composante `rust-src`, nous pouvons exécuter notre commande de construction à nouveau:

```
> cargo build --target x86_64-blog_os.json
   Compiling core v0.0.0 (/…/rust/src/libcore)
   Compiling rustc-std-workspace-core v1.99.0 (/…/rust/src/tools/rustc-std-workspace-core)
   Compiling compiler_builtins v0.1.32
   Compiling blog_os v0.1.0 (/…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

Nous voyons que `cargo build` recompile maintenant les bibliothèques `core`, `rustc-std-workspace-core` (une dépendance de `compiler_builtins`), et `compiler_builtins` pour notre cible personnalisée.

#### Détails reliés à la mémoire

Le compilateur Rust assume qu'un certain ensemble de fonctions intégrées sont disponibles pour tous les systèmes. La plupart de ces fonctions sont fournies par la crate `compiler_builtins` que nous venons de recompiler. Toutefois, certaines fonctions liées à la mémoire dans cette crate ne sont pas activées par défaut puisqu'elles sont normalement fournies par la bibliothèque C sur le système. Parmi ces fonctions, on retrouve `memset`, qui définit tous les octets dans un bloc mémoire à une certaine valeur, `memcpy`, qui copie un bloc mémoire vers un autre, et `memcmp`, qui compare deux blocs mémoire. Alors que nous n'avions pas besoin de ces fonctions pour compiler notre noyau maintenant, elles seront nécessaires aussitôt que nous lui ajouterons plus de code (par exemple, lorsque nous copierons des `struct`).

Puisque nous ne pouvons pas lier avec la bibliothèque C du système d'exploitation, nous avons besoin d'une méthode alternative de fournir ces fonctions au compilateur. Une approche possible pour ce faire serait d'implémenter nos propre fonctions `memset`, etc. et de leur appliquer l'attribut `#[no_mangle]` (pour prévenir le changement de nom automatique pendant la compilation). Or, ceci est dangereux puisque toute erreur dans l'implémentation pourrait mener à un comportement indéterminé. Par exemple, implémenter `memcpy` avec une boucle `for` pourrait mener à une recursion infinie puisque les boucles `for` invoquent implicitement la méthode _trait_ [`IntoIterator::into_iter`], qui pourrait invoquer `memcpy` de nouveau. C'est donc une bonne idée de plutôt réutiliser des implémentations existantes et éprouvées.

[`IntoIterator::into_iter`]: https://doc.rust-lang.org/stable/core/iter/trait.IntoIterator.html#tymethod.into_iter

Heureusement, la crate `compiler_builtins` contient déjà des implémentations pour toutes les fonctions nécessaires, elles sont seulement désactivées par défaut pour ne pas interférer avec les implémentations de la bibliothèque C. Nous pouvons les activer en définissant le drapeau [`build-std-features`] de cargo à `["compiler-builtins-mem"]`. Comme pour le drapeau `build-std`, ce drapeau peut être soit fourni en ligne de commande avec `-Z` ou configuré dans la table `unstable` du fichier `.cargo/config.toml`. Puisque nous voulons toujours construire avec ce drapeau, l'option du fichier de configuration fait plus de sens pour nous:

[`build-std-features`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features

```toml
# dans .cargo/config.toml

[unstable]
build-std-features = ["compiler-builtins-mem"]
build-std = ["core", "compiler_builtins"]
```

(Le support pour la fonctionnalité `compiler-builtins-mem` a [été ajouté assez récemment](https://github.com/rust-lang/rust/pull/77284), donc vous aurez besoin de la version nocturne `2020-09-30` de Rust ou plus récent pour l'utiliser.)

Dans les coulisses, ce drapeau active la [fonctionnalité `mem`][`mem` feature] de la crate `compiler_builtins`. Le résultat est que l'attribut `#[no_mangle]` est appliqué aux [implémentations `memcpy` et autres][`memcpy` etc. implementations] de la caise, ce qui les rend disponible au lieur.

[`mem` feature]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/Cargo.toml#L54-L55
[`memcpy` etc. implementations]: https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/src/mem.rs#L12-L69

Avec ce changement, notre noyau a des implémentations valides pour toutes les fonctions requises par le compilateur, donc il peut continuer à compiler même si notre code devient plus complexe.

#### Définir une cible par défaut

Pour ne pas avoir à fournir le paramètre `--target` à chaque invocation de `cargo build`, nous pouvons définir la cible par défaut. Pour ce faire, nous ajoutons le code suivant à notre fichier de [configuration Cargo][cargo configuration] dans `.cargo/config.toml`:

[cargo configuration]: https://doc.rust-lang.org/cargo/reference/config.html

```toml
# dans .cargo/config.toml

[build]
target = "x86_64-blog_os.json"
```

Ceci indique à `cargo` d'utiliser notre cible `x86_64-blog_os.json` quand il n'y a pas d'argument de cible `--target` explicitement fourni. Ceci veut dire que nous pouvons maintenant construire notre noyau avec un simple `cargo build`. Pour plus d'informations sur les options de configuration cargo, jetez un coup d'oeil à la [documentation officielle de cargo][cargo configuration].

Nous pouvons maintenant construire notre noyau pour une cible "bare metal" avec un simple `cargo build`. Toutefois, notre point d'entrée `_start`, qui sera appelé par le bootloader, est encore vide. Il est temps de lui faire afficher quelque chose à l'écran.

### Afficher à l'écran
La façon la plus facile d'afficher à l'écran à ce stade est grâce au tampon texte VGA. C'est un emplacement mémoire spécial associé au matériel VGA qui contient le contenu affiché à l'écran. Il consiste normalement en 25 lines qui contiennent chacune 80 cellules de caractère. Chaque cellule de caractère affiche un caractère ASCII avec des couleurs d'avant-plan et d'arrière-plan. Le résultat à l'écran ressemble à ceci:

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

D'abord, nous transformons l'entier `0xb8000` en un [pointeur brut][raw pointer]. Puis nous [parcourons][iterate] les octets de la [chaîne d'octets][byte string] [statique][static] `HELLO`. Nous utilisons la méthode [`enumerate`] pour aussi obtenir une variable `i`. Dans le corps de la boucle `for`, nous utilisons la méthode [`offset`] pour écrire la chaîne d'octets et l'octet de couleur correspondant(`0xb` est un cyan pâle).

[iterate]: https://doc.rust-lang.org/stable/book/ch13-02-iterators.html
[static]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate
[byte string]: https://doc.rust-lang.org/reference/tokens.html#byte-string-literals
[raw pointer]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

Notez qu'il y a un bloc [`unsafe`] qui enveloppe les écritures mémoire. La raison en est que le compilateur Rust ne peut pas prouver que les pointeurs bruts que nous créons sont valides. Ils pourraient pointer n'importe où et mener à une corruption de données. En les mettant dans un bloc `unsafe`, nous disons fondamentalement au compilateur que nous sommes absolument certains que les opérations sont valides. Notez qu'un bloc `unsafe` ne désactive pas les contrôles de sécurité de Rust. Il permet seulement de faire [cinq choses supplémentaires][five additional things].

[`unsafe`]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html
[five additional things]: https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html#unsafe-superpowers

Je veux souligner que **ce n'est pas comme cela que les choses se font en Rust!** Il est très facile de faire des erreurs en travaillant avec des pointeurs bruts à l'intérieur de blocs `unsafe`. Par exemple, nous pourrions facilement écrire au-delà de la fin du tampon si nous ne sommes pas prudents.

Alors nous voulons minimiser l'utilisation de `unsafe` autant que possible. Rust nous offre la possibilité de le faire en créant des abstractions de sécurité. Par exemple, nous pourrions créer un type tampon VGA qui encapsule les risques et qui s'assure qu'il est impossible de faire quoi que ce soit d'incorrect à l'extérieur de ce type. Ainsi, nous aurions besoin de très peu de code `unsafe` et nous serions certains que nous ne violons pas la [sécurité de mémoire][memory safety]. Nous allons créer une telle abstraction de tampon VGA buffer dans le prochain article.

[memory safety]: https://en.wikipedia.org/wiki/Memory_safety

## Exécuter notre noyau

Maintenant que nous avons un exécutable qui fait quelque chose de perceptible, il est temps de l'exécuter. D'abord, nous devons transformer notre noyau compilé en une image de disque amorçable en le liant à un bootloader. Ensuite, nous pourrons exécuter l'image de disque dans une machine virtuelle [QEMU] ou l'amorcer sur du véritable matériel en utilisant une clé USB.

### Créer une image d'amorçage

Pour transformer notre noyau compilé en image de disque amorçable, nous devons le lier avec un bootloader. Comme nous l'avons appris dans la [section à propos du lancement][section about booting], le bootloader est responsable de l'initialisation du processeur et du chargement de notre noyau.

[section about booting]: #le-processus-d-amorcage

Plutôt que d'écrire notre propre bootloader, ce qui est un projet en soi, nous utilisons la crate [`bootloader`]. Cette crate propose un bootloader BIOS de base sans dépendance C. Seulement du code Rust et de l'assembleur intégré. Pour l'utiliser afin de lancer notre noyau, nous devons ajouter une dépendance à cette crate:

[`bootloader`]: https://crates.io/crates/bootloader

```toml
# dans Cargo.toml

[dependencies]
bootloader = "0.9.8"
```

Ajouter le bootloader comme dépendance n'est pas suffisant pour réellement créer une image de disque amorçable. Le problème est que nous devons lier notre noyau avec le bootloader après la compilation, mais cargo ne supporte pas les [scripts post-build][post-build scripts].

[post-build scripts]: https://github.com/rust-lang/cargo/issues/545

Pour résoudre ce problème, nous avons créé un outil nommé `bootimage` qui compile d'abord le noyau et le bootloader, et les lie ensuite ensemble pour créer une image de disque amorçable. Pour installer cet outil, exécutez la commande suivante dans votre terminal:

```
cargo install bootimage
```

Pour exécuter `bootimage` et construire le bootloader, vous devez avoir la composante rustup `llvm-tools-preview` installée. Vous pouvez l'installer en exécutant `rustup component add llvm-tools-preview`.

Après avoir installé `bootimage` et ajouté la composante `llvm-tools-preview`, nous pouvons créer une image de disque amorçable en exécutant:

```
> cargo bootimage
```

Nous voyons que l'outil recompile notre noyau en utilisant `cargo build`, donc il utilisera automatiquement tout changements que vous faites. Ensuite, il compile le bootloader, ce qui peut prendre un certain temps. Comme toutes les dépendances de crates, il est seulement construit une fois puis il est mis en cache, donc les builds subséquentes seront beaucoup plus rapides. Enfin, `bootimage` combine le bootloader et le noyau en une image de disque amorçable.

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

Après l'écriture de l'image sur votre clé USB, vous pouvez l'exécuter sur du véritable matériel en l'amorçant à partir de la clé USB. Vous devrez probablement utiliser un menu d'amorçage spécial ou changer l'ordre d'amorçage dans votre configuration BIOS pour amorcer à partir de la clé USB. Notez que cela ne fonctionne actuellement pas avec des ordinateurs UEFI, puisque la crate `bootloader` ne supporte pas encore UEFI.

### Utilisation de `cargo run`

Pour faciliter l'exécution de notre noyau dans QEMU, nous pouvons définir la clé de configuration `runner` pour cargo:

```toml
# dans .cargo/config.toml

[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```

La table `target.'cfg(target_os = "none")'` s'applique à toutes les cibles dont le champ `"os"` dans le fichier de configuration est défini à `"none"`. Ceci inclut notre cible `x86_64-blog_os.json`. La clé `runner` key spécifie la commande qui doit être invoquée pour `cargo run`. La commande est exécutée après une build réussie avec le chemin de l'exécutable comme premier argument. Voir la [configuration cargo][cargo configuration] pour plus de détails.

La commande `bootimage runner` est spécifiquement conçue pour être utilisable comme un exécutable `runner`. Elle lie l'exécutable fourni avec le bootloader duquel dépend le projet et lance ensuite QEMU. Voir le [README de `bootimage`][Readme of `bootimage`] pour plus de détails et les options de configuration possibles.

[Readme of `bootimage`]: https://github.com/rust-osdev/bootimage

Nous pouvons maintenant utiliser `cargo run` pour compiler notre noyau et le lancer dans QEMU.

## Et ensuite?

Dans le prochain article, nous explorerons le tampon texte VGA plus en détails et nous écrirons une interface sécuritaire pour l'utiliser. Nous allons aussi mettre en place la macro `println`.
