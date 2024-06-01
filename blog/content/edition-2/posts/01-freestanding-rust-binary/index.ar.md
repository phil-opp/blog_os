+++
title = "A Freestanding Rust Binary"
weight = 1
path = "freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
+++

تتمثل الخطوة الأولى في إنشاء نواة نظام التشغيل الخاصة بنا في إنشاء ملف Rust قابل للتنفيذ لا يربط المكتبة القياسية. هذا يجعل من الممكن تشغيل شيفرة Rust على [bare metal] دون نظام تشغيل أساسي.
[bare metal]: https://en.wikipedia.org/wiki/Bare_machine
<!-- more -->

تم تطوير هذه المدونة بشكل مفتوح على [GitHub]. إذا كان لديك أي مشاكل أو أسئلة، يرجى فتح مشكلة هناك. يمكنك أيضًا ترك تعليقات [في الأسفل]. يمكن العثور على الشيفرة المصدرية الكاملة لهذا المنشور في فرع [post-01].


[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01
<!-- toc -->


## مقدمة
لكتابة نواة نظام تشغيل، نحتاج إلى شيفرة لا تعتمد على أي ميزات نظام تشغيل. هذا يعني أنه لا يمكننا استخدام سلاسل الرسائل(threads) أو الملفات(File System) أو Heap ram أو الشبكة أو الأرقام العشوائية أو الإخراج القياسي(I/O) أو أي ميزات أخرى تتطلب تجريدات نظام التشغيل أو أجهزة معينة. وهذا منطقي، لأننا نحاول كتابة نظام التشغيل الخاص بنا (OS) وبرامج التشغيل الخاصة بنا (drivers).

هذا يعني أنه لا يمكننا استخدام معظم [Rust standard library]، ولكن هناك الكثير من ميزات Rust التي _يمكننا استخدامها. على سبيل المثال، يمكننا استخدام [iterators] و [closures] و [pattern matching] و [option] و [اresult] و [string formatting] وبالطبع [ownership system]. هذه الميزات تجعل من الممكن كتابة نواة بطريقة معبرة جدًا وعالية المستوى دون القلق بشأن [undefined behavior] أو [memory safety].


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


من أجل إنشاء نواة نظام تشغيل في Rust، نحتاج إلى إنشاء ملف قابل للتنفيذ يمكن تشغيله بدون نظام تشغيل أساسي. غالبًا ما يُطلق على هذا الملف القابل للتنفيذ اسم الملف القابل للتنفيذ ”القائم بذاته“ أو ”المعدني العاري“.

يصف هذا المنشور الخطوات اللازمة لإنشاء ثنائي Rust قائم بذاته ويشرح سبب الحاجة إلى هذه الخطوات. إذا كنت مهتمًا بمثال بسيط فقط، يمكنك **[الانتقال إلى الملخص] (#ملخص)**.



## تعطيل المكتبة القياسية
بشكل افتراضي، تربط جميع صناديق Rust [standard library]، والتي تعتمد على نظام التشغيل لميزات (مثل threads, files, or networking). كما أنها تعتمد أيضًا على مكتبة C القياسية 'libc'، والتي تتفاعل بشكل وثيق مع خدمات نظام التشغيل. نظرًا لأن خطتنا هي كتابة نظام تشغيل، لا يمكننا استخدام أي مكتبات تعتمد على نظام التشغيل. لذا يجب علينا تعطيل التضمين التلقائي للمكتبة القياسية من خلال سمة [no_std].


[standard library]: https://doc.rust-lang.org/std/
[`no_std` attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

```
cargo new blog_os --bin --edition 2018
```

لقد أطلقتُ على المشروع اسم ”Blog_os“، ولكن بالطبع يمكنك اختيار اسمك الخاص. تُحدّد علامة ”bin“ أننا نريد إنشاء نسخة binary قابلة للتنفيذ (على عكس المكتبة) وتحدّد علامة ”--- Edition 2018“ أننا نريد استخدام [2018 edition] من Rust لصندوقنا. عندما نُشغّل الأمر، تُنشئ لنا الشحنة بنية الدليل التالية:

[2018 edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```
يحتوي ملف 'Cargo.toml' على تكوين الصندوق، على سبيل المثال اسم الصندوق، والمؤلف، ورقم [semantic version]، والتبعيات. يحتوي الملف 'src/main.rs' على الوحدة النمطية الجذرية للصندوق والدالة 'الرئيسية'. يمكنك تجميع قفصك من خلال 'cargo build' ثم تشغيل الملف الثنائي 'blog_os' المجمّع في المجلد الفرعي 'target/debug'.
[semantic version]: https://semver.org/

