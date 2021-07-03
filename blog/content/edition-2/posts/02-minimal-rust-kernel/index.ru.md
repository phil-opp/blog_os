+++
title = "Минимально возможное ядро на Rust"
weight = 2
path = "ru/minimal-rust-kernel"
date = 2018-02-10

[extra]
chapter = "С нуля"
translators = ["MrZloHex"]
+++

В этом посте мы создадим минимальное 64-битное ядро на Rust для архитектуры x86_64. Мы будем отталкиваться от [независимого бинарного файла][freestanding Rust binary] из предыдущего поста для создания загрузочного образа диска, который может что-то выводить на экран.

[freestanding Rust binary]: @/edition-2/posts/01-freestanding-rust-binary/index.ru.md

<!-- more -->
Этот блог открыто разрабатывается на [GitHub]. Если у вас есть несколько проблем или вопросов, пожалуйста откройте _issue_. Также можете оставлять комментарии [в конце файла][at the bottom]. Полный исходный код для этого поста вы можете найти [`post-02`][post branch] ветке репозитория.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-02

<!-- toc -->