+++
title = "Независимый бинанрник на Rust"
weight = 1
path = "ru/freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "С нуля"
translators = ["MrZloHex"]
+++

Первым шагом в создании собственного ядра операционной системы - это создание исполняемого файла на Rust, который не будет подключать стандартную библиотеку. Именно это дает возможность запускать Rust код на [голом металле][bare metal] без слоя операционной системы, которая связывает железо компьютера и программы.

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

Этот блог открыто разрабатывается на [GitHub]. Если у вас есть несколько проблем или вопросов, пожалуйста откройте _issue_. Также можете оставлять комментарии [в конце файла][at the bottom]. Полный исходный код для этого поста вы можете найти [`post-01`][post branch] ветке репозитория.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #commentsний ком
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## Введение
Для того, чтобы написать ядро операционной системы нужен код, который независит от операционной системы и ее свойств. Это озночает, что нельзя использовать потоки, файлы, [кучу][heap], сети, случайные числа, стандартный видео-вывод или другие возможности, которые предоставляет абстракция в виде ОС или очень специфичное железо.

[heap]: https://en.wikipedia.org/wiki/Heap_(data_structure)

Это значит, что нельзя использовать большинство [стандартных библиотек Rust][Rust Standart library], но также есть еще множество других возможностей, которые предоставляет Rust и их _можно использовать_. Как пример того, что можно использовать это: [итераторы][iterators], [замыкания][closures], [соответствия по шаблону][pattern matching], [опции][option] и[результат][result], [форматирование строк][string formatting] и, конечно же, [систему владения][ownership system]. Эти функции дают возможность для написания ядра в очень выразительном и высоко-уровневом стиле без беспокойства о [неопределенном поведении][undefined behavior] или [сохранности памяти][memory safety].

[option]: https://doc.rust-lang.org/core/option/
[result]:https://doc.rust-lang.org/core/result/
[Rust standard library]: https://doc.rust-lang.org/std/
[iterators]: https://doc.rust-lang.org/book/ch13-02-iterators.html
[closures]: https://doc.rust-lang.org/book/ch13-01-closures.html
[pattern matching]: https://doc.rust-lang.org/book/ch06-00-enums.html
[string formatting]: https://doc.rust-lang.org/core/macro.write.html
[ownership system]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
[undefined behavior]: https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs
[memory safety]: https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention

Вместо создания ядра ОС на Rust, нужно создать исполняемы файл, который мог бы запускаться без абстракции в виде ОС.

Этот пост описывает необходимые шаги для создания независимого исполняемого бинарного файла на Rust и объясняет зачем эти шаги нужны. Если вы заинтересованны тоьлько в простом примере, можете сразу перейти к __[итогам](#Итоги)__.

## Дизактивация стандартной библиотеки
По стандарту, все модули Rust ссылаются на [стандартную библиотеку][standart library], которая зависит от операционной системы для таких возможностей как потоки, файлы, сети. Также она зависит от стандартной библиотки C `libc`, которая очень тесно взаимодействует с сервиса ОС. С тех пор как план - это написание операционной системы, нельзя использовать библиотеки, которые зависят от операционной системы. Следовательно стоит отключить автоматические добавление стандартной библиотеки через [`no_std` аттрибут][attribute].

[standard library]: https://doc.rust-lang.org/std/
[attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

Мы начнем с создания нового cargo проекта. Самый простой способ сделать это, через командную строку:

```
cargo new blog_os --bin -- edition 2018
```

Я назвал этот проект `blog_os`, но вы можете назвать как вам угодно. Флаг `--bin` указывает на то, что мы хоти создать исполняемый бинарный файл (в сравнении с библиотекой) и флаг `--edition 2018` указывает, что мы хотим использовать [версию 2018][edition] Rust для нашего модуля. После выполнения комманды, cargo создаст каталог со следующей стркутурой:

[edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

`Cargo.toml` содержит данные и конфигурацию модуля, такие как _название, автор, версию_ и _зависимости_ от других модулей и библиотек. Файл `src/main.rs` содержит корневой файл нашего модуля и главную `main` функцию. Можно скомпилировать модуль с помощью `cargo build` и после запустить скомпилированный `blog_os` бинарный файл в `target/debub` каталоге.