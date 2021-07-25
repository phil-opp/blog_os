+++
title = "Текстовый режим VGA"
weight = 2
path = "ru/vga-text-mode"
date = 2018-02-26

[extra]
chapter = "С нуля"
translators = ["MrZloHex"]
+++

[Текстовый режим VGA][VGA text mode] &mdash; это простой способ вывода текста на экран. В этом посте мы создадим интерфейс, который делает его использование безопасным и простым, инкапсулируя все уязвимости в отдельный модуль. Мы также реализуем поддержку [макросов форматирования][formatting macros] языка Rust.

[VGA text mode]: https://en.wikipedia.org/wiki/VGA-compatible_text_mode
[formatting macros]: https://doc.rust-lang.org/std/fmt/#related-macros

<!-- more -->

Этот блог открыто разрабатывается на [GitHub]. Если у вас возникли какие-либо проблемы или вопросы, пожалуйста, создайте _issue_. Также вы можете оставлять комментарии [в конце страницы][at the bottom]. Полный исходный код для этого поста вы можете найти в репозитории в ветке [`post-03`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-03

<!-- toc -->