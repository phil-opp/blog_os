+++
title = "ثنائي Rust مستقل"
weight = 1
path = "ar/freestanding-rust-binary"
date = 2018-02-10

[extra]
# Please update this when updating the translation
translation_based_on_commit = "087a464ed77361cff6c459fb42fc655cb9eacbea"
# GitHub usernames of the people that translated this post
translators = ["ZAAFHachemrachid", "mindfreq"]
rtl = true
+++

تتمثل الخطوة الأولى في إنشاء نواة نظام التشغيل الخاصة بنا في إنشاء ملف Rust قابل للتنفيذ لا يربط المكتبة القياسية. هذا يجعل من الممكن تشغيل شيفرة Rust على [bare metal] دون نظام تشغيل أساسي.

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

تم تطوير هذه المدونة بشكل مفتوح على [GitHub]. إذا كان لديك أي مشاكل أو أسئلة، يرجى فتح مشكلة هناك. يمكنك أيضًا ترك تعليقات [في الأسفل]. يمكن العثور على الشيفرة المصدرية الكاملة لهذا المنشور في فرع [`post-01`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[في الأسفل]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## مقدمة

لكتابة نواة نظام تشغيل، نحتاج إلى شيفرة لا تعتمد على أي ميزات نظام تشغيل. هذا يعني أنه لا يمكننا استخدام الخيوط (threads) أو الملفات أو ذاكرة الكومة (heap) أو الشبكة أو الأرقام العشوائية أو الإخراج القياسي، أو أي ميزات أخرى تتطلب تجريدات نظام التشغيل أو أجهزة معينة. وهذا منطقي، لأننا نحاول كتابة نظام التشغيل الخاص بنا وبرامج التشغيل الخاصة بنا.

هذا يعني أنه لا يمكننا استخدام معظم [مكتبة Rust القياسية][Rust standard library]، ولكن هناك الكثير من ميزات Rust التي _يمكننا_ استخدامها. على سبيل المثال، يمكننا استخدام [المكررات][iterators] و[الإغلاقات][closures] و[مطابقة الأنماط][pattern matching] و[Option] و[Result] و[تنسيق السلاسل][string formatting] وبالطبع [نظام الملكية][ownership system]. هذه الميزات تجعل من الممكن كتابة نواة بطريقة معبرة وعالية المستوى دون القلق بشأن [السلوك غير المحدد][undefined behavior] أو [سلامة الذاكرة][memory safety].

[option]: https://doc.rust-lang.org/core/option/
[result]: https://doc.rust-lang.org/core/result/
[Rust standard library]: https://doc.rust-lang.org/std/
[iterators]: https://doc.rust-lang.org/book/ch13-02-iterators.html
[closures]: https://doc.rust-lang.org/book/ch13-01-closures.html
[pattern matching]: https://doc.rust-lang.org/book/ch06-00-enums.html
[string formatting]: https://doc.rust-lang.org/core/macro.write.html
[ownership system]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
[undefined behavior]: https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs
[memory safety]: https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention

من أجل إنشاء نواة نظام تشغيل في Rust، نحتاج إلى إنشاء ملف قابل للتنفيذ يمكن تشغيله بدون نظام تشغيل أساسي. غالبًا ما يُطلق على هذا الملف القابل للتنفيذ اسم الملف "المستقل" أو "bare-metal".

يصف هذا المنشور الخطوات اللازمة لإنشاء ثنائي Rust مستقل ويشرح سبب الحاجة إلى هذه الخطوات. إذا كنت مهتمًا بمثال بسيط فقط، يمكنك **[الانتقال إلى الملخص](#summary)**.

## تعطيل المكتبة القياسية

بشكل افتراضي، تربط جميع صناديق Rust [المكتبة القياسية][standard library]، والتي تعتمد على نظام التشغيل لميزات مثل الخيوط والملفات والشبكة. كما أنها تعتمد أيضًا على مكتبة C القياسية `libc`، والتي تتفاعل بشكل وثيق مع خدمات نظام التشغيل. نظرًا لأن خطتنا هي كتابة نظام تشغيل، لا يمكننا استخدام أي مكتبات تعتمد على نظام التشغيل. لذا يجب علينا تعطيل التضمين التلقائي للمكتبة القياسية من خلال [سمة `no_std`][`no_std` attribute].

[standard library]: https://doc.rust-lang.org/std/
[`no_std` attribute]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html

نبدأ بإنشاء مشروع تطبيق cargo جديد. أسهل طريقة للقيام بذلك هي عبر سطر الأوامر:

```
cargo new blog_os --bin --edition 2024
```

لقد أطلقت على المشروع اسم `blog_os`، ولكن بالطبع يمكنك اختيار اسمك الخاص. تُحدد علامة `--bin` أننا نريد إنشاء ملف ثنائي قابل للتنفيذ (في مقابل المكتبة)، وتحدد علامة `--edition 2024` أننا نريد استخدام [إصدار 2024][2024 edition] من Rust لصندوقنا. عندما نشغّل الأمر، ينشئ cargo بنية الدليل التالية:

[2024 edition]: https://doc.rust-lang.org/nightly/edition-guide/rust-2024/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

يحتوي `Cargo.toml` على تكوين الصندوق، مثل اسم الصندوق والمؤلف ورقم [الإصدار الدلالي][semantic version] والتبعيات. يحتوي ملف `src/main.rs` على الوحدة الجذرية للصندوق ودالة `main`. يمكنك تجميع صندوقك عبر `cargo build` ثم تشغيل الملف الثنائي `blog_os` المجمَّع في المجلد الفرعي `target/debug`.

[semantic version]: https://semver.org/

### سمة `no_std`

يربط صندوقنا الآن المكتبة القياسية ضمنيًا. دعونا نحاول تعطيل ذلك بإضافة [سمة `no_std`][`no_std` attribute]:

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

عندما نحاول بناءه الآن (عن طريق تشغيل `cargo build`)، يحدث الخطأ التالي:

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

سبب هذا الخطأ هو أن [ماكرو `println`][`println` macro] جزء من المكتبة القياسية التي لم نعد نضمّنها. لذا لم يعد بإمكاننا طباعة الأشياء. هذا منطقي، لأن `println` يكتب إلى [الإخراج القياسي][standard output]، وهو واصف ملف خاص يوفره نظام التشغيل.

[`println` macro]: https://doc.rust-lang.org/std/macro.println.html
[standard output]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

لذا دعنا نحذف عبارة الطباعة ونحاول مرة أخرى بدالة main فارغة:

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

الآن يفتقد المترجم إلى دالة `#[panic_handler]` و_عنصر اللغة_ (language item).

## تنفيذ Panic

تُعرِّف سمة `panic_handler` الدالة التي يجب على المترجم استدعاؤها عند حدوث [panic]. توفر المكتبة القياسية دالة معالج الـ panic الخاصة بها، ولكن في بيئة `no_std` نحتاج إلى تعريفها بأنفسنا:

[panic]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// in main.rs

use core::panic::PanicInfo;

/// هذه الدالة تُستدعى عند حدوث panic
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

تحتوي [معامل `PanicInfo`][PanicInfo] على الملف والسطر الذي حدث فيه الـ panic ورسالة الـ panic الاختيارية. يجب ألا تعود الدالة أبدًا، لذا يتم تمييزها كـ[دالة متباينة][diverging function] بإرجاع [النوع "never"][`!`] أي `!`. لا يمكننا فعل الكثير في هذه الدالة الآن، لذا نقوم فقط بالتكرار إلى ما لا نهاية.

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[diverging function]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[`!`]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## عنصر اللغة `eh_personality`

عناصر اللغة هي عناصر خاصة (سمات ودوال وأنواع وما إلى ذلك) مطلوبة داخليًا من قِبَل المترجم. على سبيل المثال، سمة [`Copy`] هي عنصر لغة يخبر المترجم بالأنواع التي لها [_دلالات النسخ_][`Copy`]. عند النظر إلى [التنفيذ][copy code]، نرى أنه يحتوي على السمة الخاصة `#[lang = "copy"]` التي تعرّفه كعنصر لغة.

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

في حين أن توفير تنفيذات مخصصة لعناصر اللغة ممكن، إلا أنه يجب القيام بذلك فقط كملاذ أخير. والسبب هو أن عناصر اللغة هي تفاصيل تنفيذ غير مستقرة للغاية ولا يتم التحقق من أنواعها حتى (أي أن المترجم لا يتحقق حتى مما إذا كانت الدالة تحتوي على أنواع الوسيطات الصحيحة). لحسن الحظ، هناك طريقة أكثر استقرارًا لإصلاح خطأ عنصر اللغة أعلاه.

يُميِّز [عنصر اللغة `eh_personality`][`eh_personality` language item] دالة تُستخدم لتنفيذ [فك تسلسل المكدس][stack unwinding]. بشكل افتراضي، يستخدم Rust الفك لتشغيل المدمِّرات لجميع متغيرات المكدس الحية في حالة حدوث [panic]. هذا يضمن تحرير جميع الذاكرة المستخدمة ويسمح للخيط الرئيسي بالتقاط الـ panic ومواصلة التنفيذ. ومع ذلك، فإن الفك عملية معقدة وتتطلب بعض المكتبات الخاصة بنظام التشغيل (مثل [libunwind] على Linux أو [معالجة الاستثناءات المنظمة][structured exception handling] على Windows)، لذا لا نريد استخدامها لنظام التشغيل الخاص بنا.

[`eh_personality` language item]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[stack unwinding]: https://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: https://www.nongnu.org/libunwind/
[structured exception handling]: https://docs.microsoft.com/en-us/windows/win32/debug/structured-exception-handling

### تعطيل الفك

هناك حالات استخدام أخرى أيضًا يكون فيها الفك غير مرغوب فيه، لذا يوفر Rust خيارًا [للإيقاف عند الـ panic][abort on panic] بدلاً من ذلك. هذا يعطّل توليد معلومات رموز الفك وبالتالي يقلل حجم الثنائي بشكل كبير. هناك أماكن متعددة يمكننا فيها تعطيل الفك. أسهل طريقة هي إضافة الأسطر التالية إلى `Cargo.toml`:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

يضبط هذا استراتيجية الـ panic على `abort` لكل من ملف التعريف `dev` (المستخدم لـ `cargo build`) وملف التعريف `release` (المستخدم لـ `cargo build --release`). الآن لم يعد عنصر اللغة `eh_personality` مطلوبًا.

[abort on panic]: https://github.com/rust-lang/rust/pull/32900

الآن أصلحنا كلا الخطأين أعلاه. ومع ذلك، إذا حاولنا تجميعه الآن، يحدث خطأ آخر:

```
> cargo build
error: requires `start` lang_item
```

يفتقر برنامجنا إلى عنصر اللغة `start`، الذي يعرّف نقطة الدخول.

## سمة `start`

قد يظن المرء أن دالة `main` هي أول دالة تُستدعى عند تشغيل برنامج. ومع ذلك، فإن معظم اللغات لديها [نظام وقت تشغيل][runtime system] مسؤول عن أشياء مثل جمع القمامة (مثلاً في Java) أو خيوط البرمجيات (مثلاً goroutines في Go). يحتاج وقت التشغيل هذا إلى الاستدعاء قبل `main`، لأنه يحتاج إلى تهيئة نفسه.

[runtime system]: https://en.wikipedia.org/wiki/Runtime_system

في ثنائي Rust نموذجي يربط المكتبة القياسية، يبدأ التنفيذ في مكتبة وقت تشغيل C تسمى `crt0` ("C runtime zero")، والتي تُهيئ البيئة لتطبيق C. يتضمن ذلك إنشاء مكدس ووضع الوسيطات في السجلات الصحيحة. ثم يستدعي وقت تشغيل C [نقطة دخول وقت تشغيل Rust][rt::lang_start]، والمميَّزة بعنصر اللغة `start`. يمتلك Rust وقت تشغيل بسيطًا للغاية يعتني ببعض الأشياء الصغيرة مثل إعداد حراس فائض المكدس أو طباعة تتبع المكدس عند الـ panic. ثم يستدعي وقت التشغيل أخيرًا دالة `main`.

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

لا يمتلك ملفنا القابل للتنفيذ المستقل وصولاً إلى وقت تشغيل Rust و`crt0`، لذا نحتاج إلى تعريف نقطة الدخول الخاصة بنا. تنفيذ عنصر اللغة `start` لن يساعد، لأنه سيتطلب `crt0` أيضًا. بدلاً من ذلك، نحتاج إلى الكتابة فوق نقطة دخول `crt0` مباشرةً.

### الكتابة فوق نقطة الدخول

لإخبار مترجم Rust بأننا لا نريد استخدام سلسلة نقاط الدخول العادية، نضيف سمة `#![no_main]`.

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// هذه الدالة تُستدعى عند حدوث panic
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

قد تلاحظ أننا حذفنا دالة `main`. والسبب هو أن `main` لا معنى لها بدون وقت تشغيل أساسي يستدعيها. بدلاً من ذلك، نقوم الآن بالكتابة فوق نقطة دخول نظام التشغيل بدالتنا `_start`:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

باستخدام سمة `#[unsafe(no_mangle)]`، نعطّل [تشويه الأسماء][name mangling] لضمان أن مترجم Rust يُخرج فعلاً دالة باسم `_start`. بدون هذه السمة، سيولّد المترجم رمزاً غامضاً مثل `_ZN3blog_os4_start7hb173fedf945531caE` لإعطاء كل دالة اسمًا فريدًا. السمة مطلوبة لأننا نحتاج إلى إخبار الرابط باسم دالة نقطة الدخول في الخطوة التالية.

كما يجب علينا تمييز الدالة كـ`extern "C"` لإخبار المترجم بأنه يجب أن يستخدم [اتفاقية استدعاء C][C calling convention] لهذه الدالة (بدلاً من اتفاقية استدعاء Rust غير المحددة). سبب تسمية الدالة `_start` هو أن هذا هو اسم نقطة الدخول الافتراضي لمعظم الأنظمة.

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[C calling convention]: https://en.wikipedia.org/wiki/Calling_convention

نوع الإرجاع `!` يعني أن الدالة متباينة، أي لا يُسمح لها بالعودة أبدًا. هذا مطلوب لأن نقطة الدخول لا تُستدعى من أي دالة، بل يتم استدعاؤها مباشرة من نظام التشغيل أو محمّل الإقلاع. لذا بدلاً من العودة، يجب أن تستدعي نقطة الدخول مثلاً [استدعاء النظام `exit`][`exit` system call] لنظام التشغيل. في حالتنا، إيقاف تشغيل الجهاز قد يكون إجراءً معقولاً، نظرًا لأنه لا يوجد شيء آخر يمكن فعله إذا عاد الثنائي المستقل. في الوقت الحالي، نستوفي هذا المتطلب بالتكرار إلى ما لا نهاية.

[`exit` system call]: https://en.wikipedia.org/wiki/Exit_(system_call)

عندما نشغّل `cargo build` الآن، نحصل على خطأ _رابط_ مزعج.

## أخطاء الرابط

الرابط هو برنامج يجمع الشيفرة المولَّدة في ملف قابل للتنفيذ. نظرًا لأن تنسيق الملف القابل للتنفيذ يختلف بين Linux وWindows وmacOS، فإن كل نظام له رابطه الخاص الذي يُلقي خطأً مختلفًا. السبب الجوهري للأخطاء هو نفسه: التكوين الافتراضي للرابط يفترض أن برنامجنا يعتمد على وقت تشغيل C، وهو ما لا يفعله.

لحل الأخطاء، نحتاج إلى إخبار الرابط بأنه لا ينبغي تضمين وقت تشغيل C. يمكننا القيام بذلك إما عن طريق تمرير مجموعة معينة من الوسيطات إلى الرابط أو عن طريق البناء لهدف bare metal.

### البناء لهدف Bare Metal

بشكل افتراضي، يحاول Rust بناء ملف قابل للتنفيذ قادر على العمل في بيئة نظامك الحالية. على سبيل المثال، إذا كنت تستخدم Windows على `x86_64`، يحاول Rust بناء ملف `.exe` لـ Windows يستخدم تعليمات `x86_64`. تُسمى هذه البيئة نظامك "المضيف".

لوصف بيئات مختلفة، يستخدم Rust سلسلة تسمى [_الثلاثي المستهدف_][_target triple_]. يمكنك رؤية الثلاثي المستهدف لنظامك المضيف عن طريق تشغيل `rustc --version --verbose`:

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

الإخراج أعلاه من نظام Linux بمعمارية `x86_64`. نرى أن الثلاثي `host` هو `x86_64-unknown-linux-gnu`، الذي يتضمن معمارية المعالج (`x86_64`) والبائع (`unknown`) ونظام التشغيل (`linux`) و[ABI] (`gnu`).

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

عن طريق التجميع لثلاثينا المضيف، يفترض مترجم Rust والرابط وجود نظام تشغيل أساسي مثل Linux أو Windows يستخدم وقت تشغيل C بشكل افتراضي، مما يتسبب في أخطاء الرابط. لذا لتجنب أخطاء الرابط، يمكننا التجميع لبيئة مختلفة بدون نظام تشغيل أساسي.

مثال على بيئة bare metal كهذه هو الثلاثي المستهدف `thumbv7em-none-eabihf`، الذي يصف نظام [ARM] [مدمج][embedded]. التفاصيل ليست مهمة، كل ما يهم هو أن الثلاثي المستهدف لا يحتوي على نظام تشغيل أساسي، وهو ما يشير إليه `none` في الثلاثي المستهدف. لكي نتمكن من التجميع لهذا الهدف، نحتاج إلى إضافته في rustup:

[embedded]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture

```
rustup target add thumbv7em-none-eabihf
```

هذا يُنزِّل نسخة من المكتبة القياسية (والأساسية) للنظام. الآن يمكننا بناء ملفنا القابل للتنفيذ المستقل لهذا الهدف:

```
cargo build --target thumbv7em-none-eabihf
```

بتمرير وسيطة `--target`، نقوم بـ[التجميع العابر][cross compile] لملفنا القابل للتنفيذ لهدف bare metal. نظرًا لأن النظام المستهدف ليس لديه نظام تشغيل، لا يحاول الرابط ربط وقت تشغيل C وينجح بناؤنا بدون أي أخطاء رابط.

[cross compile]: https://en.wikipedia.org/wiki/Cross_compiler

هذا هو النهج الذي سنستخدمه لبناء نواة نظام التشغيل الخاصة بنا. بدلاً من `thumbv7em-none-eabihf`، سنستخدم [هدفًا مخصصًا][custom target] يصف بيئة bare metal بمعمارية `x86_64`. سيتم شرح التفاصيل في المنشور التالي.

[custom target]: https://doc.rust-lang.org/rustc/targets/custom.html

### وسيطات الرابط

بدلاً من التجميع لنظام bare metal، من الممكن أيضًا حل أخطاء الرابط عن طريق تمرير مجموعة معينة من الوسيطات إلى الرابط. هذا ليس النهج الذي سنستخدمه لنواتنا، لذا هذا القسم اختياري ويُقدَّم فقط للاكتمال. انقر على _"وسيطات الرابط"_ أدناه لإظهار المحتوى الاختياري.

<details>

<summary>وسيطات الرابط</summary>

في هذا القسم، نناقش أخطاء الرابط التي تحدث على Linux وWindows وmacOS، ونشرح كيفية حلها عن طريق تمرير وسيطات إضافية إلى الرابط. لاحظ أن تنسيق الملف القابل للتنفيذ والرابط يختلفان بين أنظمة التشغيل، لذا فإن مجموعة مختلفة من الوسيطات مطلوبة لكل نظام.

#### Linux

على Linux يحدث خطأ الرابط التالي (مختصر):

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

المشكلة هي أن الرابط يتضمن روتين بدء وقت تشغيل C بشكل افتراضي، والذي يُسمى أيضًا `_start`. يتطلب بعض رموز مكتبة C القياسية `libc` التي لا نضمّنها بسبب سمة `no_std`، لذا لا يمكن للرابط حل هذه المراجع. لحل هذا، يمكننا إخبار الرابط بأنه لا ينبغي ربط روتين بدء C عن طريق تمرير علامة `-nostartfiles`.

إحدى طرق تمرير سمات الرابط عبر cargo هي أمر `cargo rustc`. يتصرف الأمر تمامًا مثل `cargo build`، ولكنه يسمح بتمرير خيارات إلى `rustc`، مترجم Rust الأساسي. يمتلك `rustc` علامة `-C link-arg` التي تمرر وسيطة إلى الرابط. مجتمعةً، يبدو أمر البناء الجديد لدينا كالتالي:

```
cargo rustc -- -C link-arg=-nostartfiles
```

الآن يُبنى صندوقنا كملف قابل للتنفيذ مستقل على Linux!

لم نحتج إلى تحديد اسم دالة نقطة الدخول الخاصة بنا صراحةً لأن الرابط يبحث عن دالة باسم `_start` بشكل افتراضي.

#### Windows

على Windows، يحدث خطأ رابط مختلف (مختصر):

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

خطأ "يجب تعريف نقطة الدخول" يعني أن الرابط لا يمكنه إيجاد نقطة الدخول. على Windows، اسم نقطة الدخول الافتراضي [يعتمد على النظام الفرعي المستخدم][windows-subsystems]. بالنسبة لنظام `CONSOLE` الفرعي، يبحث الرابط عن دالة باسم `mainCRTStartup` وبالنسبة لنظام `WINDOWS` الفرعي، يبحث عن دالة باسم `WinMainCRTStartup`. لتجاوز الافتراضي وإخبار الرابط بالبحث عن دالة `_start` الخاصة بنا بدلاً من ذلك، يمكننا تمرير وسيطة `/ENTRY` إلى الرابط:

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

الآن يحدث خطأ رابط مختلف:

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

يحدث هذا الخطأ لأن ملفات Windows القابلة للتنفيذ يمكنها استخدام [أنظمة فرعية][windows-subsystems] مختلفة. بالنسبة للبرامج العادية، يتم استنتاجها بناءً على اسم نقطة الدخول: إذا كانت نقطة الدخول تُسمى `main`، يُستخدم النظام الفرعي `CONSOLE`، وإذا كانت تُسمى `WinMain`، يُستخدم النظام الفرعي `WINDOWS`. نظرًا لأن دالة `_start` الخاصة بنا لها اسم مختلف، نحتاج إلى تحديد النظام الفرعي صراحةً:

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

نستخدم النظام الفرعي `CONSOLE` هنا، ولكن النظام الفرعي `WINDOWS` سيعمل أيضًا. بدلاً من تمرير `-C link-arg` عدة مرات، نستخدم `-C link-args` الذي يأخذ قائمة مفصولة بمسافات من الوسيطات.

مع هذا الأمر، يجب أن يُبنى ملفنا القابل للتنفيذ بنجاح على Windows.

#### macOS

على macOS، يحدث خطأ الرابط التالي (مختصر):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

تخبرنا رسالة الخطأ هذه أن الرابط لا يمكنه إيجاد دالة نقطة الدخول بالاسم الافتراضي `main` (لسبب ما، جميع الدوال مسبوقة بـ `_` على macOS). لتعيين نقطة الدخول إلى دالة `_start` الخاصة بنا، نمرر وسيطة الرابط `-e`:

```
cargo rustc -- -C link-args="-e __start"
```

تحدد علامة `-e` اسم دالة نقطة الدخول. نظرًا لأن جميع الدوال لها بادئة `_` إضافية على macOS، نحتاج إلى تعيين نقطة الدخول إلى `__start` بدلاً من `_start`.

الآن يحدث خطأ الرابط التالي:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

لا يدعم macOS [رسميًا الثنائيات المرتبطة بشكل ثابت][does not officially support statically linked binaries] ويتطلب من البرامج ربط مكتبة `libSystem` بشكل افتراضي. لتجاوز هذا وربط ثنائي ثابت، نمرر علامة `-static` إلى الرابط:

[does not officially support statically linked binaries]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

هذا لا يزال غير كافٍ، حيث يحدث خطأ رابط ثالث:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

يحدث هذا الخطأ لأن البرامج على macOS ترتبط بـ`crt0` ("C runtime zero") بشكل افتراضي. هذا مشابه للخطأ الذي واجهناه على Linux ويمكن حله أيضًا بإضافة وسيطة الرابط `-nostartfiles`:

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

الآن يجب أن يُبنى برنامجنا بنجاح على macOS.

#### توحيد أوامر البناء

الآن لدينا أوامر بناء مختلفة حسب النظام الأساسي المضيف، وهو ليس مثاليًا. لتجنب ذلك، يمكننا إنشاء ملف يُسمى `.cargo/config.toml` يحتوي على الوسيطات الخاصة بالنظام الأساسي:

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

يحتوي مفتاح `rustflags` على وسيطات تُضاف تلقائيًا إلى كل استدعاء لـ`rustc`. لمزيد من المعلومات حول ملف `.cargo/config.toml`، راجع [الوثائق الرسمية](https://doc.rust-lang.org/cargo/reference/config.html).

الآن يجب أن يكون برنامجنا قابلاً للبناء على جميع الأنظمة الأساسية الثلاثة بأمر `cargo build` بسيط.

#### هل يجب عليك القيام بذلك؟

في حين أنه من الممكن بناء ملف قابل للتنفيذ مستقل لـ Linux وWindows وmacOS، إلا أنه على الأرجح ليس فكرة جيدة. السبب هو أن ملفنا القابل للتنفيذ لا يزال يتوقع أشياء مختلفة، على سبيل المثال أن يتم تهيئة المكدس عند استدعاء دالة `_start`. بدون وقت تشغيل C، قد لا تكون بعض هذه المتطلبات مستوفاة، مما قد يتسبب في فشل برنامجنا، مثلاً من خلال خطأ التجزئة (segmentation fault).

</details>

## Summary

يبدو الحد الأدنى من ثنائي Rust المستقل كالتالي:

`src/main.rs`:

```rust
#![no_std] // لا تربط مكتبة Rust القياسية
#![no_main] // تعطيل جميع نقاط دخول مستوى Rust

use core::panic::PanicInfo;

#[unsafe(no_mangle)] // لا تشوّه اسم هذه الدالة
pub extern "C" fn _start() -> ! {
    // هذه الدالة هي نقطة الدخول، لأن الرابط يبحث عن دالة
    // باسم `_start` بشكل افتراضي
    loop {}
}

/// هذه الدالة تُستدعى عند حدوث panic
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

# الملف الشخصي المستخدم لـ `cargo build`
[profile.dev]
panic = "abort" # تعطيل فك تسلسل المكدس عند panic

# الملف الشخصي المستخدم لـ `cargo build --release`
[profile.release]
panic = "abort" # تعطيل فك تسلسل المكدس عند panic
```

لبناء هذا الثنائي، نحتاج إلى التجميع لهدف bare metal مثل `thumbv7em-none-eabihf`:

```
cargo build --target thumbv7em-none-eabihf
```

بدلاً من ذلك، يمكننا تجميعه للنظام المضيف عن طريق تمرير وسيطات رابط إضافية:

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

لاحظ أن هذا مجرد مثال بسيط لثنائي Rust مستقل. يتوقع هذا الثنائي أشياء مختلفة، على سبيل المثال، أن يتم تهيئة المكدس عند استدعاء دالة `_start`. **لذا لأي استخدام حقيقي لمثل هذا الثنائي، هناك خطوات إضافية مطلوبة**.

## جعل `rust-analyzer` سعيداً

مشروع [`rust-analyzer`](https://rust-analyzer.github.io/) طريقة رائعة للحصول على إكمال تلقائي للشيفرة ودعم "الانتقال إلى التعريف" (والعديد من الميزات الأخرى) لشيفرة Rust في محررك. يعمل بشكل جيد للغاية لمشاريع `#![no_std]` أيضًا، لذا أوصي باستخدامه لتطوير النواة!

إذا كنت تستخدم ميزة [`checkOnSave`](https://rust-analyzer.github.io/book/configuration.html#checkOnSave) لـ`rust-analyzer` (مُفعَّلة بشكل افتراضي)، فقد تُبلِّغ عن خطأ لدالة panic في نواتنا:

```
found duplicate lang item `panic_impl`
```

سبب هذا الخطأ هو أن `rust-analyzer` يستدعي `cargo check --all-targets` بشكل افتراضي، والذي يحاول أيضًا بناء الثنائي في وضع [الاختبار](https://doc.rust-lang.org/book/ch11-01-writing-tests.html) و[المعايرة](https://doc.rust-lang.org/rustc/tests/index.html#benchmarks).

<div class="note">

### المعنيان لـ"الهدف"

علامة `--all-targets` لا علاقة لها تمامًا بوسيطة `--target`.
هناك معنيان مختلفان لمصطلح "الهدف" في `cargo`:

- تحدد علامة `--target` **[_هدف التجميع_][_compilation target_]** الذي يجب تمريره إلى مترجم `rustc`. يجب تعيين هذا على [الثلاثي المستهدف] للجهاز الذي يجب أن يشغّل شيفرتنا.
- تشير علامة `--all-targets` إلى **[_هدف الحزمة_][_package target_]** لـ Cargo. يمكن أن تكون حزم Cargo مكتبة وثنائياً في نفس الوقت، لذا يمكنك تحديد الطريقة التي تريد بناء صندوقك بها. بالإضافة إلى ذلك، لدى Cargo أيضًا أهداف حزمة لـ[الأمثلة](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#examples) و[الاختبارات](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#tests) و[المعايرات](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#benchmarks). يمكن أن تتعايش أهداف الحزمة هذه، لذا يمكنك بناء/فحص نفس الصندوق مثلاً في وضع المكتبة أو الاختبار.

[_compilation target_]: https://doc.rust-lang.org/rustc/targets/index.html
[target triple]: https://clang.llvm.org/docs/CrossCompilation.html#target-triple
[_package target_]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html

</div>

بشكل افتراضي، يبني `cargo check` فقط أهداف حزمة _المكتبة_ و_الثنائي_.
ومع ذلك، يختار `rust-analyzer` فحص جميع أهداف الحزمة بشكل افتراضي عند تفعيل [`checkOnSave`](https://rust-analyzer.github.io/book/configuration.html#checkOnSave).
هذا هو السبب في أن `rust-analyzer` يُبلِّغ عن خطأ `lang item` أعلاه الذي لا نراه في `cargo check`.
إذا شغّلنا `cargo check --all-targets`، نرى الخطأ أيضًا:

```
error[E0152]: found duplicate lang item `panic_impl`
  --> src/main.rs:13:1
   |
13 | / fn panic(_info: &PanicInfo) -> ! {
14 | |     loop {}
15 | | }
   | |_^
   |
   = note: the lang item is first defined in crate `std` (which `test` depends on)
   = note: first definition in `std` loaded from /home/[...]/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/lib/libstd-8df6be531efb3fd0.rlib
   = note: second definition in the local crate (`blog_os`)
```

تخبرنا الملاحظة الأولى أن عنصر اللغة panic مُعرَّف بالفعل في صندوق `std`، وهو تبعية لصندوق `test`.
يتم تضمين صندوق `test` تلقائيًا عند بناء صندوق في [وضع الاختبار](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#tests).
هذا لا معنى له لنواتنا `#![no_std]` لأنه لا توجد طريقة لدعم المكتبة القياسية على bare metal.
لذا هذا الخطأ غير ذي صلة بمشروعنا ويمكننا تجاهله بأمان.

الطريقة الصحيحة لتجنب هذا الخطأ هي تحديد في `Cargo.toml` أن ثنائينا لا يدعم البناء في وضعَي `test` و`bench`.
يمكننا القيام بذلك بإضافة قسم `[[bin]]` إلى `Cargo.toml` لـ[تكوين البناء](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#configuring-a-target) لثنائيناطعنا:

```toml
# in Cargo.toml

[[bin]]
name = "blog_os"
test = false
bench = false
```

الأقواس المزدوجة حول `bin` ليست خطأً، هذه هي الطريقة التي يُعرِّف بها تنسيق TOML المفاتيح التي يمكن أن تظهر عدة مرات.
نظرًا لأن الصندوق يمكن أن يحتوي على ثنائيات متعددة، يمكن أن يظهر قسم `[[bin]]` عدة مرات في `Cargo.toml` أيضًا.
هذا هو السبب أيضًا في وجود حقل `name` الإلزامي، الذي يجب أن يتطابق مع اسم الثنائي (حتى يعرف `cargo` أي الإعدادات يجب تطبيقها على أي ثنائي).

بتعيين حقلَي [`test`](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-test-field) و[`bench`](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-bench-field) إلى `false`، نوجّه `cargo` لعدم بناء ثنائينا في وضع الاختبار أو المعايرة.
الآن لا ينبغي أن يُلقي `cargo check --all-targets` أي أخطاء بعد الآن، ويجب أن يكون تنفيذ `checkOnSave` لـ`rust-analyzer` سعيدًا أيضًا.

## ما التالي؟

يشرح [المنشور التالي] الخطوات اللازمة لتحويل ثنائينا المستقل إلى حد أدنى من نواة نظام التشغيل. يتضمن ذلك إنشاء هدف مخصص، ودمج ملفنا القابل للتنفيذ مع محمّل الإقلاع، وتعلم كيفية طباعة شيء على الشاشة.

[المنشور التالي]: @/edition-2/posts/02-minimal-rust-kernel/index.md