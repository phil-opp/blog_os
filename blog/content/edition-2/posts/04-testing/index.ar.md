+++
title = "الاختبار"
weight = 4
path = "ar/testing"
date = 2019-04-27

[extra]
# Please update this when updating the translation
translation_based_on_commit = "211f460251cd332905225c93eb66b1aff9f4aefd"
chapter = "Bare Bones"
comments_search_term = 1009
# GitHub usernames of the people that translated this post
translators = ["mindfreq"]
rtl = true
+++

يستكشف هذا المقال اختبارات الوحدة والتكامل في executables `no_std`. سنستخدم دعم Rust لـ test frameworks المخصصة لتنفيذ دوال الاختبار داخل نواتنا. للإبلاغ عن النتائج خارج QEMU، سنستخدم ميزات مختلفة من QEMU وأداة `bootimage`.

<!-- more -->

هذا المدونة مطوّرة بشكل مفتوح على [GitHub]. إذا كان لديك أي مشاكل أو أسئلة، يرجى فتح issue هناك. يمكنك أيضًا ترك تعليقات [في الأسفل]. يمكن العثور على الكود المصدري الكامل لهذا المقال في فرع [`post-04`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[في الأسفل]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-04

<!-- toc -->

## المتطلبات

يحل هذا المقال محل مقالات [_Unit Testing_] و [_Integration Tests_] (المهجورة الآن). يفترض أنك اتبعت مقال [_A Minimal Rust Kernel_] بعد 2019-04-27. بشكل رئيسي، يتطلب أن يكون لديك ملف `.cargo/config.toml` يـ[يضبط هدفًا افتراضيًا][sets a default target] و[يحدد runner executable][defines a runner executable].

[_Unit Testing_]: @/edition-2/posts/deprecated/04-unit-testing/index.md
[_Integration Tests_]: @/edition-2/posts/deprecated/05-integration-tests/index.md
[_A Minimal Rust Kernel_]: @/edition-2/posts/02-minimal-rust-kernel/index.md
[sets a default target]: @/edition-2/posts/02-minimal-rust-kernel/index.md#set-a-default-target
[defines a runner executable]: @/edition-2/posts/02-minimal-rust-kernel/index.md#using-cargo-run

## الاختبار في Rust

لدى Rust [test framework مدمج][built-in test framework] قادر على تشغيل اختبارات الوحدة دون الحاجة إلى إعداد أي شيء. فقط أنشئ دالة تتحقق من بعض النتائج من خلال assertions وأضف السمة `#[test]` إلى ترويسة الدالة. ثم `cargo test` ستجد تلقائيًا وتنفذ جميع دوال الاختبار في crate الخاص بك.

[built-in test framework]: https://doc.rust-lang.org/book/ch11-00-testing.html

لتفعيل الاختبار لثنائي نواتنا، يمكننا تعيين العلم `test` في Cargo.toml إلى `true`:

```toml
# in Cargo.toml

[[bin]]
name = "blog_os"
test = true
bench = false
```

يحدد [قسم `[[bin]]`](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#configuring-a-target) كيف يجب على `cargo` تجميع executable `blog_os` الخاص بنا.
يحدد حقل `test` ما إذا كان الاختبار مدعومًا لهذا الـ executable.
قمنا بتعيين `test = false` في المقال الأول [لجعل `rust-analyzer` سعيدًا](@/edition-2/posts/01-freestanding-rust-binary/index.md#making-rust-analyzer-happy)، لكننا الآن نريد تفعيل الاختبار، لذلك نعيّنه إلى `true`.

لسوء الحظ، الاختبار أكثر تعقيدًا بعض الشيء لتطبيقات `no_std` مثل نواتنا. المشكلة هي أن test framework في Rust يستخدم ضمنيًا مكتبة [`test`] المدمجة، التي تعتمد على مكتبة القياسية. هذا يعني أننا لا نستطيع استخدام test framework الافتراضي لنواتنا `#[no_std]`.

[`test`]: https://doc.rust-lang.org/test/index.html

يمكننا رؤية هذا عندما نحاول تشغيل `cargo test` في مشروعنا:

```
> cargo test
   Compiling blog_os v0.1.0 (/…/blog_os)
error[E0463]: can't find crate for `test`
```

بما أن مكتبة `test` تعتمد على مكتبة القياسية، فهي غير متاحة لهدف bare metal الخاص بنا. بينما [نقل مكتبة `test` إلى سياق `#[no_std]` ممكن][utest]، فهو غير مستقر للغاية ويحتاج إلى بعض الحيل، مثل إعادة تعريف macro `panic`.

[utest]: https://github.com/japaric/utest

### أطر الاختبار المخصصة

لحسن الحظ، يدعم Rust استبدال test framework الافتراضي من خلال الميزة غير المستقرة [`custom_test_frameworks`]. لا تحتاج هذه الميزة إلى مكتبات خارجية وبالتالي تعمل أيضًا في بيئات `#[no_std]`. تعمل عن طريق جمع جميع الدوال الموسومة بسمة `#[test_case]` ثم استدعاء دالة runner محددة من قبل المستخدم مع قائمة الاختبارات كوسيطة. بهذا، تعطي التنفيذ تحكمًا أقصى في عملية الاختبار.

[`custom_test_frameworks`]: https://doc.rust-lang.org/unstable-book/language-features/custom-test-frameworks.html

العيب مقارنة بـ test framework الافتراضي هو أن العديد من الميزات المتقدمة، مثل [اختبارات `should_panic`][`should_panic` tests]، غير متاحة. بدلاً من ذلك، يعتمد على التنفيذ نفسه توفير هذه الميزات إذا لزم الأمر. هذا مثالي بالنسبة لنا لأن لدينا بيئة تنفيذ خاصة جدًا حيث لن تعمل التطبيقات الافتراضية لهذه الميزات المتقدمة على أي حال. على سبيل المثال، تعتمد السمة `#[should_panic]` على stack unwinding لcatch الـ panics، التي عطّلناها لنواتنا.

[`should_panic` tests]: https://doc.rust-lang.org/book/ch11-01-writing-tests.html#checking-for-panics-with-should_panic

لتنفيذ test framework مخصص لنواتنا، نضيف ما يلي إلى `main.rs`:

```rust
// in src/main.rs

#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}
```

الـ runner الخاص بنا يطبع فقط رسالة تصحيح قصيرة ثم يستدعي كل دالة اختبار في القائمة. نوع الوسيطة `&[&dyn Fn()]` هي [_slice_] من مراجع [_trait object_] لـ trait [_Fn()_]. إنها أساسًا قائمة مراجع لأنواع يمكن استدعاؤها كدالة. بما أن الدالة غير مفيدة لتشغيلات non-test، نستخدم السمة `#[cfg(test)]` لتضمينها فقط للاختبارات.

[_slice_]: https://doc.rust-lang.org/std/primitive.slice.html
[_trait object_]: https://doc.rust-lang.org/1.30.0/book/first-edition/trait-objects.html
[_Fn()_]: https://doc.rust-lang.org/std/ops/trait.Fn.html

عندما نشغّل `cargo test` الآن، نرى أنه ينجح (إذا لم ينجح، راجع الملاحظة أدناه). ومع ذلك، لا نزال نرى "Hello World" بدلاً من الرسالة من `test_runner` الخاص بنا. السبب هو أن دالة `_start` الخاصة بنا لا تزال تستخدم كـ entry point. تولّد ميزة custom test frameworks دالة `main` تستدعي `test_runner`، ولكن هذه الدالة تُتجاهل لأننا نستخدم السمة `#[no_main]` ونوفر entry point خاص بنا.

<div class = "warning">

**ملاحظة:** يوجد حاليًا bug في cargo يؤدي إلى أخطاء "duplicate lang item" عند `cargo test` في بعض الحالات. يحدث عندما تعيّن `panic = "abort"` لـ profile في `Cargo.toml`. حاول إزالته، ثم `cargo test` سيعمل. بدلاً من ذلك، إذا لم يعمل ذلك، أضف `panic-abort-tests = true` إلى قسم `[unstable]` في ملف `.cargo/config.toml`. راجع [cargo issue](https://github.com/rust-lang/cargo/issues/7359) لمزيد من المعلومات حول هذا.

</div>

لحل هذا، نحتاج أولاً إلى تغيير اسم الدالة المولّدة إلى شيء مختلف عن `main` من خلال السمة `reexport_test_harness_main`. ثم يمكننا استدعاء الدالة المعاد تسميتها من دالة `_start` الخاصة بنا:

```rust
// in src/main.rs

#![reexport_test_harness_main = "test_main"]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}
```

نعيّن اسم دالة entry point لـ test framework إلى `test_main` ونستدعيها من entry point `_start` الخاص بنا. نستخدم [التجميع الشرطي][conditional compilation] لإضافة استدعاء `test_main` فقط في سياقات الاختبار لأن الدالة لا تُولّد في تشغيل عادي.

[conditional compilation]: https://doc.rust-lang.org/1.30.0/book/first-edition/conditional-compilation.html

عندما ننفّذ `cargo test` الآن، نرى رسالة "Running 0 tests" من `test_runner` الخاص بنا على الشاشة. الآن نحن مستعدون لإنشاء أول دالة اختبار لنا:

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}
```

عندما نشغّل `cargo test` الآن، نرى الإخراج التالي:

![QEMU printing "Hello World!", "Running 1 tests", and "trivial assertion... [ok]"](qemu-test-runner-output.png)

الـ `tests` slice الممرر إلى دالة `test_runner` الخاصة بنا يحتوي الآن على مرجع إلى دالة `trivial_assertion`. من الإخراج `trivial assertion... [ok]` على الشاشة، نرى أن الاختبار تم استدعاؤه وأنه نجح.

بعد تنفيذ الاختبارات، يعود `test_runner` الخاص بنا إلى دالة `test_main`، التي بدورها تعود إلى دالة entry point `_start` الخاصة بنا. في نهاية `_start`، ندخل في loop غير محدود لأن دالة entry point غير مسموح لها بالعودة. هذه مشكلة، لأننا نريد `cargo test` أن ينتهي بعد تشغيل جميع الاختبارات.

## الخروج من QEMU

الآن، لدينا loop غير محدود في نهاية دالة `_start` الخاصة بنا ونحتاج إلى إغلاق QEMU يدويًا في كل تشغيل لـ `cargo test`. هذا مؤسف لأننا نريد أيضًا تشغيل `cargo test` في scripts دون تفاعل المستخدم. الحل النظيف لهذا سيكون تنفيذ طريقة مناسبة لإيقاف نظام التشغيل الخاص بنا. لسوء الحظ، هذا معقد نسبيًا لأنه يتطلب تنفيذ دعم إما لمعيار إدارة الطاقة [APM] أو [ACPI].

[APM]: https://wiki.osdev.org/APM
[ACPI]: https://wiki.osdev.org/ACPI

لحسن الحظ، هناك مخرج: يدعم QEMU جهازًا خاصًا يسمى `isa-debug-exit`، يوفر طريقة سهلة للخروج من QEMU من نظام الضيف. لتفعيله، نحتاج إلى تمرير وسيطة `-device` إلى QEMU. يمكننا ذلك بإضافة مفتاح تكوين `package.metadata.bootimage.test-args` في `Cargo.toml`:

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
```

يضيف `bootimage runner` الـ `test-args` إلى أمر QEMU الافتراضي لجميع executables الاختبار. لـ `cargo run` العادي، تُتجاهل الوسائط.

مع اسم الجهاز (`isa-debug-exit`)، نمرر المعلمتين `iobase` و `iosize` اللتين تحددان _I/O port_ الذي يمكن الوصول إليه من نواتنا.

### منافذ الإدخال/الإخراج

هنا نهجان مختلفان للتواصل بين وحدة المعالجة المركزية والأجهزة الطرفية على x86، **memory-mapped I/O** و **port-mapped I/O**. استخدمنا بالفعل memory-mapped I/O للوصول إلى [VGA text buffer] من خلال عنوان الذاكرة `0xb8000`. هذا العنوان غير مُعيّن إلى RAM بل إلى ذاكرة ما على جهاز VGA.

[VGA text buffer]: @/edition-2/posts/03-vga-text-buffer/index.md

على النقيض، يستخدم port-mapped I/O I/O bus منفصل للتواصل. كل جهاز طرفي متصل لديه رقم port واحد أو أكثر. للتواصل مع مثل هذا I/O port، هناك تعليمات CPU خاصة تسمى `in` و `out`، التي تأخذ رقم port وبايت بيانات (هناك أيضًا تنويعات من هذه الأوامر تسمح بإرسال `u16` أو `u32`).

يستخدم جهاز `isa-debug-exit` port-mapped I/O. تحدد المعلمة `iobase` على أي عنوان port يجب أن يكون الجهاز (`0xf4` هو [port غير مستخدم عمومًا][list of x86 I/O ports] على IO bus لـ x86) وتحدد `iosize` حجم port (`0x04` يعني أربعة بايتات).

[list of x86 I/O ports]: https://wiki.osdev.org/I/O_Ports#The_list

### استخدام جهاز الخروج

وظيفة جهاز `isa-debug-exit` بسيطة جدًا. عند كتابة `value` إلى I/O port المحدد بواسطة `iobase`، تجعل QEMU يخرج بـ [exit status] `(value << 1) | 1`. لذلك عندما نكتب `0` إلى port، سيخرج QEMU بـ exit status `(0 << 1) | 1 = 1`، وعندما نكتب `1` إلى port، سيخرج بـ exit status `(1 << 1) | 1 = 3`.

[exit status]: https://en.wikipedia.org/wiki/Exit_status

بدلاً من استدعاء تعليمات assembly `in` و `out` يدويًا، نستخدم التجريدات التي توفرها مكتبة [`x86_64`]. لإضافة dependency على تلك المكتبة، نضيفها إلى قسم `dependencies` في `Cargo.toml`:

[`x86_64`]: https://docs.rs/x86_64/0.14.2/x86_64/

```toml
# in Cargo.toml

[dependencies]
x86_64 = "0.14.2"
```

الآن يمكننا استخدام النوع [`Port`] الذي توفره المكتبة لإنشاء دالة `exit_qemu`:

[`Port`]: https://docs.rs/x86_64/0.14.2/x86_64/instructions/port/struct.Port.html

```rust
// in src/main.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

الدالة تنشئ [`Port`] جديدًا عند `0xf4`، وهو `iobase` لجهاز `isa-debug-exit`. ثم تكتب exit code الممرر إلى port. نستخدم `u32` لأننا حددنا `iosize` لجهاز `isa-debug-exit` كـ 4 بايتات. كلتا العمليتين غير آمنتين لأن الكتابة إلى I/O port يمكن أن تؤدي عمومًا إلى سلوك عشوائي.

لتحديد exit status، ننشئ enum `QemuExitCode`. الفكرة هي الخروج بـ exit code النجاح إذا نجحت جميع الاختبارات و exit code الفشل بخلاف ذلك. الـ enum محدد كـ `#[repr(u32)]` لتمثيل كل variant بـ عدد صحيح `u32`. نستخدم exit code `0x10` للنجاح و `0x11` للفشل. exit codes الفعلية لا تهم كثيرًا، طالما لا تتعارض مع exit codes الافتراضية لـ QEMU. على سبيل المثال، استخدام exit code `0` للنجاح ليس فكرة جيدة لأنه يصبح `(0 << 1) | 1 = 1` بعد التحويل، وهو exit code الافتراضي عندما يفشل QEMU في التشغيل. لذلك لن نستطيع التمييز بين خطأ QEMU وتشغيل اختبار ناجح.

يمكننا الآن تحديث `test_runner` للخروج من QEMU بعد تشغيل جميع الاختبارات:

```rust
// in src/main.rs

fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    /// new
    exit_qemu(QemuExitCode::Success);
}
```

عندما نشغّل `cargo test` الآن، نرى أن QEMU يُغلق فورًا بعد تنفيذ الاختبارات. المشكلة هي أن `cargo test` يفسر الاختبار على أنه فشل حتى لو مررنا exit code `Success`:

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.03s
     Running target/x86_64-blog_os/debug/deps/blog_os-5804fc7d2dd4c9be
Building bootloader
   Compiling bootloader v0.5.3 (/home/philipp/Documents/bootloader)
    Finished release [optimized + debuginfo] target(s) in 1.07s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-5804fc7d2dd4c9be.bin -device isa-debug-exit,iobase=0xf4,
    iosize=0x04`
error: test failed, to rerun pass '--bin blog_os'
```

المشكلة هي أن `cargo test` يعتبر جميع error codes باستثناء `0` كفشل.

### رمز الخروج الناجح

للتعامل مع هذا، يوفر `bootimage` مفتاح تكوين `test-success-exit-code` يربط exit code محدد بـ exit code `0`:

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = […]
test-success-exit-code = 33         # (0x10 << 1) | 1
```

مع هذا التكوين، يربط `bootimage` exit code النجاح الخاص بنا بـ exit code 0، بحيث يتعرف `cargo test` بشكل صحيح على حالة النجاح ولا يحسب الاختبار على أنه فشل.

الـ runner الخاص بنا الآن يغلق QEMU تلقائيًا ويربط نتائج الاختبار بشكل صحيح. لا نزال نرى نافذة QEMU تفتح لفترة قصيرة جدًا، لكنها لا تكفي لقراءة النتائج. سيكون من الجيد لو كان بإمكاننا طباعة نتائج الاختبار إلى console بدلاً من ذلك، حتى نتمكن من رؤيتها بعد خروج QEMU.

## الطباعة على وحدة التحكم

لرؤية إخراج الاختبار على console، نحتاج إلى إرسال البيانات من نواتنا إلى نظام المضيف بطريقة ما. هناك طرق مختلفة لتحقيق هذا، على سبيل المثال، بإرسال البيانات عبر واجهة TCP network. ومع ذلك، إعداد network stack معقد للغاية، لذلك سنختار حلًا أبسط بدلاً من ذلك.

### المنفذ التسلسلي

طريقة بسيطة لإرسال البيانات هي استخدام [serial port]، معيار واجهة قديم لم يعد موجودًا في أجهزة الكمبيوتر الحديثة. من السهل برمجته ويمكن لـ QEMU إعادة توجيه البايتات المرسلة عبر serial إلى stdout أو ملف.

[serial port]: https://en.wikipedia.org/wiki/Serial_port

الرقائق التي تنفذ واجهة serial تسمى [UARTs]. هناك [عديد من نماذج UART][lots of UART models] على x86، لكن لحسن الحظ الاختلافات الوحيدة بينها هي بعض الميزات المتقدمة التي لا نحتاجها. UARTs الشائعة اليوم جميعها متوافقة مع [16550 UART]، لذلك سنستخدم ذلك النموذج لـ test framework الخاص بنا.

[UARTs]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter
[lots of UART models]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter#Models
[16550 UART]: https://en.wikipedia.org/wiki/16550_UART

سنستخدم مكتبة [`uart_16550`] لتهيئة UART وإرسال البيانات عبر serial port. لإضافتها كـ dependency، نحدّث `Cargo.toml` و `main.rs`:

[`uart_16550`]: https://docs.rs/uart_16550

```toml
# in Cargo.toml

[dependencies]
uart_16550 = "0.6.0"
```

تحتوي مكتبة `uart_16550` على نوع [`Uart16550Tty`](https://docs.rs/uart_16550/latest/uart_16550/struct.Uart16550Tty.html) يهيئ UART في وضع [TTY](https://en.wikipedia.org/wiki/Teleprinter)، مما يتيح لنا إرسال النصوص بسهولة.

لنستخدم هذا النوع في module `serial` جديد:

```rust
// in src/main.rs

mod serial;
```

```rust
// in src/serial.rs

use uart_16550::{Config, Uart16550Tty, backend::PioBackend};
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL1: Mutex<Uart16550Tty<PioBackend>> = Mutex::new(unsafe {
        Uart16550Tty::new_port(0x3F8, Config::default())
            .expect("failed to initialize UART")
    });
}
```

مثل [VGA text buffer][vga lazy-static]، نستخدم `lazy_static` و spinlock لإنشاء instance `static` writer. باستخدام `lazy_static` نضمن أن UART تُهيَّأ مرة واحدة فقط عند أول استخدام.

مثل جهاز `isa-debug-exit`، يُبرمج UART باستخدام port I/O، وهو ما يشير إليه parameter [`PioBackend`](https://docs.rs/uart_16550/latest/uart_16550/backend/struct.PioBackend.html). بما أن UART أكثر تعقيدًا، يستخدم عدة I/O ports لبرمجة سجلات الجهاز المختلفة. الدالة غير الآمنة `Uart16550Tty::new_port` تتوقع عنوان أول I/O port لـ UART كوسيطة، من خلاله يمكن حساب عناوين جميع ports المطلوبة. نمرر عنوان port `0x3F8`، وهو رقم port القياسي لأول واجهة serial.

[vga lazy-static]: @/edition-2/posts/03-vga-text-buffer/index.md#lazy-statics

لجعل serial port سهل الاستخدام، نضيف macros `serial_print!` و `serial_println!`:

```rust
// in src/serial.rs

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
```

التنفيذ مشابه جدًا لتنفيذ macros `print` و `println` الخاصة بنا. بما أن نوع `Uart16550Tty` ينفذ بالفعل trait [`fmt::Write`]، لا نحتاج إلى توفير تنفيذنا الخاص.

[`fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

الآن يمكننا الطباعة إلى واجهة serial بدلاً من VGA text buffer في كود الاختبار:

```rust
// in src/main.rs

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    […]
}

#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

لاحظ أن macro `serial_println` تعيش مباشرة تحت namespace الجذر لأننا استخدمنا السمة `#[macro_export]`، لذلك الاستيراد عبر `use crate::serial::serial_println` لن يعمل.

### وسائط QEMU

لرؤية serial output من QEMU، نحتاج إلى استخدام الوسيطة `-serial` لإعادة توجيه الإخراج إلى stdout:

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio"
]
```

عندما نشغّل `cargo test` الآن، نرى إخراج الاختبار مباشرة في console:

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [ok]
```

ومع ذلك، عندما يفشل اختبار، لا نزال نرى الإخراج داخل QEMU لأن panic handler لا يزال يستخدم `println`. لمحاكاة هذا، يمكننا تغيير assertion في اختبار `trivial_assertion` إلى `assert_eq!(0, 1)`:

![QEMU printing "Hello World!" and "panicked at 'assertion failed: `(left == right)`
    left: `0`, right: `1`', src/main.rs:55:5](qemu-failed-test.png)

نرى أن رسالة panic لا تزال تُطبع إلى buffer VGA، بينما يُطبع إخراج الاختبار الآخر إلى serial port. رسالة panic مفيدة جدًا، لذلك سيكون من المفيد رؤيتها في console أيضًا.

### طباعة رسالة خطأ عند الـ panic

للخروج من QEMU مع رسالة خطأ عند panic، يمكننا استخدام [التجميع الشرطي][conditional compilation] لاستخدام panic handler مختلف في وضع الاختبار:

```rust
// in src/main.rs

// our existing panic handler
#[cfg(not(test))] // new attribute
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

// our panic handler in test mode
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}
```

لـ panic handler الاختبار، نستخدم `serial_println` بدلاً من `println` ثم نخرج من QEMU بـ exit code فشل. لاحظ أننا لا نزال نحتاج إلى `loop` غير محدود بعد استدعاء `exit_qemu` لأن المترجم لا يعرف أن جهاز `isa-debug-exit` يسبب خروج البرنامج.

الآن QEMU يخرج أيضًا للاختبارات الفاشلة ويطبع رسالة خطأ مفيدة على console:

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [failed]

Error: panicked at 'assertion failed: `(left == right)`
  left: `0`,
 right: `1`', src/main.rs:65:5
```

بما أننا نرى جميع إخراج الاختبار على console الآن، لم نعد بحاجة إلى نافذة QEMU التي تظهر لفترة قصيرة. لذلك يمكننا إخفاؤها تمامًا.

### إخفاء QEMU

بما أننا نדווח عن نتائج الاختبار الكاملة باستخدام جهاز `isa-debug-exit` و serial port، لم نعد بحاجة إلى نافذة QEMU. يمكننا إخفاؤها بتمرير الوسيطة `-display none` إلى QEMU:

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio",
    "-display", "none"
]
```

الآن يعمل QEMU تمامًا في الخلفية ولا تفتح نافذة بعد الآن. هذا ليس أقل إزعاجًا فحسب، بل يسمح أيضًا لـ test framework بالعمل في بيئات بدون واجهة مستخدم رسومية، مثل خدمات CI أو اتصالات [SSH].

[SSH]: https://en.wikipedia.org/wiki/Secure_Shell

### المهلات الزمنية

بما أن `cargo test` ينتظر حتى ينتهي test runner، فإن اختبار لا يعود أبدًا يمكن أن يحجب test runner إلى الأبد. هذا مؤسف، لكن ليس مشكلة كبيرة في الممارسة العملية لأنه عادةً ما يكون من السهل تجنب loops غير محدودة. في حالتنا، يمكن أن تحدث loops غير محدودة في حالات مختلفة:

- يفشل bootloader في تحميل نواتنا، مما يسبب إعادة إقلاع النظام بشكل غير محدود.
- يفشل BIOS/UEFI firmware في تحميل bootloader، مما يسبب نفس إعادة الإقلاع غير المحدودة.
- يدخل وحدة المعالجة المركزية في عبارة `loop {}` في نهاية بعض دوالنا، على سبيل المثال لأن جهاز خروج QEMU لا يعمل بشكل صحيح.
- تسبب الأجهزة إعادة ضبط النظام، على سبيل المثال عند عدم التقاط استثناء وحدة المعالجة المركزية (يُشرح في مقال مستقبلي).

بما أن loops غير محدودة يمكن أن تحدث في العديد من الحالات، تضبط أداة `bootimage` timeout مدته 5 دقائق لكل executable اختبار افتراضيًا. إذا لم ينتهِ الاختبار خلال هذا الوقت، يُحدّد على أنه فاشل وتُطبع خطأ "Timed Out" إلى console. تضمن هذه الميزة أن الاختبارات العالقة في loop غير محدود لا تحجب `cargo test` إلى الأبد.

يمكنك تجربة ذلك بنفسك بإضافة عبارة `loop {}` في اختبار `trivial_assertion`. عندما تشغّل `cargo test`، ترى أن الاختبار يُحدّد على أنه timed out بعد 5 دقائق. مدة Timeout [قابلة للتكوين][bootimage config] عبر مفتاح `test-timeout` في Cargo.toml:

[bootimage config]: https://github.com/rust-osdev/bootimage#configuration

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-timeout = 300          # (in seconds)
```

إذا كنت لا تريد الانتظار 5 دقائق لي timed out اختبار `trivial_assertion`، يمكنك تقليل القيمة أعلاه مؤقتًا.

### إدراج عبارات الطباعة تلقائيًا

يحتاج اختبار `trivial_assertion` حاليًا إلى طباعة معلومات حالته الخاصة باستخدام `serial_print!`/`serial_println!`:

```rust
#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

إضافة عبارات الطباعة هذه يدويًا لكل اختبار نكتبه مرهق، لذلك لنحدّث `test_runner` لطباعة هذه الرسائل تلقائيًا. لذلك، نحتاج إلى إنشاء trait `Testable` جديد:

```rust
// in src/main.rs

pub trait Testable {
    fn run(&self) -> ();
}
```

الحيلة الآن هي تنفيذ هذا trait لجميع الأنواع `T` التي تنفذ [trait `Fn()`][`Fn()` trait]:

[`Fn()` trait]: https://doc.rust-lang.org/stable/core/ops/trait.Fn.html

```rust
// in src/main.rs

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}
```

ننفذ دالة `run` عن طريق طباعة اسم الدالة أولاً باستخدام دالة [`any::type_name`]. هذه الدالة منفذة مباشرة في المترجم وتعيد وصفًا نصيًا لكل نوع. للدوال، النوع هو اسمها، لذلك هذا بالضبط ما نريده في هذه الحالة. الحرف `\t` هو [حرف tab][tab character]، الذي يضيف بعض المحاذاة لرسائل `[ok]`.

[`any::type_name`]: https://doc.rust-lang.org/stable/core/any/fn.type_name.html
[tab character]: https://en.wikipedia.org/wiki/Tab_character

بعد طباعة اسم الدالة، نستدعي دالة الاختبار عبر `self()`. هذا يعمل فقط لأننا نتطلب أن `self` ينفذ trait `Fn()`. بعد أن تعود دالة الاختبار، نطبع `[ok]` للإشارة إلى أن الدالة لم تُ panic.

الخطوة الأخيرة هي تحديث `test_runner` لاستخدام trait `Testable` الجديد:

```rust
// in src/main.rs

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) { // new
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run(); // new
    }
    exit_qemu(QemuExitCode::Success);
}
```

التغييران الوحيدان هما نوع وسيطة `tests` من `&[&dyn Fn()]` إلى `&[&dyn Testable]` وحقيقة أننا نستدعي الآن `test.run()` بدلاً من `test()`.

يمكننا الآن إزالة عبارات الطباعة من اختبار `trivial_assertion` لأنها تُطبع تلقائيًا الآن:

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
```

إخراج `cargo test` الآن يبدو كالتالي:

```
Running 1 tests
blog_os::trivial_assertion...	[ok]
```

اسم الدالة يتضمن الآن المسار الكامل إلى الدالة، وهو مفيد عندما يكون لدوال الاختبار في modules مختلفة نفس الاسم. بخلاف ذلك، يبدو الإخراج كما كان من قبل، لكن لم نعد بحاجة إلى إضافة عبارات طباعة إلى اختباراتنا يدويًا.

## اختبار Buffer VGA

الآن بعد أن لدينا test framework يعمل، يمكننا إنشاء بعض الاختبارات لتنفيذ VGA buffer الخاص بنا. أولاً، ننشئ اختبارًا بسيطًا جدًا للتحقق من أن `println` يعمل دون panic:

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}
```

الاختبار يطبع فقط شيئًا إلى buffer VGA. إذا انتهى دون panic، فهذا يعني أن استدعاء `println` لم يُ panic أيضًا.

لضمان عدم حدوث panic حتى لو طُبعت العديد من الأسطر وتم إزالتها من الشاشة، يمكننا إنشاء اختبار آخر:

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}
```

يمكننا أيضًا إنشاء دالة اختبار للتحقق من أن الأسطر المطبوعة تظهر فعليًا على الشاشة:

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_output() {
    let s = "Some test string that fits on a single line";
    println!("{}", s);
    for (i, c) in s.chars().enumerate() {
        let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
        assert_eq!(char::from(screen_char.ascii_character), c);
    }
}
```

الدالة تحدد سلسلة نصية اختبارية، تطبعها باستخدام `println`، ثم تمر على أحرف الشاشة لـ `WRITER` الثابت، الذي يمثل VGA text buffer. بما أن `println` يطبع إلى آخر سطر شاشة ثم يضيف فورًا سطرًا جديدًا، يجب أن تظهر السلسلة النصية في السطر `BUFFER_HEIGHT - 2`.

باستخدام [`enumerate`]، نعد عدد التكرارات في المتغير `i`، الذي نستخدمه لتحميل حرف الشاشة المقابل لـ `c`. بمقارنة `ascii_character` لحرف الشاشة مع `c`، نضمن أن كل حرف من السلسلة النصية يظهر فعليًا في VGA text buffer.

[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate

كما يمكنك التخيل، يمكننا إنشاء العديد من دوال الاختبار الأخرى. على سبيل المثال، دالة تختبر عدم حدوث panic عند طباعة أسطر طويلة جدًا وأنها تُ zabat بشكل صحيح، أو دالة لاختبار أن أسطر جديدة وأحرف غير قابلة للطباعة وأحرف non-unicode تُتعامل معها بشكل صحيح.

لفترة متبقية من هذا المقال، سنشرح كيفية إنشاء _integration tests_ لاختبار تفاعل المكونات المختلفة معًا.

## اختبارات التكامل

الاصطلاح لـ [integration tests] في Rust هو وضعها في دليل `tests` في جذر المشروع (بجانب دليل `src`). كلاً من test framework الافتراضي و custom test frameworks سيجدان تلقائيًا وينفذان جميع الاختبارات في ذلك الدليل.

[integration tests]: https://doc.rust-lang.org/book/ch11-03-test-organization.html#integration-tests

جميع integration tests هي executables مستقلة و completely منفصلة عن `main.rs` الخاص بنا. هذا يعني أن كل اختبار يحتاج إلى تحديد entry point خاص به. لننشئ integration test نموذجيًا يسمى `basic_boot` لمعرفة كيف يعمل بالتفصيل:

```rust
// in tests/basic_boot.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

#[unsafe(no_mangle)] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

fn test_runner(tests: &[&dyn Fn()]) {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
```

بما أن integration tests هي executables مستقلة، نحتاج إلى توفير جميع سمات crate (`no_std` و `no_main` و `test_runner` إلخ) مرة أخرى. نحتاج أيضًا إلى إنشاء entry point `_start` جديدة، التي تستدعي دالة entry point الاختبار `test_main`. لا نحتاج إلى أي سمات `cfg(test)` لأن integration test executables لا تُبنى أبدًا في وضع non-test.

نستخدم macro [`unimplemented`] الذي يُ panic دائمًا كـ placeholder لدالة `test_runner` ونضع فقط `loop` في معالج `panic` حاليًا. بشكل مثالي، نريد تنفيذ هذه الدوال بالضبط كما فعلنا في `main.rs` باستخدام macro `serial_println` ودالة `exit_qemu`. المشكلة هي أننا لا نملك الوصول إلى هذه الدوال لأن الاختبارات تُبنى completely منفصلة عن executable `main.rs` الخاص بنا.

[`unimplemented`]: https://doc.rust-lang.org/core/macro.unimplemented.html

إذا شغّلت `cargo test` في هذه المرحلة، ستحصل على loop غير محدود لأن panic handler يُ loop بشكل غير محدود. تحتاج إلى استخدام اختصار لوحة المفاتيح `ctrl+c` للخروج من QEMU.

### إنشاء مكتبة

لجعل الدوال المطلوبة متاحة لـ integration test، نحتاج إلى فصل مكتبة من `main.rs`، التي يمكن تضمينها من قبل crates أخرى و integration test executables. لذلك، ننشئ ملف `src/lib.rs` جديدًا:

```rust
// src/lib.rs

#![no_std]

```

مثل `main.rs`، فإن `lib.rs` هو ملف خاص يتعرف عليه cargo تلقائيًا. المكتبة هي وحدة تجميع منفصلة، لذلك نحتاج إلى تحديد السمة `#![no_std]` مرة أخرى.

لجعل مكتبتنا تعمل مع `cargo test`، نحتاج أيضًا إلى نقل دوال الاختبار والسمات من `main.rs` إلى `lib.rs`:

```rust
// in src/lib.rs

#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

/// Entry point for `cargo test`
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    test_main();
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
```

لجعل `test_runner` متاحًا لـ executables و integration tests، نجعله public ولا نطبق السمة `cfg(test)` عليه. نفصل أيضًا تنفيذ panic handler إلى دالة `test_panic_handler` public، حتى تكون متاحة لـ executables أيضًا.

بما أن `lib.rs` يُختبر بشكل مستقل عن `main.rs`، نحتاج إلى إضافة entry point `_start` و panic handler عندما تُجمّع المكتبة في وضع الاختبار. باستخدام crate attribute [`cfg_attr`]، نفعل شرطيًا السمة `no_main` في هذه الحالة.

[`cfg_attr`]: https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute

ننقل أيضًا enum `QemuExitCode` ودالة `exit_qemu` ونجعلها public:

```rust
// in src/lib.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

الآن يمكن لـ executables و integration tests استيراد هذه الدوال من المكتبة ولا تحتاج إلى تحديد تطبيقاتها الخاصة. لجعل `println` و `serial_println` متاحين أيضًا، ننقل إعلانات modules:

```rust
// in src/lib.rs

pub mod serial;
pub mod vga_buffer;
```

نجعل modules public لجعلها قابلة للاستخدام خارج مكتبتنا. هذا مطلوب أيضًا لجعل macros `println` و `serial_println` قابلة للاستخدام لأنها تستخدم دوال `_print` للـ modules.

الآن يمكننا تحديث `main.rs` لاستخدام المكتبة:

```rust
// in src/main.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use blog_os::println;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

المكتبة قابلة للاستخدام مثل أي external crate عادية. تسمى `blog_os`، مثل crate الخاص بنا. يستخدم الكود أعلاه دالة `blog_os::test_runner` في السمة `test_runner` ودالة `blog_os::test_panic_handler` في panic handler `cfg(test)` الخاص بنا. يستورد أيضًا macro `println` لجعلها متاحة لدوال `_start` و `panic` الخاصة بنا.

في هذه المرحلة، يجب أن يعمل `cargo run` و `cargo test` مرة أخرى. بالطبع، `cargo test` لا يزال يُ loop بشكل غير محدود (يمكنك الخروج بـ `ctrl+c`). لنصلح ذلك باستخدام دوال المكتبة المطلوبة في integration test.

### إكمال اختبار التكامل

مثل `src/main.rs`، يمكن لـ executable `tests/basic_boot.rs` استيراد أنواع من مكتبتنا الجديدة. هذا يسمح لنا باستيراد المكونات المفقودة لإكمال اختبارنا:

```rust
// in tests/basic_boot.rs

#![test_runner(blog_os::test_runner)]

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

بدلاً من إعادة تنفيذ test runner، نستخدم دالة `test_runner` من مكتبتنا بتغيير السمة `#![test_runner(crate::test_runner)]` إلى `#![test_runner(blog_os::test_runner)]`. ثم لم نعد بحاجة إلى دالة `test_runner` الـ stub في `basic_boot.rs`، لذلك يمكننا إزالتها. لمعالج `panic`، نستدعي دالة `blog_os::test_panic_handler` كما فعلنا في `main.rs`.

الآن `cargo test` يخرج بشكل طبيعي مرة أخرى. عندما تشغله، سترى أنه يبني ويشغل الاختبارات لـ `lib.rs` و `main.rs` و `basic_boot.rs` بشكل منفصل واحدًا تلو الآخر. بالنسبة لـ `main.rs` و `basic_boot` integration tests، يُبلّغ "Running 0 tests" لأن هذه الملفات ليس لديها أي دوال موسومة بـ `#[test_case]`.

يمكننا الآن إضافة اختبارات إلى `basic_boot.rs`. على سبيل المثال، يمكننا اختبار أن `println` يعمل دون panic، كما فعلنا في اختبارات VGA buffer:

```rust
// in tests/basic_boot.rs

use blog_os::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}
```

عندما نشغّل `cargo test` الآن، نرى أنه يجد وينفذ دالة الاختبار.

قد يبدو الاختبار غير مفيد بعض الشيء الآن لأنه متطابق تقريبًا مع أحد اختبارات VGA buffer. ومع ذلك، في المستقبل، قد تنمو دوال `_start` في `main.rs` و `lib.rs` وتستدعي routines تهيئة مختلفة قبل تشغيل `test_main`، بحيث يُنفّذ الاختباران في بيئات مختلفة جدًا.

باختبار `println` في بيئة `basic_boot` دون استدعاء routines تهيئة في `_start`، يمكننا التأكد من أن `println` يعمل مباشرة بعد الإقلاع. هذا مهم لأننا نعتمد عليه، على سبيل المثال، لطباعة رسائل panic.

### الاختبارات المستقبلية

قوة integration tests هي أنها تُعامل كـ executables completely منفصلة. هذا يمنحها تحكمًا كاملًا في البيئة، مما يجعل من الممكن اختبار أن الكود يتفاعل بشكل صحيح مع وحدة المعالجة المركزية أو أجهزة الأجهزة.

اختبار `basic_boot` هو مثال بسيط جدًا على integration test. في المستقبل، سيصبح نواتنا أكثر غنى بالميزات ويتفاعل مع الأجهزة بطرق مختلفة. بإضافة integration tests، يمكننا ضمان أن هذه التفاعلات تعمل (وتستمر في العمل) كما هو متوقع. بعض الأفكار لاختبارات مستقبلية محتملة:

- **CPU Exceptions**: عندما ينفذ الكود عمليات غير صالحة (مثل القسمة على صفر)، يرمي وحدة المعالجة المركزية exception. يمكن للنواة تسجيل دوال معالجة لهذه exceptions. يمكن لـ integration test التحقق من أن معالج exception الصحيح يُستدعى عند حدوث CPU exception أو أن التنفيذ يستمر بشكل صحيح بعد exception قابل للحل.
- **Page Tables**: تحدد page tables أي مناطق ذاكرة صالحة وقابلة للوصول. بتعديل page tables، يمكن تخصيص مناطق ذاكرة جديدة، على سبيل المثال عند إطلاق برامج. يمكن لـ integration test تعديل page tables في دالة `_start` والتحقق من أن التعديلات لها التأثيرات المطلوبة في دوال `#[test_case]`.
- **Userspace Programs**: برامج Userspace هي برامج لها وصول محدود إلى موارد النظام. على سبيل المثال، ليس لديها وصول إلى هياكل بيانات النواة أو ذاكرة البرامج الأخرى. يمكن لـ integration test إطلاق برامج userspace تُنفّذ عمليات محظورة والتحقق من أن النواة تمنعها جميعًا.

كما يمكنك التخيل، هناك العديد من الاختبارات الأخرى الممكنة. بإضافة مثل هذه الاختبارات، يمكننا ضمان عدم كسرها عن طريق الخطأ عند إضافة ميزات جديدة إلى نواتنا أو refactor الكود. هذا مهم بشكل خاص عندما تصبح نواتنا أكبر وأكثر تعقيدًا.

### الاختبارات التي يجب أن تُصيب بالـ panic

يدعم test framework في مكتبة القياسية [سمة `#[should_panic]`][should_panic] التي تسمح بإنشاء اختبارات يجب أن تفشل. هذا مفيد، على سبيل المثال، للتحقق من أن دالة تفشل عند تمرير وسيطة غير صالحة. لسوء الحظ، هذه السمة غير مدعومة في crates `#[no_std]` لأنها تتطلب دعمًا من مكتبة القياسية.

[should_panic]: https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html#testing-panics

بينما لا نستطيع استخدام السمة `#[should_panic]` في نواتنا، يمكننا الحصول على سلوك مماثل بإنشاء integration test يخرج بـ exit code نجاح من panic handler. لنبدأ بإنشاء مثل هذا الاختبار باسم `should_panic`:

```rust
// in tests/should_panic.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use blog_os::{QemuExitCode, exit_qemu, serial_println};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

هذا الاختبار لا يزال غير مكتمل لأنه لا يحدد دالة `_start` أو أيًا من سمات custom test runner بعد. لنضف الأجزاء المفقودة:

```rust
// in tests/should_panic.rs

#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test();
        serial_println!("[test did not panic]");
        exit_qemu(QemuExitCode::Failed);
    }
    exit_qemu(QemuExitCode::Success);
}
```

بدلاً من إعادة استخدام `test_runner` من `lib.rs`، يحدد الاختبار دالة `test_runner` خاصة به تخرج بـ exit code فشل عندما يعود اختبار دون panic (نريد اختباراتنا أن تُ panic). إذا لم تُحدد دالة اختبار، يخرج الـ runner بـ exit code نجاح. بما أن الـ runner يخرج دائمًا بعد تشغيل اختبار واحد، لا معنى لتحديد أكثر من دالة `#[test_case]`.

الآن يمكننا إنشاء اختبار يجب أن يفشل:

```rust
// in tests/should_panic.rs

use blog_os::serial_print;

#[test_case]
fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}
```

الاختبار يستخدم `assert_eq` للادعاء بأن `0` و `1` متساويان. بالطبع، هذا يفشل، لذلك يُ panic اختبارنا كما هو مرغوب. لاحظ أننا نحتاج إلى طباعة اسم الدالة يدويًا باستخدام `serial_print!` هنا لأننا لا نستخدم trait `Testable`.

عندما نشغّل الاختبار عبر `cargo test --test should_panic` نرى أنه ناجح لأن الاختبار يُ panic كما هو متوقع. عندما نعلّق على assertion ونشغّل الاختبار مرة أخرى، نرى أنه يفشل فعليًا برسالة _"test did not panic"_.

عيب كبير لهذا النهج هو أنه يعمل فقط لدالة اختبار واحدة. مع عدة دوال `#[test_case]`، تُنفّذ الدالة الأولى فقط لأن التنفيذ لا يمكن أن يستمر بعد استدعاء panic handler. لا أعرف حاليًا طريقة جيدة لحل هذه المشكلة، لذلك أخبرني إذا كانت لديك فكرة!

### الاختبارات بدون harness

لـ integration tests التي لديها دالة اختبار واحدة فقط (مثل اختبار `should_panic`)، لا يكون الـ test runner مطلوبًا فعليًا. لحالات كهذه، يمكننا تعطيل test runner completely وتشغيل اختبارنا مباشرة في دالة `_start`.

المفتاح لذلك هو تعطيل علم `harness` للاختبار في `Cargo.toml`، الذي يحدد ما إذا كان test runner يُستخدم لـ integration test. عندما يُعيّن إلى `false`، يُعطل كلاً من test runner الافتراضي وميزة custom test runner، بحيث يُعامل الاختبار كـ executable عادي.

لنعطل علم `harness` لاختبار `should_panic`:

```toml
# in Cargo.toml

[[test]]
name = "should_panic"
harness = false
```

الآن نبسّط بشكل كبير اختبار `should_panic` بإزالة الكود المتعلق بـ `test_runner`. النتيجة تبدو كالتالي:

```rust
// in tests/should_panic.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    loop{}
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

نستدعي الآن دالة `should_fail` مباشرة من دالة `_start` ونخرج بـ exit code فشل إذا عادت. عندما نشغّل `cargo test --test should_panic` الآن، نرى أن الاختبار يتصرف بالضبط كما كان من قبل.

بالإضافة إلى إنشاء اختبارات `should_panic`، يمكن تعطيل السمة `harness` أن يكون مفيدًا أيضًا لـ integration tests المعقدة، على سبيل المثال، عندما يكون لدوال الاختبار الفردية side effects وتحتاج إلى أن تُنفّذ بترتيب محدد.

## الخلاصة

الاختبار تقنية مفيدة جدًا لضمان أن مكونات معينة لها السلوك المرغوب. حتى لو لم تكن قادرة على إظهار عدم وجود bugs، فهي لا تزال أداة مفيدة للعثور عليها و بشكل خاص لتجنب الانحدارات.

شرح هذا المقال كيفية إعداد test framework لنواتنا Rust. استخدمنا ميزة custom test frameworks في Rust لتنفيذ دعم لسمة `#[test_case]` بسيطة في بيئة bare metal الخاصة بنا. باستخدام جهاز `isa-debug-exit` في QEMU، يمكن لـ test runner الخروج من QEMU بعد تشغيل الاختبارات وreport حالة الاختبار. لطباعة رسائل الخطأ إلى console بدلاً من VGA buffer، أنشأنا driver أساسي لـ serial port.

بعد إنشاء بعض الاختبارات لـ macro `println`، استكشفنا integration tests في النصف الثاني من المقال. تعلمنا أنها تعيش في دليل `tests` وتُعامل كـ executables completely منفصلة. لإعطائها الوصول إلى دالة `exit_qemu` و macro `serial_println`، نقلنا معظم كودنا إلى مكتبة يمكن استيرادها من جميع executables و integration tests. بما أن integration tests تعمل في بيئة منفصلة خاصة بها، تجعل من الممكن اختبار التفاعلات مع الأجهزة أو إنشاء اختبارات يجب أن تُ panic.

الآن لدينا test framework يعمل في بيئة واقعية داخل QEMU. بإنشاء المزيد من الاختبارات في المقالات المستقبلية، يمكننا الحفاظ على نواتنا قابلة للصيانة عندما تصبح أكثر تعقيدًا.

## ما التالي؟

في المقال التالي، سنستكشف _CPU exceptions_. هذه exceptions تُرمى من قبل وحدة المعالجة المركزية عندما يحدث شيء غير قانوني، مثل القسمة على صفر أو الوصول إلى صفحة ذاكرة غير مُعيّنة (ما يسمى "page fault"). القدرة على catch وفحص هذه exceptions مهمة جدًا لتصحيح الأخطاء المستقبلية. معالجة exceptions مشابهة جدًا لمعالجة hardware interrupts، المطلوبة لدعم لوحة المفاتيح.
