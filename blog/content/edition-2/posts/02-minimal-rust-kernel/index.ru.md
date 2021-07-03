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

## Последовательность процессов запуска

Когда вы включаете компьютер, он начинает выполнять код микропрограммы, который хранится в [ПЗУ][ROM] материнской платы. Этот код выполняет [самотестирование при включении][power-on self-test], определяет доступную оперативную память и выполняет предварительную инициализацию процессора и аппаратного обеспечения. После этого он ищет загрузочный диск и начинает загрузку ядра операционной системы.

[ROM]: https://en.wikipedia.org/wiki/Read-only_memory
[power-on self-test]: https://en.wikipedia.org/wiki/Power-on_self-test

Для архитектуры x86 существует два стандарта прошивки: “Basic Input/Output System“ ("Базовая система ввода/вывода" **[BIOS]**) и более новый “Unified Extensible Firmware Interface”  ("Унифицированный расширяемый интерфейс прошивки" **[UEFI]**). Стандарт BIOS - старый и устаревший, но простой и хорошо поддерживаемый на любой машине x86 с 1980-х годов. UEFI, напротив, более современный и имеет гораздо больше возможностей, но более сложен в настройке (по крайней мере, на мой взгляд).

[BIOS]: https://en.wikipedia.org/wiki/BIOS
[UEFI]: https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface

В данный момент, мы обеспечиваем поддержку только BIOS, но планируется поддержка и UEFI. Если вы хотите помочь нам в этом, обратитесь к [Github issue](https://github.com/phil-opp/blog_os/issues/349).