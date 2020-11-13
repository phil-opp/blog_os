+++
title = "تست کردن"
weight = 4
path = "fa/testing"
date = 2019-04-27

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "d007af4811469b974f7abb988dd9c9d1373b55f0"
# GitHub usernames of the people that translated this post
translators = ["hamidrezakp", "MHBahrampour"]
rtl = true
+++

این پست به بررسی تست‌های واحد (ترجمه: unit) و یکپارچه (ترجمه: integration) در فایل‌های اجرایی ‌`no_std` می‌پردازد. ما از پشتیبانی Rust برای فریم‌ورک تست‌های سفارشی استفاده می‌کنیم تا توابع تست را درون کرنل‌مان اجرا کنیم. برای گزارش کردن نتایج خارج از QEMU، از ویژگی‌های مختلف QEMU و ابزار `bootimage` استفاده می‌کنیم.

<!-- more -->

این بلاگ بصورت آزاد روی [گیت‌هاب] توسعه داده شده است. اگر شما مشکل یا سوالی دارید، لطفاً آن‌جا یک ایشو باز کنید. شما همچنین می‌توانید [در زیر] این پست کامنت بگذارید. منبع کد کامل این پست را می‌توانید در بِرَنچ [`post-04`][post branch] پیدا کنید.

[گیت‌هاب]: https://github.com/phil-opp/blog_os
[در زیر]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-04

<!-- toc -->

## نیازمندی‌ها

این پست جایگزین (حالا منسوخ شده) پست‌های [_Unit Testing_] و [_Integration Tests_] می‌شود. فرض بر این است که شما پست [_یک کرنل مینیمال با Rust_] را پس از 27-09-2019 دنبال کرده‌اید. اساساً نیاز است که شما یک فایل `.cargo/config.toml` داشته باشید که [یک هدف پیشفرض مشخص می‌کند] و [یک اجرا کننده قابل اجرا تعریف می‌کند].

[_Unit Testing_]: @/second-edition/posts/deprecated/04-unit-testing/index.md
[_Integration Tests_]: @/second-edition/posts/deprecated/05-integration-tests/index.md
[_یک کرنل مینیمال با Rust_]: @/second-edition/posts/02-minimal-rust-kernel/index.md
[یک هدف پیشفرض مشخص می‌کند]: @/second-edition/posts/02-minimal-rust-kernel/index.md#set-a-default-target
[یک اجرا کننده قابل اجرا تعریف می‌کند]: @/second-edition/posts/02-minimal-rust-kernel/index.md#using-cargo-run

## تست کردن در Rust

زبان Rust یک [فریم‌ورک تست توکار] دارد که قادر به اجرای تست‌های واحد بدون نیاز به تنظیم هر چیزی است. فقط کافی است تابعی ایجاد کنید که برخی نتایج را از طریق اَسرشن‌ها (کلمه: assertions) بررسی کند و صفت `#[test]` را به هدر تابع (ترجمه: function header) اضافه کنید. سپس `cargo test` به طور خودکار تمام تابع‌های تست کریت شما را پیدا و اجرا می‌کند.

[فریم‌ورک تست توکار]: https://doc.rust-lang.org/book/second-edition/ch11-00-testing.html

متأسفانه برای برنامه‌های `no_std` مانند هسته ما کمی پیچیده‌تر است. مسئله این است که فریم‌ورک تست Rust به طور ضمنی از کتابخانه [`test`] داخلی استفاده می‌کند که به کتابخانه استاندارد وابسته‌ است. این بدان معناست که ما نمی‌توانیم از فریم‌ورک تست پیشفرض برای هسته `#[no_std]` خود استفاده کنیم.

[`test`]: https://doc.rust-lang.org/test/index.html

وقتی می‌خواهیم `cargo test` را در پروژه خود اجرا کنیم، چنین چیزی می‌بینیم:

```
> cargo test
   Compiling blog_os v0.1.0 (/…/blog_os)
error[E0463]: can't find crate for `test`
```

از آن‌جایی که کریت `test` به کتابخانه استاندارد وابسته است، برای هدف bare metal ما در دسترس نیست. در حالی که استفاده از کریت `test` در یک `#[no_std]` [امکان پذیر است][utest]، اما بسیار ناپایدار بوده و به برخی هک‌ها مانند تعریف مجدد ماکرو `panic` نیاز دارد.

[utest]: https://github.com/japaric/utest

### فریم‌ورک تست سفارشی

خوشبختانه، Rust از جایگزین کردن فریم‌ورک تست پیشفرض از طریق ویژگی [`custom_test_frameworks`] ناپایدار پشتیبانی می‌کند. این ویژگی به کتابخانه خارجی احتیاج ندارد و بنابراین در محیط‌های `#[no_std]` نیز کار می‌کند. این کار با جمع آوری تمام توابع دارای صفت `#[test_case]` و سپس فراخوانی یک تابع اجرا کننده مشخص شده توسط کاربر و با لیست تست‌ها به عنوان آرگومان کار می‌کند. بنابراین حداکثر کنترل فرآیند تست را به ما می‌دهد.

[`custom_test_frameworks`]: https://doc.rust-lang.org/unstable-book/language-features/custom-test-frameworks.html

نقطه ضعف آن در مقایسه با فریم‌ورک تست پیشفرض این است که بسیاری از ویژگی‌های پیشرفته مانند [تست‌های `should_panic`] در دسترس نیست. در عوض، تهیه این ویژگی‌ها در صورت نیاز به پیاده‌سازی ما بستگی دارد. این برای ما ایده آل است، زیرا ما یک محیط اجرای بسیار ویژه داریم که پیاده سازی پیشفرض چنین ویژگی‌های پیشرفته‌ای احتمالاً کارساز نخواهد بود. به عنوان مثال‌، صفت `#[should_panic]` متکی به stack unwinding برای گرفتن پنیک‌ها (کلمه: panics) است، که ما آن را برای هسته خود غیرفعال کردیم.

[تست‌های `should_panic`]: https://doc.rust-lang.org/book/ch11-01-writing-tests.html#checking-for-panics-with-should_panic

برای اجرای یک فریم‌ورک تست سفارشی برای هسته خود، موارد زیر را به `main.rs` اضافه می‌کنیم:

```rust
// in src/main.rs

#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}
```

اجرا کننده ما فقط یک پیام کوتاه اشکال زدایی را چاپ می‌کند و سپس هر تابع تست درون لیست را فراخوانی می‌کند. نوع آرگومان `&[&dyn Fn()]` یک [_slice_] از [_trait object_] است که آن هم ارجاعی از تِرِیت (کلمه: trait) [_Fn()_] می‌باشد. در اصل لیستی از ارجاع به انواع است که می‌توان آن‌ها را مانند یک تابع صدا زد. از آن‌جایی که این تابع برای اجراهایی که تست نباشند بی فایده است، از ویژگی `#[cfg(test)]` استفاده می‌کنیم تا آن را فقط برای تست کردن در اضافه کنیم.

[_slice_]: https://doc.rust-lang.org/std/primitive.slice.html
[_trait object_]: https://doc.rust-lang.org/1.30.0/book/first-edition/trait-objects.html
[_Fn()_]: https://doc.rust-lang.org/std/ops/trait.Fn.html

حال وقتی که `cargo test` را اجرا می‌کنیم، می‌بینیم که الان موفقیت آمیز است (اگر اینطور نیست یادداشت زیر را بخوانید). اگرچه، همچنان “Hello World” را به جای پیام `test_runner` می‌بینیم. دلیلش این است که تابع `_start` هنوز بعنوان نقطه شروع استفاده می‌شود. ویژگی فریم‌ورک تست سفارشی، یک تابع `main` ایجاد می‌کند که `test_runner` را صدا می‌زند، اما این تابع نادیده گرفته می‌شود چرا که ما از ویژگی `#[no_main]` استفاده می‌کنیم و نقطه شروع خودمان را ایجاد کردیم.

<div class = "warning">

**یادداشت:** درحال حاضر یک باگ در کارگو وجود دارد که در برخی موارد وقتی از `cargo test` استفاده می‌کنیم ما را به سمت خطای “duplicate lang item” می‌برد. زمانی رخ می‌دهد که شما `panic = "abort"` را برای یک پروفایل در `Cargo.toml` تنظیم کرده‌اید. سعی کنید آن را حذف کنید، سپس `cargo test` باید به درستی کار کند. برای اطلاعات بیشتر [ایشوی کارگو](https://github.com/rust-lang/cargo/issues/7359) را ببینید.

</div>

برای حل کردن این مشکل، ما ابتدا نیاز داریم که نام تابع تولید شده را از طریق صفت `reexport_test_harness_main` به چیزی غیر از `main` تغییر دهیم. سپس می‌توانیم تابع تغییر نام داده شده را از تابع `_start` صدا بزنیم:

```rust
// in src/main.rs

#![reexport_test_harness_main = "test_main"]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}
```

ما نام فریم‌ورک تست تابع شروع را `test_main` گذاشتیم و آن را درون `_start` صدا زدیم. از [conditional compilation] برای اضافه کردن فراخوانی `test_main` فقط در زمینه‌های تست استفاده می‌کنیم زیرا تابع روی یک اجرای عادی تولید نشده است.

زمانی که `cargo test` را اجرا می‌کنیم، می‌بینیم که پیام "Running 0 tests" از `test_runner` روی صفحه نمایش داده می‌شود. حال ما آماده‌ایم تا اولین تابع تست را بسازیم:

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}
```

حال وقتی `cargo test` را اجرا می‌کنیم، خروجی زیر را می‌بینیم:

![QEMU printing "Hello World!", "Running 1 tests", and "trivial assertion... [ok]"](qemu-test-runner-output.png)

حالا بخش `tests` ارسال شده به تابع `test_runner` شامل یک ارجاع به تابع `trivial_assertion` است. از خروجی `trivial assertion... [ok]` روی صفحه می‌فهمیم که تست مورد نظر فراخوانی شده و موفقیت آمیز بوده است.

پس از اجرای تست‌ها، `test_runner` به تابع `test_main` برمی‌گردد، که به نوبه خود به تابع `_start` برمی‌گردد. در انتهای `_start`، یک حلقه بی‌پایان ایجاد می‌کنیم زیرا تابع شروع اجازه برگردادن چیزی را ندارد (یعنی بدون خروجی است). این یک مشکل است، زیرا می‌خواهیم `cargo test` پس از اجرای تمام تست‌ها به کار خود پایان دهد.

## خروج از QEMU

در حال حاضر ما یک حلقه بی‌پایان در انتهای تابع `"_start"` داریم و باید QEMU را به صورت دستی در هر مرحله از `cargo test` ببندیم. این جای تأسف دارد زیرا ما همچنین می‌خواهیم `cargo test` را در اسکریپت‌ها بدون تعامل کاربر اجرا کنیم. یک راه حل خوب می‌تواند اجرای یک روش مناسب برای خاموش کردن سیستم عامل باشد. متأسفانه این کار نسبتاً پیچیده است، زیرا نیاز به پشتیبانی از استاندارد [APM] یا [ACPI] مدیریت توان دارد.

[APM]: https://wiki.osdev.org/APM
[ACPI]: https://wiki.osdev.org/ACPI

خوشبختانه، یک دریچه فرار وجود دارد: QEMU از یک دستگاه خاص `isa-debug-exit` پشتیبانی می‌کند، که راهی آسان برای خروج از سیستم QEMU از سیستم مهمان فراهم می‌کند. برای فعال کردن آن، باید یک آرگومان `-device` را به QEMU منتقل کنیم. ما می‌توانیم این کار را با اضافه کردن کلید پیکربندی `pack.metadata.bootimage.test-args` در` Cargo.toml` انجام دهیم:

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
```

`bootimage runner` برای کلیه تست‌های اجرایی ` test-args` را به دستور پیش فرض QEMU اضافه می کند. برای یک `cargo run` عادی، آرگومان‌ها نادیده گرفته می‌شوند.

همراه با نام دستگاه (`isa-debug-exit`)، دو پارامتر `iobase` و `iosize` را عبور می‌دهیم که _پورت I/O_ را مشخص می‌کند و هسته از طریق آن می‌تواند به دستگاه دسترسی داشته باشد.

### پورت‌های I/O

برای برقراری ارتباط بین پردازنده و سخت افزار جانبی در x86، دو رویکرد مختلف وجود دارد،**memory-mapped I/O** و **port-mapped I/O**. ما قبلاً برای دسترسی به [بافر متن VGA] از طریق آدرس حافظه `0xb8000` از memory-mapped I/O استفاده کرده‌ایم. این آدرس به RAM مپ (ترسیم) نشده است، بلکه به برخی از حافظه‌های دستگاه VGA مپ شده است.

[بافر متن VGA]: @/second-edition/posts/03-vga-text-buffer/index.md

در مقابل، port-mapped I/O از یک گذرگاه I/O جداگانه برای ارتباط استفاده می‌کند. هر قسمت جانبی متصل دارای یک یا چند شماره پورت است. برای برقراری ارتباط با چنین پورت I/O، دستورالعمل‌های CPU خاصی وجود دارد که `in` و `out` نامیده می‌شوند، که یک عدد پورت و یک بایت داده را می‌گیرند (همچنین این دستورات تغییراتی دارند که اجازه می دهد یک `u16` یا `u32` ارسال کنید).

دستگاه‌های `isa-debug-exit` از port-mapped I/O استفاده می‌کنند. پارامتر `iobase` مشخص می‌کند که دستگاه باید در کدام آدرس پورت قرار بگیرد (`0xf4` یک پورت [معمولاً استفاده نشده][list of x86 I/O ports] در گذرگاه IO x86 است) و `iosize` اندازه پورت را مشخص می‌کند (`0x04` یعنی چهار بایت).

[list of x86 I/O ports]: https://wiki.osdev.org/I/O_Ports#The_list

### استفاده از دستگاه خروج

عملکرد دستگاه `isa-debug-exit` بسیار ساده است. وقتی یک مقدار به پورت I/O مشخص شده توسط `iobase` نوشته می‌شود، باعث می شود QEMU با [exit status] خارج شود `(value << 1) | 1`. بنابراین هنگامی که ما `0` را در پورت می‌نویسیم، QEMU با وضعیت خروج `(0 << 1) | 1 = 1` خارج می‌شود و وقتی که ما `1` را در پورت می‌نویسیم با وضعیت خروج `(1 << 1) | 1 = 3` از آن خارج می شود.

[exit status]: https://en.wikipedia.org/wiki/Exit_status

به جای فراخوانی دستی دستورالعمل های اسمبلی `in` و `out`، ما از انتزاعات ارائه شده توسط کریت [`x86_64`] استفاده می‌کنیم. برای افزودن یک وابستگی به آن کریت، آن را به بخش `dependencies` در `Cargo.toml` اضافه می‌کنیم:

[`x86_64`]: https://docs.rs/x86_64/0.12.1/x86_64/

```toml
# in Cargo.toml

[dependencies]
x86_64 = "0.12.1"
```

اکنون می‌توانیم از نوع [`Port`] ارائه شده توسط کریت برای ایجاد عملکرد `exit_qemu` استفاده کنیم:

[`Port`]: https://docs.rs/x86_64/0.12.1/x86_64/instructions/port/struct.Port.html

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

این تابع یک [`Port`] جدید در `0xf4` ایجاد می‌کند، که `iobase` دستگاه `isa-debug-exit` است. سپس کد خروجی عبور داده شده را در پورت می‌نویسد. ما از `u32` استفاده می‌کنیم زیرا `iosize` دستگاه `isa-debug-exit` را به عنوان 4 بایت مشخص کردیم. هر دو عملیات ایمن نیستند، زیرا نوشتن در یک پورت I/O می‌تواند منجر به رفتار خودسرانه شود.

برای تعیین وضعیت خروج، یک ای‌نام (کلمه: enum) `QemuExitCode` ایجاد می کنیم. ایده این است که اگر همه تست‌ها موفقیت آمیز بود، با کد خروج موفقیت (ترجمه: success exit code) خارج شود و در غیر این صورت با کد خروج شکست (ترجمه: failure exit code) خارج شود. enum به عنوان `#[repr(u32)]` علامت گذاری شده است تا هر نوع را با یک عدد صحیح `u32` نشان دهد. برای موفقیت از کد خروجی `0x10` و برای شکست از `0x11` استفاده می‌کنیم. کدهای خروجی واقعی چندان هم مهم نیستند، به شرطی که با کدهای خروجی پیش فرض QEMU مغایرت نداشته باشند. به عنوان مثال، استفاده از کد خروجی `0` برای موفقیت ایده خوبی نیست زیرا پس از تغییر شکل تبدیل به `(0 << 1) | 1 = 1` می‌شود، که کد خروجی پیش فرض است برای زمانی که QEMU نمی‌تواند اجرا شود. بنابراین ما نمی‌توانیم خطای QEMU را از یک تست موفقیت آمیز تشخیص دهیم.

اکنون می توانیم `test_runner` خود را به روز کنیم تا پس از اتمام تست‌ها از QEMU خارج شویم:

```rust
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    /// new
    exit_qemu(QemuExitCode::Success);
}
```

حال وقتی `cargo test` را اجرا می‌کنیم، می‌بینیم که QEMU پس از اجرای تست‌ها بلافاصله بسته می‌شود. مشکل این است که `cargo test` تست را به عنوان شکست تفسیر می‌کند حتی اگر کد خروج `Success` را عبور دهیم:

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

مسئله این است که `cargo test` همه کدهای خطا به غیر از `0` را به عنوان شکست در نظر می‌گیرد.

### کد خروج موفقیت

برای کار در این مورد، `bootimage` یک کلید پیکربندی `test-success-exit-code` ارائه می‌دهد که یک کد خروجی مشخص را به کد خروجی `0` مپ می‌کند:

```toml
[package.metadata.bootimage]
test-args = […]
test-success-exit-code = 33         # (0x10 << 1) | 1
```

با استفاده از این پیکربندی، `bootimage` کد خروج موفقیت ما را به کد خروج 0 مپ می‌کند، به طوری که `cargo test` به درستی مورد موفقیت را تشخیص می‌دهد و تست را شکست خورده به حساب نمی‌آورد.

اجرا کننده تست ما اکنون به طور خودکار QEMU را می‌بندد و نتایج تست را به درستی گزارش می‌کند. ما همچنان می‌بینیم که پنجره QEMU برای مدت بسیار کوتاهی باز است، اما این مدت بسیار کوتاه برای خواندن نتایج کافی نیست. جالب می‌شود اگر بتوانیم نتایج تست را به جای QEMU در کنسول چاپ کنیم، بنابراین پس از خروج از QEMU هنوز می‌توانیم آنها را ببینیم.

## چاپ کردن در کنسول

برای دیدن خروجی تست روی کنسول، باید داده‌ها را از هسته خود به نحوی به سیستم میزبان ارسال کنیم. روش‌های مختلفی برای دستیابی به این هدف وجود دارد، به عنوان مثال با ارسال داده‌ها از طریق رابط شبکه TCP. با این حال، تنظیم پشته شبکه یک کار کاملا پیچیده است، بنابراین ما به جای آن راه حل ساده‌تری را انتخاب خواهیم کرد.

### پورت سریال

یک راه ساده برای ارسال داده‌ها استفاده از [پورت سریال] است، یک استاندارد رابط قدیمی که دیگر در رایانه‌های مدرن یافت نمی‌شود. پیاده‌سازی آن آسان است و QEMU می‌تواند بایت‌های ارسالی از طریق سریال را به خروجی استاندارد میزبان یا یک فایل هدایت کند.

[پورت سریال]: https://en.wikipedia.org/wiki/Serial_port

تراشه‌های پیاده سازی یک رابط سریال [UART] نامیده می‌شوند. در x86 [مدلهای UART زیادی] وجود دارد، اما خوشبختانه تنها تفاوت آنها ویژگی‌های پیشرفته‌ای است که نیازی به آن‌ها نداریم. UART هایِ رایج امروزه همه با [16550 UART] سازگار هستند، بنابراین ما از آن مدل برای فریم‌ورک تست خود استفاده خواهیم کرد.

[UARTs]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter
[مدلهای UART زیادی]: https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter#UART_models
[16550 UART]: https://en.wikipedia.org/wiki/16550_UART

ما از کریت [`uart_16550`] برای شروع اولیه UART و ارسال داده‌ها از طریق پورت سریال استفاده خواهیم کرد. برای افزودن آن به عنوان یک وابستگی، ما `Cargo.toml` و `main.rs` خود را به روز می‌کنیم:

[`uart_16550`]: https://docs.rs/uart_16550

```toml
# in Cargo.toml

[dependencies]
uart_16550 = "0.2.0"
```

کریت `uart_16550` حاوی ساختار `SerialPort` است که نمایانگر ثبات‌های UART است، اما ما هنوز هم باید نمونه‌ای از آن را خودمان بسازیم. برای آن ما یک ماژول `‌serial` جدید با محتوای زیر ایجاد می‌کنیم:

```rust
// in src/main.rs

mod serial;
```

```rust
// in src/serial.rs

use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}
```

مانند [بافر متن VGA] [vga lazy-static]، ما از `lazy_static` و یک spinlock برای ایجاد یک نمونه نویسنده `static` استفاده می‌کنیم. با استفاده از `lazy_static` می‌توان اطمینان حاصل کرد که متد `init` در اولین استفاده دقیقاً یک بار فراخوانی می‌شود.

مانند دستگاه `isa-debug-exit`، UART با استفاده از پورت I/O برنامه نویسی می‌شود. از آنجا که UART پیچیده‌تر است، از چندین پورت I/O برای برنامه نویسی رجیسترهای مختلف دستگاه استفاده می‌کند. تابع ناامن `SerialPort::new` انتظار دارد که آدرس اولین پورت I/O از UART به عنوان آرگومان باشد، که از آن می‌تواند آدرس تمام پورت‌های مورد نیاز را محاسبه کند. ما در حال عبور دادنِ آدرس پورت `0x3F8` هستیم که شماره پورت استاندارد برای اولین رابط سریال است.

[vga lazy-static]: @/second-edition/posts/03-vga-text-buffer/index.md#lazy-statics

برای اینکه پورت سریال به راحتی قابل استفاده باشد، ماکروهای `serial_print!` و `serial_println!` را اضافه می‌کنیم:

```rust
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

پیاده سازی بسیار شبیه به پیاده سازی ماکروهای `print` و` println` است. از آنجا که نوع `SerialPort` تِرِیت [`fmt::Write`] را پیاده سازی می‌کند، نیازی نیست این پیاده سازی را خودمان انجام دهیم.

[`fmt::Write`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html

اکنون می‌توانیم به جای بافر متن VGA در کد تست خود، روی رابط سریال چاپ کنیم:

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

توجه داشته باشید که ماکرو `serial_println` مستقیماً در زیر فضای نام (ترجمه: namespace) ریشه قرار می‌گیرد زیرا ما از صفت `#[macro_export]` استفاده کردیم، بنابراین وارد کردن آن از طریق `use crate::serial::serial_println` کار نمی کند.

### آرگومان‌‌های QEMU

برای دیدن خروجی سریال از QEMU، باید از آرگومان `-serial` برای هدایت خروجی به stdout (خروجی استاندارد) استفاده کنیم:

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio"
]
```

حالا وقتی `cargo test` را اجرا می‌کنیم، خروجی تست را مستقیماً در کنسول مشاهده خواهیم گرد:

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

با این حال، هنگامی که یک تست ناموفق بود، ما همچنان خروجی را داخل QEMU مشاهده می‌کنیم، زیرا panic handler هنوز از `println` استفاده می‌کند. برای شبیه‌سازی این، می‌توانیم assertion درون تست `trivial_assertion` را به `assert_eq!(0, 1)` تغییر دهیم:

![QEMU printing "Hello World!" and "panicked at 'assertion failed: `(left == right)`
    left: `0`, right: `1`', src/main.rs:55:5](qemu-failed-test.png)

می‌بینیم که پیام panic (تلفظ: پَنیک) هنوز در بافر VGA چاپ می‌شود، در حالی که خروجی‌ تست دیگر (منظور تستی می‌باشد که پنیک نکند) در پورت سریال چاپ می‌شود. پیام پنیک کاملاً مفید است، بنابراین دیدن آن در کنسول نیز مفید خواهد بود.

### چاپ کردن پیام خطا هنگام پنیک کردن

برای خروج از QEMU با یک پیام خطا هنگامی که پنیک رخ می‌دهد، می‌توانیم از [conditional compilation] برای استفاده از یک panic handler متفاوت در حالت تست استفاده کنیم:

[conditional compilation]: https://doc.rust-lang.org/1.30.0/book/first-edition/conditional-compilation.html

```rust
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

برای panic handler تستِ خودمان، از `serial_println` به جای `println` استفاده می‌کنیم و سپس با کد خروج خطا از QEMU خارج می‌شویم. توجه داشته باشید که بعد از فراخوانی `exit_qemu` هنوز به یک حلقه بی‌پایان نیاز داریم زیرا کامپایلر نمی‌داند که دستگاه `isa-debug-exit` باعث خروج برنامه می‌شود.

اکنون QEMU برای تست‌های ناموفق نیز خارج شده و یک پیام خطای مفید روی کنسول چاپ می کند:

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

از آن‌جایی که اکنون همه خروجی‌های تست را در کنسول مشاهده می‌کنیم، دیگر نیازی به پنجره QEMU نداریم که برای مدت کوتاهی ظاهر می‌شود. بنابراین می‌توانیم آن را کاملا پنهان کنیم.

### پنهان کردن QEMU

از آنجا که ما نتایج کامل تست را با استفاده از دستگاه `isa-debug-exit` و پورت سریال گزارش می‌کنیم، دیگر نیازی به پنجره QEMU نداریم. ما می‌توانیم آن را با عبور دادن آرگومان `-display none` به QEMU پنهان کنیم:

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-serial", "stdio",
    "-display", "none"
]
```

اکنون QEMU کاملا در پس زمینه اجرا می‌شود و دیگر هیچ پنجره‌ای باز نمی‌شود. این نه تنها کمتر آزار دهنده است، بلکه به فریم‌ورک تست ما این امکان را می‌دهد که در محیط‌های بدون رابط کاربری گرافیکی مانند سرویس‌های CI یا کانکشن‌های [SSH] اجرا شود.

[SSH]: https://en.wikipedia.org/wiki/Secure_Shell

### Timeouts

از آنجا که `cargo test` منتظر می‌ماند تا test runner (ترجمه: اجرا کننده تست) پایان یابد، تستی که هرگز به اتمام نمی‌رسد (چه موفق، چه ناموفق) می‌تواند برای همیشه اجرا کننده تست را مسدود کند. این جای تأسف دارد، اما در عمل مشکل بزرگی نیست زیرا اجتناب از حلقه‌های بی‌پایان به طور معمول آسان است. با این حال، در مورد ما، حلقه‌های بی‌پایان می‌توانند در موقعیت‌های مختلف رخ دهند:

- بوت لودر موفق به بارگیری هسته نمی‌شود، در نتیجه سیستم به طور بی‌وقفه راه اندازی مجدد شود.
- فریم‌ورک BIOS/UEFI قادر به بارگیری بوت لودر نمی‌شود، در نتیجه باز هم باعث راه‌اندازی مجدد بی‌پایان می‌شود.
- وقتی که CPU در انتهای برخی از توابع ما وارد یک `loop {}` (حلقه بی‌پایان) می‌شود، به عنوان مثال به دلیل اینکه دستگاه خروج QEMU به درستی کار نمی‌کند.
- یا وقتی که سخت افزار باعث ریست شدن سیستم می‌شود، به عنوان مثال وقتی یک استثنای پردازنده (ترجمه: CPU exception) گیر نمی‌افتد (در پست بعدی توضیح داده شده است).

از آنجا که حلقه های بی‌پایان در بسیاری از شرایط ممکن است رخ دهد، به طور پیش فرض ابزار `bootimage` برای هر تست ۵ دقیقه زمان تعیین می‌کند. اگر تست در این زمان به پایان نرسد، به عنوان ناموفق علامت گذاری شده و خطای "Timed Out" در کنسول چاپ می شود. این ویژگی تضمین می‌کند که تست‌هایی که در یک حلقه بی‌پایان گیر کرده‌اند، `cargo test` را برای همیشه مسدود نمی‌کنند.

خودتان می‌توانید با افزودن عبارت `loop {}` در تست `trivial_assertion` آن را امتحان کنید. هنگامی که `cargo test` را اجرا می‌کنید، می‌بینید که این تست پس از ۵ دقیقه به پایان رسیده است. مدت زمان مهلت از طریق یک کلید `test-timeout` در Cargo.toml [قابل پیکربندی][bootimage config] است:

[bootimage config]: https://github.com/rust-osdev/bootimage#configuration

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-timeout = 300          # (in seconds)
```

اگر نمی‌خواهید ۵ دقیقه منتظر بمانید تا تست `trivial_assertion` تمام شود، می‌توانید به طور موقت مقدار فوق را کاهش دهید.

### اضافه کردن چاپ خودکار

تست `trivial_assertion` در حال حاضر باید اطلاعات وضعیت خود را با استفاده از `serial_print!`/`serial_println!` چاپ کند:

```rust
#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

افزودن دستی این دستورات چاپی برای هر تستی که می‌نویسیم دست و پا گیر است، بنابراین بیایید `test_runner` خود را به روز کنیم تا به صورت خودکار این پیام‌ها را چاپ کنیم. برای انجام این کار، ما باید یک تریت جدید به نام `Testable` ایجاد کنیم:

```rust
// in src/main.rs

pub trait Testable {
    fn run(&self) -> ();
}
```

این ترفند اکنون پیاده سازی این تریت برای همه انواع `T` است که [`Fn()` trait] را پیاده سازی می‌کنند:

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

ما با اولین چاپِ نام تابع از طریق تابعِ [`any::type_name`]، تابع `run` را پیاده سازی می کنیم. این تابع مستقیماً در کامپایلر پیاده سازی شده و یک رشته توضیح از هر نوع را برمی‌گرداند. برای توابع، نوع آنها نامشان است، بنابراین این دقیقاً همان چیزی است که ما در این مورد می‌خواهیم. کاراکتر `\t` [کاراکتر tab] است، که مقداری ترازبندی‌ به پیام‌های `[ok]` اضافه می‌کند.

[`any::type_name`]: https://doc.rust-lang.org/stable/core/any/fn.type_name.html
[کاراکتر tab]: https://en.wikipedia.org/wiki/Tab_key#Tab_characters

پس از چاپ نام تابع، ما از طریق `self ()` تابع تست را فراخوانی می‌کنیم. این فقط به این دلیل کار می‌کند که ما نیاز داریم که `self` تریت `Fn()` را پیاده سازی کند. بعد از بازگشت تابع تست، ما `[ok]` را چاپ می‌کنیم تا نشان دهد که تابع پنیک نکرده است.

آخرین مرحله به روزرسانی `test_runner` برای استفاده از تریت جدید` Testable` است:

```rust
// in src/main.rs

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run(); // new
    }
    exit_qemu(QemuExitCode::Success);
}
```

تنها دو تغییر رخ داده، نوع آرگومان `tests` از `&[&dyn Fn()]` به `&[&dyn Testable]` است و ما اکنون `test.run()` را به جای `test()` فراخوانی می‌کنیم.

اکنون می‌توانیم عبارات چاپ را از تست `trivial_assertion` حذف کنیم، زیرا آن‌ها اکنون به طور خودکار چاپ می‌شوند:

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
```

خروجی `cargo test` اکنون به این شکل است:

```
Running 1 tests
blog_os::trivial_assertion...	[ok]
```

نام تابع اکنون مسیر کامل به تابع را شامل می‌شود، که زمانی مفید است که توابع تست در ماژول‌های مختلف نام یکسانی دارند. در غیر اینصورت خروجی همانند قبل است، اما دیگر نیازی نیست که به صورت دستی دستورات چاپ را به تست‌های خود اضافه کنیم.

## تست کردن بافر VGA

اکنون که یک فریم‌ورک تستِ کارا داریم، می‌توانیم چند تست برای اجرای بافر VGA خود ایجاد کنیم. ابتدا، ما یک تست بسیار ساده برای تأیید اینکه `println` بدون پنیک کردن کار می‌کند ایجاد می‌کنیم:


```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}
```

این تست فقط چیزی را در بافر VGA چاپ می کند. اگر بدون پنیک تمام شود، به این معنی است که فراخوانی `println` نیز پنیک نکرده است.

برای اطمینان از این‌ که پنیک ایجاد نمی‌شود حتی اگر خطوط زیادی چاپ شده و خطوط از صفحه خارج شوند، می‌توانیم آزمایش دیگری ایجاد کنیم:

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}
```

همچنین می‌توانیم تابع تستی ایجاد کنیم تا تأیید کنیم که خطوط چاپ شده واقعاً روی صفحه ظاهر می شوند:

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

این تابع یک رشته آزمایشی را تعریف می‌کند، آن را با استفاده از `println` چاپ می‌کند و سپس بر روی کاراکترهای صفحه از ` WRITER` ثابت تکرار (iterate) می‌کند، که نشان دهنده بافر متن vga است. از آنجا که `println` در آخرین خط صفحه چاپ می‌شود و سپس بلافاصله یک خط جدید اضافه می‌کند، رشته باید در خط` BUFFER_HEIGHT - 2` ظاهر شود.

با استفاده از [`enumerate`]، تعداد تکرارها را در متغیر `i` حساب می‌کنیم، سپس از آن‌ها برای بارگذاری کاراکتر صفحه مربوط به `c` استفاده می‌کنیم. با مقایسه `ascii_character` از کاراکتر صفحه با `c`، اطمینان حاصل می‌کنیم که هر کاراکتر از این رشته واقعاً در بافر متن vga ظاهر می‌شود.

[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate

همانطور که می‌توانید تصور کنید، ما می‌توانیم توابع تست بیشتری ایجاد کنیم، به عنوان مثال تابعی که تست می‌کند هنگام چاپ خطوط طولانی پنیک ایجاد نمی‌شود و به درستی بسته‌بندی می‌شوند. یا تابعی برای تست این که خطوط جدید، کاراکترهای غیرقابل چاپ (ترجمه: non-printable) و کاراکترهای non-unicode به درستی اداره می‌شوند.

برای بقیه این پست، ما نحوه ایجاد _integration tests_ را برای تست تعامل اجزای مختلف با هم توضیح خواهیم داد.

## تست‌های یکپارچه

قرارداد [تست‌های یکپارچه] در Rust این است که آن‌ها را در یک دایرکتوری `tests` در ریشه پروژه قرار دهید (یعنی در کنار فهرست `src`).  فریم‌ورک تست پیش فرض و فریم‌ورک‌های تست سفارشی به طور خودکار تمام تست‌های موجود در آن فهرست را انتخاب و اجرا می‌کنند.

[تست‌های یکپارچه]: https://doc.rust-lang.org/book/ch11-03-test-organization.html#integration-tests

همه تست‌های یکپارچه، فایل اجرایی خاص خودشان هستند و کاملاً از `main.rs` جدا هستند. این بدان معناست که هر تست باید تابع نقطه شروع خود را مشخص کند. بیایید یک نمونه تست یکپارچه به نام `basic_boot` ایجاد کنیم تا با جزئیات ببینیم که چگونه کار می‌کند:

```rust
// in tests/basic_boot.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

#[no_mangle] // don't mangle the name of this function
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

از آن‌جا که تست‌های یکپارچه فایل‌های اجرایی جداگانه‌ای هستند، ما باید تمام صفت‌های کریت (`no_std`، `no_main`، `test_runner` و غیره) را دوباره تهیه کنیم. ما همچنین باید یک تابع شروع جدید `_start` ایجاد کنیم که تابع نقطه شروع تست `test_main` را فراخوانی می‌کند. ما به هیچ یک از ویژگی‌های `cfg (test)` نیازی نداریم زیرا اجرایی‌های تست یکپارچه هرگز در حالت غیر تست ساخته نمی‌شوند.

ما از ماکرو [ʻunimplemented] استفاده می‌کنیم که همیشه به عنوان یک مکان نگهدار برای تابع `test_runner` پنیک می‌کند و فقط در حلقه رسیدگی کننده `panic` فعلاً `loop` می‌زند. در حالت ایده آل، ما می‌خواهیم این توابع را دقیقاً همانطور که در `main.rs` خود با استفاده از ماکرو` serial_println` و تابع `exit_qemu` پیاده سازی کردیم، پیاده سازی کنیم. مشکل این است که ما به این توابع دسترسی نداریم زیرا تست‌ها کاملاً جدا از اجرایی `main.rs` ساخته شده‌اند.

[`unimplemented`]: https://doc.rust-lang.org/core/macro.unimplemented.html

اگر در این مرحله `cargo test` را انجام دهید، یک حلقه بی‌پایان خواهید گرفت زیرا رسیدگی کننده پنیک دارای حلقه بی‌پایان است. برای خروج از QEMU باید از میانبر صفحه کلید `Ctrl + c` استفاده کنید.

### ساخت یک کتابخانه

برای در دسترس قرار دادن توابع مورد نیاز در تست یکپارچه، باید یک کتابخانه را از `main.rs` جدا کنیم، کتابخانه‌ای که می‌تواند توسط کریت‌های دیگر و تست‌های یکپارچه مورد استفاده قرار بگیرد. برای این کار، یک فایل جدید `src/lib.rs` ایجاد می‌کنیم:

```rust
// src/lib.rs

#![no_std]

```

مانند `main.rs` ،`lib.rs` یک فایل خاص است که به طور خودکار توسط کارگو شناسایی می‌شود. کتابخانه یک واحد تلفیقی جداگانه است، بنابراین باید ویژگی `#![no_std]` را دوباره مشخص کنیم.

برای اینکه کتابخانه‌مان با `cargo test` کار کند، باید توابع و صفت‌های تست را نیز اضافه کنیم:
To make our library work with `cargo test`, we need to also add the test functions and attributes:

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
#[no_mangle]
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

برای اینکه `test_runner` را در دسترس ‌تست‌های یکپارچه و فایل‌های اجرایی قرار دهیم، صفت `cfg(test)` را روی آن اعمال نمی‌کنیم و عمومی نمی‌کنیم. ما همچنین پیاده سازی رسیدگی کننده پنیک خود را به یک تابع عمومی `test_panic_handler` تبدیل می‌کنیم، به طوری که برای اجرایی‌ها نیز در دسترس باشد.

از آن‌جا که `lib.rs` به طور مستقل از` main.rs` ما تست می‌شود، هنگام کامپایل کتابخانه در حالت تست، باید یک نقطه شروع `_start` و یک رسیدگی کننده پنیک اضافه کنیم. با استفاده از صفت کریت [`cfg_attr`]، در این حالت ویژگی`no_main` را به طور مشروط فعال می‌کنیم.

[`cfg_attr`]: https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute

ما همچنین ای‌نام `QemuExitCode` و تابع `exit_qemu` را عمومی می‌کنیم:

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

اکنون فایل‌های اجرایی و تست‌های یکپارچه می‌توانند این توابع را از کتابخانه وارد کنند و نیازی به تعریف پیاده سازی‌های خود ندارند. برای در دسترس قرار دادن `println` و `serial_println`، اعلان ماژول‌ها را نیز منتقل می‌کنیم:

```rust
// in src/lib.rs

pub mod serial;
pub mod vga_buffer;
```

ما ماژول‌ها را عمومی می‌کنیم تا از خارج از کتابخانه قابل استفاده باشند. این امر همچنین برای استفاده از ماکروهای `println` و `serial_println` مورد نیاز است، زیرا آنها از توابع `_print` ماژول‌ها استفاده می‌کنند.


اکنون می توانیم `main.rs` خود را برای استفاده از کتابخانه به روز کنیم:

```rust
// src/main.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use blog_os::println;

#[no_mangle]
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

کتابخانه مانند یک کریت خارجی معمولی قابل استفاده است. و مانند کریت (که در مورد ما کریت `blog_os` است) فراخوانی می‌شود. کد فوق از تابع `blog_os :: test_runner` در صفت `test_runner` و تابع `blog_os :: test_panic_handler` در رسیدگی کننده پنیک `cfg(test)` استفاده می‌کند. همچنین ماکرو `println` را وارد می‌کند تا در اختیار توابع `_start` و `panic` قرار گیرد.

در این مرحله، `cargo run` و `cargo test` باید دوباره کار کنند. البته، `cargo test` هنوز هم در یک حلقه بی‌پایان گیر می‌کند (با `ctrl + c` می‌توانید خارج شوید). بیایید با استفاده از توابع مورد نیاز کتابخانه در تست یکپارچه این مشکل را برطرف کنیم.

### تمام کردن تست یکپارچه

مانند `src/main.rs`، اجرایی` test/basic_boot.rs` می‌تواند انواع مختلفی را از کتابخانه جدید ما وارد کند. که این امکان را به ما می‌دهد تا اجزای گمشده را برای تکمیل آزمایش وارد کنیم.

```rust
// in tests/basic_boot.rs

#![test_runner(blog_os::test_runner)]

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

ما به جای پیاده سازی مجدد اجرا کننده تست، از تابع `test_runner` در کتابخانه خود استفاده می‌کنیم. برای رسیدگی کننده `panic`، ما تابع `blog_os::test_panic_handler` را مانند آن‌چه در `main.rs` انجام دادیم، فراخوانی می‌کنیم.

اکنون `cargo test` مجدداً به طور معمول وجود دارد. وقتی آن را اجرا می‌کنید ، می‌بینید که تست‌های `lib.rs`، `main.rs` و `basic_boot.rs` ما را به طور جداگانه و یکی پس از دیگری ایجاد و اجرا می‌کند. برای تست‌های یکپارچه `main.rs` و `basic_boot`، متن "Running 0 tests" را نشان می‌دهد زیرا این فایل‌ها هیچ تابعی با حاشیه نویسی `#[test_case]` ندارد.

اکنون می‌توانیم تست‌ها را به `basic_boot.rs` خود اضافه کنیم. به عنوان مثال، ما می‌توانیم آزمایش کنیم که `println` بدون پنیک کار می‌کند، مانند آنچه در تست‌های بافر vga انجام دادیم:

```rust
// in tests/basic_boot.rs

use blog_os::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}
```

حال وقتی `cargo test` را اجرا می‌کنیم، می‌بینیم که این تابع تست را پیدا و اجرا می‌کند.

این تست ممکن است در حال حاضر کمی بی‌فایده به نظر برسد، زیرا تقریباً مشابه یکی از تست‌های بافر VGA است. با این حال، در آینده ممکن است توابع `_start` ما از `main.rs` و `lib.rs` رشد کرده و روال‌های اولیه مختلفی را قبل از اجرای تابع `test_main` فراخوانی کنند، به طوری که این دو تست در محیط‌های بسیار مختلف اجرا می‌شوند.

### تست‌های آینده

قدرت تست‌های یکپارچه این است که با آن‌ها به عنوان اجرایی کاملاً جداگانه برخورد می‌شود. این امر به آن‌ها اجازه کنترل کامل بر محیط را می‌دهد، و امکان تست کردن این که کد به درستی با CPU یا دستگاه‌های سخت‌افزاری ارتباط دارد را به ما می‌دهد.

تست `basic_boot` ما یک مثال بسیار ساده برای تست یکپارچه است. در آینده، هسته ما ویژگی‌های بسیار بیشتری پیدا می‌کند و از راه‌های مختلف با سخت افزار ارتباط برقرار می‌کند. با افزودن تست های یکپارچه، می‌توانیم اطمینان حاصل کنیم که این تعاملات مطابق انتظار کار می‌کنند (و به کار خود ادامه می‌دهند). برخی از ایده‌ها برای تست‌های احتمالی در آینده عبارتند از:

- **استثنائات CPU**: هنگامی که این کد عملیات نامعتبری را انجام می‌دهد (به عنوان مثال تقسیم بر صفر)، CPU یک استثنا را ارائه می‌دهد. هسته می‌تواند توابع رسیدگی کننده را برای چنین مواردی ثبت کند. یک تست یکپارچه می‌تواند تأیید کند که در صورت بروز استثنا پردازنده ، رسیدگی کننده استثنای صحیح فراخوانی می‌شود یا اجرای آن پس از استثناهای قابل حل به درستی ادامه دارد.

- **جدول‌های صفحه**: جدول‌های صفحه مشخص می‌کند که کدام مناطق حافظه معتبر و قابل دسترسی هستند. با اصلاح جدول‌های صفحه، می‌توان مناطق حافظه جدیدی را اختصاص داد، به عنوان مثال هنگام راه‌اندازی برنامه‌ها. یک تست یکپارچه می‌تواند برخی از تغییرات جدول‌های صفحه را در تابع `_start` انجام دهد و سپس تأیید کند که این تغییرات در تابع‌های `# [test_case]` اثرات مطلوبی دارند.

- **برنامه‌های فضای کاربر**: برنامه‌های فضای کاربر برنامه‌هایی با دسترسی محدود به منابع سیستم هستند. به عنوان مثال، آنها به ساختار داده‌های هسته یا حافظه برنامه‌های دیگر دسترسی ندارند. یک تست یکپارچه می‌تواند برنامه‌های فضای کاربر را که عملیات‌های ممنوعه را انجام می‌دهند راه‌اندازی کرده و بررسی کند هسته از همه آن‌ها جلوگیری می‌کند.

همانطور که می‌توانید تصور کنید، تست‌های بیشتری امکان پذیر است. با افزودن چنین تست‌هایی، می‌توانیم اطمینان حاصل کنیم که وقتی ویژگی‌های جدیدی به هسته خود اضافه می‌کنیم یا کد خود را دوباره می‌سازیم، آن‌ها را به طور تصادفی خراب نمی‌کنیم. این امر به ویژه هنگامی مهم‌تر می‌شود که هسته ما بزرگتر و پیچیده‌تر شود.

### تست‌هایی که باید پنیک کنند

فریم‌ورک تست کتابخانه استاندارد از [صفت `#[should_panic]`][should_panic] پشتیبانی می‌کند که اجازه می‌دهد تست‌هایی را بسازد که باید ناموفق شوند (باید پنیک کنند). این مفید است، به عنوان مثال برای تأیید پنیک کردن یک تابع هنگام عبور دادن یک آرگومان نامعتبر به آن. متأسفانه این ویژگی در کریت‌های `#[no_std]` پشتیبانی نمی‌شود زیرا به پشتیبانی از کتابخانه استاندارد نیاز دارد.

[should_panic]: https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html#testing-panics

اگرچه نمی‌توانیم از صفت `#[should_panic]` در هسته خود استفاده کنیم، اما می‌توانیم با ایجاد یک تست یکپارچه که با کد خطای موفقیت آمیز از رسیدگی کننده پنیک خارج می‌شود، رفتار مشابهی داشته باشیم. بیایید شروع به ایجاد چنین تستی با نام `should_panic` کنیم:

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

این تست هنوز ناقص است زیرا هنوز تابع `_start` یا هیچ یک از صفت‌های اجرا کننده تست سفارشی را مشخص نکرده. بیایید قسمت‌های گمشده را اضافه کنیم:

```rust
// in tests/should_panic.rs

#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

#[no_mangle]
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

به جای استفاده مجدد از `test_runner` از `lib.rs`، تست تابع `test_runner` خود را تعریف می‌کند که هنگام بازگشت یک تست بدون پنیک با یک کد خروج خطا خارج می‌شود (ما می‌خواهیم تست‌هایمان پنیک داشته باشند). اگر هیچ تابع تستی تعریف نشده باشد، اجرا کننده با کد خطای موفقیت خارج می‌شود. از آن‌جا که اجرا کننده همیشه پس از اجرای یک تست خارج می‌شود، منطقی نیست که بیش از یک تابع `#[test_case]` تعریف شود.

اکنون می‌توانیم یک تست ایجاد کنیم که باید شکست بخورد:

```rust
// in tests/should_panic.rs

use blog_os::serial_print;

#[test_case]
fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}
```

این تست با استفاده از `assert_eq` ادعا (ترجمه: assert) می‌کند که `0` و `1` برابر هستند. این البته ناموفق است، به طوری که تست ما مطابق دلخواه پنیک می‌کند. توجه داشته باشید که ما باید نام تابع را با استفاده از `serial_print!` در اینجا چاپ دستی کنیم زیرا از تریت `Testable` استفاده نمی‌کنیم.

هنگامی که ما تست را از طریق `cargo test --test should_panic` انجام دهیم، می‌بینیم که موفقیت آمیز است زیرا تست مطابق انتظار پنیک کرد. وقتی ادعا را کامنت کنیم و تست را دوباره اجرا کنیم، می‌بینیم که با پیام _"test did not panic"_ با شکست مواجه می‌شود.

یک اشکال قابل توجه در این روش این است که این روش فقط برای یک تابع تست کار می‌کند. با چندین تابع `#[test_case]`، فقط اولین تابع اجرا می‌شود زیرا پس این‌که رسیدگی کننده پنیک فراخوانی شد، اجرا تمام می‌شود. من در حال حاضر راه خوبی برای حل این مشکل نمی‌دانم، بنابراین اگر ایده‌ای دارید به من اطلاع دهید!

### تست های بدون مهار

برای تست‌های یکپارچه که فقط یک تابع تست دارند (مانند تست `should_panic` ما)، اجرا کننده تست مورد نیاز نیست. برای مواردی از این دست، ما می‌توانیم اجرا کننده تست را به طور کامل غیرفعال کنیم و تست خود را مستقیماً در تابع `_start` اجرا کنیم.

کلید این کار غیرفعال کردن پرچم `harness` برای تست در` Cargo.toml` است، که مشخص می‌کند آیا از یک اجرا کننده تست برای تست یکپارچه استفاده می‌شود. وقتی روی `false` تنظیم شود، هر دو اجرا ککنده تست پیش فرض و سفارشی غیرفعال می‌شوند، بنابراین با تست مانند یک اجرای معمولی رفتار می‌شود.

بیایید پرچم `harness` را برای تست `should_panic` خود غیرفعال کنیم:

```toml
# in Cargo.toml

[[test]]
name = "should_panic"
harness = false
```

اکنون ما با حذف کد مربوط به آاجرا کننده تست، تست `should_panic` خود را بسیار ساده کردیم. نتیجه به این شکل است:

```rust
// in tests/should_panic.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};

#[no_mangle]
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

اکنون تابع `should_fail` را مستقیماً از تابع `_start` خود فراخوانی می‌کنیم و در صورت بازگشت با کد خروج شکست خارج می‌شویم. اکنون وقتی `cargo test --test should_panic` را اجرا می‌کنیم، می‌بینیم که تست دقیقاً مانند قبل عمل می‌کند.

غیر از ایجاد تست‌های `should_panic`، غیرفعال کردن صفت `harness` همچنین می‌تواند برای تست‌های یکپارچه پیچیده مفید باشد، به عنوان مثال هنگامی که تابع‌های منفرد دارای عوارض جانبی هستند و باید به ترتیب مشخصی اجرا شوند.

## خلاصه

تست کردن یک تکنیک بسیار مفید است تا اطمینان حاصل شود که اجزای خاصی رفتار مطلوبی دارند. حتی اگر آن‌ها نتوانند فقدان اشکالات را نشان دهند، آن‌ها هنوز هم یک ابزار مفید برای یافتن آن‌ها و به ویژه برای جلوگیری از دوباره کاری و پسرفت هستند.

در این پست نحوه تنظیم فریم‌ورک تست برای هسته Rust ما توضیح داده شده است. ما از ویژگی فریم‌ورک تست سفارشی Rust برای پیاده سازی پشتیبانی از یک صفت ساده `#[test_case]` در محیط bare-metal خود استفاده کردیم. با استفاده از دستگاه `isa-debug-exit` شبیه‌ساز ماشین و مجازی‌ساز QEMU، اجرا کننده تست ما می‌تواند پس از اجرای تست‌ها از QEMU خارج شده و وضعیت تست را گزارش دهد. برای چاپ پیام‌های خطا به جای بافر VGA در کنسول، یک درایور اساسی برای پورت سریال ایجاد کردیم.

پس از ایجاد چند تست برای ماکرو `println`، در نیمه دوم پست به بررسی تست‌های یکپارچه پرداختیم. ما فهمیدیم که آن‌ها در دایرکتوری `tests` قرار می‌گیرند و به عنوان اجرایی کاملاً مستقل با آن‌ها رفتار می‌شود. برای دسترسی دادن به آن‌ها به تابع `exit_qemu` و ماکرو `serial_println`، بیشتر کدهای خود را به یک کتابخانه منتقل کردیم که می‌تواند توسط همه اجراها و تست‌های یکپارچه وارد (import) شود. از آن‌جا که تست‌های یکپارچه در محیط جداگانه خود اجرا می‌شوند، آن‌ها تست تعاملاتی با سخت‌افزار یا ایجاد تست‌هایی که باید پنیک کنند را امکان پذیر می کنند.

اکنون یک فریم‌ورک تست داریم که در یک محیط واقع گرایانه در داخل QEMU اجرا می‌شود. با ایجاد تست‌های بیشتر در پست‌های بعدی، می‌توانیم هسته خود را هنگامی که پیچیده‌تر شود، نگهداری کنیم.

## مرحله بعدی چیست؟

در پست بعدی، ما _استثنائات CPU_ را بررسی خواهیم کرد. این موارد استثنایی توسط CPU در صورت بروز هرگونه اتفاق غیرقانونی، مانند تقسیم بر صفر یا دسترسی به صفحه حافظه مپ نشده (اصطلاحاً "خطای صفحه")، رخ می‌دهد. امکان کشف و بررسی این موارد استثنایی برای رفع اشکال در خطاهای آینده بسیار مهم است. رسیدگی به استثناها نیز بسیار شبیه رسیدگی به وقفه‌های سخت‌افزاری است، که برای پشتیبانی صفحه کلید مورد نیاز است.
