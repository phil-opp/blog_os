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