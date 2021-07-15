+++
title = " یک باینری مستقل Rust"
weight = 1
path = "fa/freestanding-rust-binary"
date = 2018-02-10

[extra]
chapter = "Bare Bones"
# Please update this when updating the translation
translation_based_on_commit = "80136cc0474ae8d2da04f391b5281cfcda068c1a"
# GitHub usernames of the people that translated this post
translators = ["hamidrezakp", "MHBahrampour"]
rtl = true
+++

اولین قدم برای نوشتن سیستم‌عامل، ساخت یک باینری راست (کلمه: Rust) هست که به کتابخانه استاندارد نیازمند نباشد. این باعث می‌شود تا بتوانیم کد راست را بدون سیستم‌عامل زیرین، بر روی سخت افزار [bare metal] اجرا کنیم.

[bare metal]: https://en.wikipedia.org/wiki/Bare_machine

<!-- more -->

این بلاگ بصورت آزاد بر روی [گیت‌هاب] توسعه داده شده. اگر مشکل یا سوالی دارید، لطفاً آن‌جا یک ایشو باز کنید. همچنین می‌توانید [در زیر] این پست کامنت بگذارید. سورس کد کامل این پست را می‌توانید در بِرَنچ [`post-01`][post branch] پیدا کنید.

[گیت‌هاب]: https://github.com/phil-opp/blog_os
[در زیر]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-01

<!-- toc -->

## مقدمه
برای نوشتن هسته سیستم‌عامل، ما به کدی نیاز داریم که به هیچ یک از ویژگی‌های سیستم‌عامل نیازی نداشته باشد. یعنی نمی‌توانیم از نخ‌ها (ترجمه: Threads)، فایل‌ها، حافظه هیپ (کلمه: Heap)، شبکه، اعداد تصادفی، ورودی استاندارد، یا هر ویژگی دیگری که نیاز به انتزاعات سیستم‌عامل یا سخت‌افزار خاصی داشته، استفاده کنیم. منطقی هم به نظر می‌رسد، چون ما سعی داریم سیستم‌عامل و درایور‌های خودمان را بنویسیم.

نداشتن انتزاعات سیستم‌عامل به این معنی هست که نمی‌توانیم از بخش زیادی از [کتابخانه استاندارد راست] استفاده کنیم، اما هنوز بسیاری از ویژگی‌های راست هستند که می‌توانیم از آن‌ها استفاده کنیم. به عنوان مثال، می‌توانیم از [iterator] ها، [closure] ها، [pattern matching]، [option]، [result]، [string formatting] و البته [سیستم ownership] استفاده کنیم. این ویژگی‌ها به ما امکان نوشتن هسته به طور رسا، سطح بالا و بدون نگرانی درباره [رفتار تعریف نشده] و [امنیت حافظه] را میدهند.

[option]: https://doc.rust-lang.org/core/option/
[result]:https://doc.rust-lang.org/core/result/
[کتابخانه استاندارد راست]: https://doc.rust-lang.org/std/
[iterators]: https://doc.rust-lang.org/book/ch13-02-iterators.html
[closures]: https://doc.rust-lang.org/book/ch13-01-closures.html
[pattern matching]: https://doc.rust-lang.org/book/ch06-00-enums.html
[string formatting]: https://doc.rust-lang.org/core/macro.write.html
[سیستم ownership]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
[رفتار تعریف نشده]: https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs
[امنیت حافظه]: https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention

برای ساختن یک هسته سیستم‌عامل به زبان راست، باید فایل اجرایی‌ای بسازیم که بتواند بدون سیستم‌عامل زیرین اجرا بشود. چنین فایل اجرایی، فایل اجرایی مستقل (ترجمه: freestanding) یا فایل اجرایی “bare-metal” نامیده می‌شود.

این پست قدم‌های لازم برای ساخت یک باینری مستقل راست و اینکه چرا این قدم‌ها نیاز هستند را توضیح می‌دهد. اگر علاقه‌ایی به خواندن کل توضیحات ندارید، می‌توانید **[به قسمت خلاصه مراجعه کنید](#summary)**.

## غیر فعال کردن کتابخانه استاندارد
به طور پیش‌فرض تمام کِرِیت‌های راست، از [کتابخانه استاندارد] استفاده می‌کنند(لینک به آن دارند)، که به سیستم‌عامل برای قابلیت‌هایی مثل نخ‌ها، فایل‌ها یا شبکه وابستگی دارد. همچنین به کتابخانه استاندارد زبان سی، `libc` هم وابسطه هست که با سرویس‌های سیستم‌عامل تعامل نزدیکی دارند. از آن‌جا که قصد داریم یک سیستم‌عامل بنویسیم، نمی‌توانیم از هیچ کتابخانه‌ایی که به سیستم‌عامل نیاز داشته باشد استفاده کنیم. بنابراین باید اضافه شدن خودکار کتابخانه استاندارد را از طریق [خاصیت `no_std`] غیر فعال کنیم.

[کتابخانه استاندارد]: https://doc.rust-lang.org/std/
[خاصیت `no_std`]: https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html


با ساخت یک اپلیکیشن جدید کارگو شروع می‌کنیم. ساده‌ترین راه برای انجام این کار از طریق خط فرمان است:

```
cargo new blog_os --bin --edition 2018
```

نام پروژه را `blog_os‍` گذاشتم، اما شما می‌توانید نام دلخواه خود را انتخاب کنید. پرچمِ (ترجمه: Flag) `bin--` مشخص می‌کند که ما می‌خواهیم یک فایل اجرایی ایجاد کنیم (به جای یک کتابخانه) و پرچمِ `edition 2018--` مشخص می‌کند که می‌خواهیم از [ویرایش 2018] زبان راست برای کریت خود استفاده کنیم. وقتی دستور را اجرا می‌کنیم، کارگو ساختار پوشه‌های زیر را برای ما ایجاد می‌کند:

[ویرایش 2018]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

فایل `Cargo.toml` شامل تنظیمات کریت می‌باشد، به عنوان مثال نام کریت، نام نویسنده، شماره [نسخه سمنتیک] و وابستگی‌ها. فایل `src/main.rs` شامل ماژول ریشه برای کریت ما و تابع `main` است. می‌توانید کریت خود را با دستور `cargo build‍` کامپایل کنید و سپس باینری کامپایل شده ‍‍`blog_os` را در زیرپوشه `target/debug` اجرا کنید.

[نسخه سمنتیک]: https://semver.org/

### خاصیت `no_std`

در حال حاظر کریت ما بطور ضمنی به کتابخانه استاندارد لینک دارد. بیایید تا سعی کنیم آن را با اضافه کردن  [خاصیت `no_std`] غیر فعال کنیم:

```rust
// main.rs

#![no_std]

fn main() {
    println!("Hello, world!");
}
```

حالا وقتی سعی می‌کنیم تا بیلد کنیم (با اجرای دستور `cargo build`)، خطای زیر رخ می‌دهد:

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^^
```

دلیل این خطا این هست که [ماکروی `println`]\(ترجمه: macro) جزوی از کتابخانه استاندارد است، که ما دیگر آن را نداریم. بنابراین نمی‌توانیم چیزی را چاپ کنیم. منطقی هست زیرا `println` در [خروجی استاندارد] می‌نویسد، که یک توصیف کننده فایل (ترجمه: File Descriptor) خاص است که توسط سیستم‌عامل ارائه می‌شود.

[ماکروی `println`]: https://doc.rust-lang.org/std/macro.println.html
[خروجی استاندارد]: https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29

پس بیایید قسمت مروبط به چاپ را پاک کرده و این‌ بار با یک تابع main خالی امتحان کنیم:

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

حالا کامپایلر با کمبود یک تابع `#[panic_handler]` و یک _language item_ روبرو است.

## پیاده‌سازی پنیک (کلمه: Panic)

خاصیت `panic_handler` تابعی را تعریف می‌کند که کامپایلر باید در هنگام رخ دادن یک [پنیک] اجرا کند. کتابخانه استاندارد تابع مدیریت پنیک خود را ارائه می‌دهد، اما در یک محیط `no_std` ما باید خودمان آن را تعریف کنیم.

[پنیک]: https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html

```rust
// in main.rs

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

[پارامتر `PanicInfo`][PanicInfo] شامل فایل و شماره خطی که پنیک رخ داده و پیام پنیکِ اختیاری می‌باشد. تابع هیچ وقت نباید چیزی را برگرداند به همین دلیل به عنوان یک [تابع واگرا]\(ترجمه: diverging function) بوسیله نوع برگشتی `!` [نوع ”هرگز“] علامت‌گذاری شده است. فعلا کار زیادی نیست که بتوانیم در این تابع انجام دهیم، بنابراین فقط یک حلقه بی‌نهایت می‌نویسیم.

[PanicInfo]: https://doc.rust-lang.org/nightly/core/panic/struct.PanicInfo.html
[تابع واگرا]: https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions
[نوع ”هرگز“]: https://doc.rust-lang.org/nightly/std/primitive.never.html

## آیتم زبان `eh_personality`

آیتم‌های زبان، توابع و انواع خاصی هستند که برای استفاده درون کامپایلر ضروری‌اند. به عنوان مثال، تِرِیت ‍‍[`Copy`]\(کلمه: Trait) یک آیتم زبان است که به کامپایلر می‌گوید کدام انواع دارای [_مفهوم کپی_][`Copy`] هستند. وقتی به [پیاده‌سازی][copy code] آن نگاه می‌کنیم، می‌بینیم که یک خاصیت ویژه `#[lang = "copy"]` دارد که آن را به عنوان یک آیتم زبان تعریف می‌کند.

[`Copy`]: https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html
[copy code]: https://github.com/rust-lang/rust/blob/485397e49a02a3b7ff77c17e4a3f16c653925cb3/src/libcore/marker.rs#L296-L299

درحالی که می‌توان پیاده‌سازی خاص برای آیتم‌های زبان فراهم کرد، فقط باید به عنوان آخرین راه حل از آن استفاده کرد. زیرا آیتم‌های زبان بسیار در جزئیات پیاده‌سازی ناپایدار هستند و حتی انواع آن‌ها نیز چک نمی‌شود (بنابراین کامپایلر حتی چک نمی‌کند که آرگومان تابع نوع درست را دارد). خوشبختانه یک راه پایدارتر برای حل مشکل آیتم زبان بالا وجود دارد.

[آیتم زبان `eh_personality`] یک تابع را به عنوان تابعی که برای پیاده‌سازی [بازکردن پشته (Stack Unwinding)] استفاده شده، علامت‌گذاری می‌کند. راست بطور پیش‌فرض از _بازکردن_ (ترجمه: unwinding) برای اجرای نابودگرهای (ترجمه: Destructors) تمام متغیرهای زنده درون استک در مواقع [پنیک] استفاده می‌کند. این تضمین می‌کند که تمام حافظه استفاده شده آزاد می‌شود و به نخ اصلی اجازه می‌دهد پنیک را دریافت کرده و اجرا را ادامه دهد. باز کردن، یک فرآیند پیچیده است و به برخی از کتابخانه‌های خاص سیستم‌عامل (به عنوان مثال [libunwind] در لینوکس یا [مدیریت اکسپشن ساخت یافته] در ویندوز) نیاز دارد، بنابراین ما نمی‌خواهیم از آن برای سیستم‌عامل خود استفاده کنیم.

[آیتم زبان `eh_personality`]: https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45
[بازکردن پشته (Stack Unwinding)]: https://www.bogotobogo.com/cplusplus/stackunwinding.php
[libunwind]: https://www.nongnu.org/libunwind/
[مدیریت اکسپشن ساخت یافته]: https://docs.microsoft.com/en-us/windows/win32/debug/structured-exception-handling

### غیرفعال کردن Unwinding

موارد استفاده دیگری نیز وجود دارد که باز کردن نامطلوب است، بنابراین راست به جای آن گزینه [قطع در پنیک] را فراهم می‌کند. این امر تولید اطلاعات نمادها (ترجمه: Symbol) را از بین می‌برد و بنابراین اندازه باینری را بطور قابل توجهی کاهش می‌دهد. چندین مکان وجود دارد که می توانیم باز کردن را غیرفعال کنیم. ساده‌ترین راه این است که خطوط زیر را به `Cargo.toml` اضافه کنید:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

این استراتژی پنیک را برای دو پروفایل `dev` (در `cargo build` استفاده می‌شود)  و پروفایل `release` (در ` cargo build --release` استفاده می‌شود) تنظیم می‌کند. اکنون آیتم زبان `eh_personality` نباید دیگر لازم باشد.

[قطع در پنیک]: https://github.com/rust-lang/rust/pull/32900

اکنون هر دو خطای فوق را برطرف کردیم. با این حال‌، اگر اکنون بخواهیم آن را کامپایل کنیم، خطای دیگری رخ می‌دهد:

```
> cargo build
error: requires `start` lang_item
```

برنامه ما آیتم زبان `start` که نقطه ورود را مشخص می‌کند، را ندارد.

## خاصیت `start`

ممکن است تصور شود که تابع `main` اولین تابعی است که هنگام اجرای یک برنامه فراخوانی می‌شود. با این حال، بیشتر زبان‌ها دارای [سیستم رانتایم] هستند که مسئول مواردی مانند جمع آوری زباله (به عنوان مثال در جاوا) یا نخ‌های نرم‌افزار (به عنوان مثال goroutines در Go) است. این رانتایم باید قبل از `main` فراخوانی شود، زیرا باید خود را مقداردهی اولیه و آماده کند.

[سیستم رانتایم]: https://en.wikipedia.org/wiki/Runtime_system

در یک باینری معمولی راست که از کتابخانه استاندارد استفاده می‌کند، اجرا در یک کتابخانه رانتایم C به نام `crt0` ("زمان اجرا صفر C") شروع می‌شود، که محیط را برای یک برنامه C تنظیم می‌کند. این شامل ایجاد یک پشته و قرار دادن آرگومان‌ها در رجیسترهای مناسب است. سپس رانتایم C [ورودی رانتایم راست][rt::lang_start] را فراخوانی می‌کند، که با آیتم زبان `start` مشخص شده است. راست فقط یک رانتایم بسیار کوچک دارد، که مواظب برخی از کارهای کوچک مانند راه‌اندازی محافظ‌های سرریز پشته یا چاپ backtrace با پنیک می‌باشد. رانتایم در نهایت تابع `main` را فراخوانی می‌کند.

[rt::lang_start]: https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73

برنامه اجرایی مستقل ما به رانتایم و `crt0` دسترسی ندارد، بنابراین باید نقطه ورود را مشخص کنیم. پیاده‌سازی آیتم زبان `start` کمکی نخواهد کرد، زیرا همچنان به `crt0` نیاز دارد. در عوض، باید نقطه ورود `crt0` را مستقیماً بازنویسی کنیم.

### بازنویسی نقطه ورود

برای اینکه به کامپایلر راست بگوییم که نمی‌خواهیم از زنجیره نقطه ورودی عادی استفاده کنیم، ویژگی `#![no_main]` را اضافه می‌کنیم.

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

ممکن است متوجه شده باشید که ما تابع `main` را حذف کردیم. دلیل این امر این است که `main` بدون یک رانتایم اساسی که آن را صدا کند معنی ندارد. در عوض، ما در حال بازنویسی نقطه ورود سیستم‌عامل با تابع `start_` خود هستیم:

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
```

با استفاده از ویژگی `[no_mangle]#` ما [name mangling] را غیرفعال می کنیم تا اطمینان حاصل کنیم که کامپایلر راست تابعی با نام `start_` را خروجی می‌دهد. بدون این ویژگی، کامپایلر برخی از نمادهای رمزنگاری شده `ZN3blog_os4_start7hb173fedf945531caE_` را تولید می‌کند تا به هر تابع یک نام منحصر به فرد بدهد. این ویژگی لازم است زیرا در مرحله بعدی باید نام تایع نقطه ورود را به لینکر (کلمه: linker) بگوییم.

ما همچنین باید تابع را به عنوان `"extern "C` علامت‌گذاری کنیم تا به کامپایلر بگوییم که باید از [قرارداد فراخوانی C] برای این تابع استفاده کند (به جای قرارداد مشخص نشده فراخوانی راست). دلیل نامگذاری تابع `start_` این است که این نام نقطه پیش‌فرض ورودی برای اکثر سیستم‌ها است.

[name mangling]: https://en.wikipedia.org/wiki/Name_mangling
[قرارداد فراخوانی C]: https://en.wikipedia.org/wiki/Calling_convention

نوع بازگشت `!` به این معنی است که تایع واگرا است، یعنی اجازه بازگشت ندارد. این مورد لازم است زیرا نقطه ورود توسط هیچ تابعی فراخوانی نمی‌شود، بلکه مستقیماً توسط سیستم‌عامل یا بوت‌لودر فراخوانی می‌شود. بنابراین به جای بازگشت، نقطه ورود باید به عنوان مثال [فراخوان سیستمی `exit`] از سیستم‌عامل را فراخوانی کند. در مورد ما، خاموش کردن دستگاه می‌تواند اقدامی منطقی باشد، زیرا در صورت بازگشت باینری مستقل دیگر کاری برای انجام دادن وجود ندارد. در حال حاضر، ما این نیاز را با حلقه‌های بی‌پایان انجام می‌دهیم.

[فراخوان سیستمی `exit`]: https://en.wikipedia.org/wiki/Exit_(system_call)

حالا وقتی `cargo build` را اجرا می‌کنیم، با یک خطای _لینکر_ زشت مواجه می‌شویم.

## خطا‌های لینکر (Linker)

لینکر برنامه‌ای است که کد تولید شده را ترکیب کرده و یک فایل اجرایی می‌سازد. از آن‌جا که فرمت اجرایی بین لینوکس، ویندوز و macOS متفاوت است، هر سیستم لینکر خود را دارد که خطای متفاوتی ایجاد می‌کند. علت اصلی خطاها یکسان است: پیکربندی پیش‌فرض لینکر فرض می‌کند که برنامه ما به رانتایم C وابسته است، که این طور نیست.

برای حل خطاها، باید به لینکر بگوییم که نباید رانتایم C را اضافه کند. ما می‌توانیم این کار را با اضافه کردن مجموعه‌ای از آرگمان‌ها به لینکر یا با ساختن یک هدف (ترجمه: Target) bare metal انجام دهیم.

### بیلد کردن برای یک هدف bare metal

راست به طور پیش‌فرض سعی در ایجاد یک اجرایی دارد که بتواند در محیط سیستم فعلی شما اجرا شود. به عنوان مثال، اگر از ویندوز در `x86_64` استفاده می‌کنید، راست سعی در ایجاد یک `exe.` اجرایی ویندوز دارد که از دستورالعمل‌های `x86_64` استفاده می‌کند. به این محیط سیستم "میزبان" شما گفته می‌شود.

راست برای توصیف محیط‌های مختلف، از رشته‌ای به نام [_target triple_]\(سه‌گانه هدف) استفاده می‌کند. با اجرای `rustc --version --verbose` می‌توانید target triple را برای سیستم میزبان خود مشاهده کنید:

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

خروجی فوق از یک سیستم لینوکس `x86_64` است. می‌بینیم که سه‌گانه میزبان `x86_64-unknown-linux-gnu` است که شامل معماری پردازنده (`x86_64`)، فروشنده (`ناشناخته`)، سیستم‌عامل (` linux`) و [ABI] (`gnu`) است.

[ABI]: https://en.wikipedia.org/wiki/Application_binary_interface

با کامپایل کردن برای سه‌گانه میزبان‌مان، کامپایلر راست و لینکر فرض می‌کنند که یک سیستم‌عامل زیرین مانند Linux یا Windows وجود دارد که به طور پیش‌فرض از رانتایم C استفاده می‌کند، که باعث خطاهای لینکر می‌شود. بنابراین برای جلوگیری از خطاهای لینکر، می‌توانیم برای محیطی متفاوت و بدون سیستم‌عامل زیرین کامپایل کنیم.

یک مثال برای چنین محیطِ bare metal ی، سه‌گانه هدف `thumbv7em-none-eabihf` است، که یک سیستم [تعبیه شده][ARM] را توصیف می‌کند. جزئیات مهم نیستند، مهم این است که سه‌گانه هدف فاقد سیستم‌عامل زیرین باشد، که با `none` در سه‌گانه هدف نشان داده می‌شود. برای این که بتوانیم برای این هدف کامپایل کنیم، باید آن را به rustup اضافه کنیم:

[تعبیه شده]: https://en.wikipedia.org/wiki/Embedded_system
[ARM]: https://en.wikipedia.org/wiki/ARM_architecture

```
rustup target add thumbv7em-none-eabihf
```

با این کار نسخه‌ای از کتابخانه استاندارد (و core) برای سیستم بارگیری می‌شود. اکنون می‌توانیم برای این هدف اجرایی مستقل خود را بسازیم:

```
cargo build --target thumbv7em-none-eabihf
```

با استفاده از یک آرگومان `target--`، ما اجرایی خود را برای یک سیستم هدف bare metal [کراس کامپایل] می‌کنیم. از آن‌جا که سیستم هدف فاقد سیستم‌عامل است، لینکر سعی نمی‌کند رانتایم C را به آن پیوند دهد و بیلد ما بدون هیچ گونه خطای لینکر با موفقیت انجام می‌شود.

[کراس کامپایل]: https://en.wikipedia.org/wiki/Cross_compiler

این روشی است که ما برای ساخت هسته سیستم‌عامل خود استفاده خواهیم کرد. به جای `thumbv7em-none-eabihf`، ما از یک [هدف سفارشی] استفاده خواهیم کرد که یک محیط `x86_64` bare metal را توصیف می‌کند. جزئیات در پست بعدی توضیح داده خواهد شد.

[هدف سفارشی]: https://doc.rust-lang.org/rustc/targets/custom.html

### آرگومان‌های لینکر

به جای کامپایل کردن برای یک سیستم bare metal، می‌توان خطاهای لینکر را با استفاده از مجموعه خاصی از آرگومان‌ها به لینکر حل کرد. این روشی نیست که ما برای هسته خود استفاده کنیم، بنابراین این بخش اختیاری است و فقط برای کامل بودن ارائه می‌شود. برای نشان دادن محتوای اختیاری، روی _"آرگومان‌های لینکر"_ در زیر کلیک کنید.

<details>

<summary>آرگومان‌های لینکر</summary>

در این بخش، ما در مورد خطاهای لینکر که در لینوکس، ویندوز و macOS رخ می‌دهد بحث می‌کنیم و نحوه حل آن‌ها را با استفاده از آرگومان‌های اضافی به لینکر توضیح می‌دهیم. توجه داشته باشید که فرمت اجرایی و لینکر بین سیستم‌عامل‌ها متفاوت است، بنابراین برای هر سیستم مجموعه‌ای متفاوت از آرگومان‌ها مورد نیاز است.

#### لینوکس

در لینوکس خطای لینکر زیر رخ می‌دهد (کوتاه شده):

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

مشکل این است که لینکر به طور پیش‌فرض شامل روال راه‌اندازی رانتایم C است که به آن `start_` نیز گفته می‌شود. به برخی از نمادهای کتابخانه استاندارد C یعنی `libc` نیاز دارد که به دلیل ویژگی`no_std` آن‌ها را نداریم، بنابراین لینکر نمی‌تواند این مراجع را پیدا کند. برای حل این مسئله، با استفاده از پرچم `nostartfiles-` می‌توانیم به لینکر بگوییم که نباید روال راه‌اندازی C را لینک دهد.

یکی از راه‌های عبور صفات لینکر از طریق cargo، دستور `cargo rustc` است. این دستور دقیقاً مانند `cargo build` رفتار می‌کند، اما اجازه می‌دهد گزینه‌ها را به `rustc`، کامپایلر اصلی راست انتقال دهید. `rustc` دارای پرچم`C link-arg-` است که آرگومان را به لینکر منتقل می‌کند. با ترکیب همه این‌ها، دستور بیلد جدید ما به این شکل است:

```
cargo rustc -- -C link-arg=-nostartfiles
```

اکنون کریت ما بصورت اجرایی مستقل در لینوکس ساخته می‌شود!

لازم نیست که صریحاً نام تابع نقطه ورود را مشخص کنیم، زیرا لینکر به طور پیش‌فرض به دنبال تابعی با نام `start_` می‌گردد.

#### ویندوز

در ویندوز، یک خطای لینکر متفاوت رخ می‌دهد (کوتاه شده):

```
error: linking with `link.exe` failed: exit code: 1561
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1561: entry point must be defined
```

خطای "entry point must be defined" به این معنی است که لینکر نمی‌تواند نقطه ورود را پیدا کند. در ویندوز، نام پیش‌فرض نقطه ورود  [بستگی به زیر سیستم استفاده شده دارد] [windows-subsystem]. برای زیر سیستم `CONSOLE` لینکر به دنبال تابعی به نام `mainCRTStartup` و برای زیر سیستم `WINDOWS` به دنبال تابعی به نام `WinMainCRTStartup` می‌گردد. برای بازنویسی این پیش‌فرض و به لینکر گفتن که در عوض به دنبال تابع `_start` ما باشد ، می توانیم یک آرگومان `ENTRY/` را به لینکر ارسال کنیم:

[windows-subsystems]: https://docs.microsoft.com/en-us/cpp/build/reference/entry-entry-point-symbol

```
cargo rustc -- -C link-arg=/ENTRY:_start
```

از متفاوت بودن فرمت آرگومان، به وضوح می‌فهمیم که لینکر ویندوز یک برنامه کاملاً متفاوت از لینکر Linux است.

اکنون یک خطای لینکر متفاوت رخ داده است:

```
error: linking with `link.exe` failed: exit code: 1221
  |
  = note: "C:\\Program Files (x86)\\…\\link.exe" […]
  = note: LINK : fatal error LNK1221: a subsystem can't be inferred and must be
          defined
```

این خطا به این دلیل رخ می‌دهد که برنامه‌های اجرایی ویندوز می‌توانند از [زیر سیستم های][windows-subsystems] مختلف استفاده کنند. برای برنامه‌های عادی، بسته به نام نقطه ورود استنباط می شوند: اگر نقطه ورود `main` نامگذاری شود، از زیر سیستم `CONSOLE` و اگر نقطه ورود `WinMain` نامگذاری شود، از زیر سیستم `WINDOWS` استفاده می‌شود. از آن‌جا که تابع `start_` ما نام دیگری دارد، باید زیر سیستم را صریحاً مشخص کنیم:

```
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
```

ما در اینجا از زیر سیستم `CONSOLE` استفاده می‌کنیم، اما زیر سیستم `WINDOWS` نیز کار خواهد کرد. به جای اینکه چند بار از `C link-arg-` استفاده کنیم، از`C link-args-` استفاده می‌کنیم که لیستی از آرگومان‌ها به صورت جدا شده با فاصله را دریافت می‌کند.

با استفاده از این دستور، اجرایی ما باید با موفقیت بر روی ویندوز ساخته شود.

#### macOS

در macOS، خطای لینکر زیر رخ می‌دهد (کوتاه شده):

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: entry point (_main) undefined. for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

این پیام خطا به ما می‌گوید که لینکر نمی‌تواند یک تابع نقطه ورود را با نام پیش‌فرض `main` پیدا کند (به دلایلی همه توابع در macOS دارای پیشوند `_` هستند). برای تنظیم نقطه ورود به تابع `start_` ، آرگومان لینکر `e-` را استفاده می‌کنیم:

```
cargo rustc -- -C link-args="-e __start"
```

پرچم `e‌-‌` نام تابع نقطه ورود را مشخص می‌کند. از آن‌جا که همه توابع در macOS دارای یک پیشوند اضافی `_` هستند، ما باید به جای `start_` نقطه ورود را روی `start__` تنظیم کنیم.

اکنون خطای لینکر زیر رخ می‌دهد:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: dynamic main executables must link with libSystem.dylib
          for architecture x86_64
          clang: error: linker command failed with exit code 1 […]
```

سیستم‌عامل مک‌ [رسماً باینری‌هایی را که بطور استاتیک با هم پیوند دارند پشتیبانی نمی‌کند] و بطور پیش‌فرض به برنامه‌هایی برای پیوند دادن کتابخانه `libSystem` نیاز دارد. برای تغییر این حالت و پیوند دادن یک باینری استاتیک، پرچم `static-` را به لینکر ارسال می‌کنیم:

[باینری‌هایی را که بطور استاتیک با هم پیوند دارند پشتیبانی نمی‌کند]: https://developer.apple.com/library/archive/qa/qa1118/_index.html

```
cargo rustc -- -C link-args="-e __start -static"
```

این نیز کافی نیست، سومین خطای لینکر رخ می‌دهد:

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" […]
  = note: ld: library not found for -lcrt0.o
          clang: error: linker command failed with exit code 1 […]
```

این خطا رخ می‌دهد زیرا برنامه های موجود در macOS به طور پیش‌فرض به `crt0` ("رانتایم صفر C") پیوند دارند. این همان خطایی است که در لینوکس داشتیم و با افزودن آرگومان لینکر `nostartfiles-` نیز قابل حل است:

```
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

اکنون برنامه ما باید با موفقیت بر روی macOS ساخته شود.

#### متحد کردن دستورات Build

در حال حاضر بسته به سیستم‌عامل میزبان، دستورات ساخت متفاوتی داریم که ایده آل نیست. برای جلوگیری از این، می‌توانیم فایلی با نام `cargo/config.toml.` ایجاد کنیم که حاوی آرگومان‌های خاص هر پلتفرم است:

```toml
# in .cargo/config.toml

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

کلید `rustflags` شامل آرگومان‌هایی است که بطور خودکار به هر فراخوانی `rustc` اضافه می‌شوند. برای کسب اطلاعات بیشتر در مورد فایل `cargo/config.toml‌.‌` به [اسناد رسمی](https://doc.rust-lang.org/cargo/reference/config.html) مراجعه کنید.

اکنون برنامه ما باید در هر سه سیستم‌عامل با یک `cargo build` ساده قابل بیلد باشد.

#### آیا شما باید این کار را انجام دهید؟

اگرچه ساخت یک اجرایی مستقل برای لینوکس، ویندوز و macOS امکان پذیر است، اما احتمالاً ایده خوبی نیست. چرا که اجرایی ما هنوز انتظار موارد مختلفی را دارد، به عنوان مثال با فراخوانی تابع `start_` یک پشته مقداردهی اولیه شده است. بدون رانتایم C، ممکن است برخی از این الزامات برآورده نشود، که ممکن است باعث شکست برنامه ما شود، به عنوان مثال از طریق `segmentation fault`.

اگر می خواهید یک باینری کوچک ایجاد کنید که بر روی سیستم‌عامل موجود اجرا شود، اضافه کردن `libc` و تنظیم ویژگی `[start]#` همان‌طور که [اینجا](https://doc.rust-lang.org/1.16.0/book/no-stdlib.html) شرح داده شده است، احتمالاً ایده بهتری است.

</details>

## خلاصه

یک باینری مستقل مینیمال راست مانند این است:

`src/main.rs`:

```rust
#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use core::panic::PanicInfo;

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}

/// This function is called on panic.
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

# the profile used for `cargo build`
[profile.dev]
panic = "abort" # disable stack unwinding on panic

# the profile used for `cargo build --release`
[profile.release]
panic = "abort" # disable stack unwinding on panic
```

برای ساخت این باینری، ما باید برای یک هدف bare metal مانند `thumbv7em-none-eabihf` کامپایل کنیم:

```
cargo build --target thumbv7em-none-eabihf
```

یک راه دیگر این است که می‌توانیم آن را برای سیستم میزبان با استفاده از آرگومان‌های اضافی لینکر کامپایل کنیم:

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

توجه داشته باشید که این فقط یک نمونه حداقلی از باینری مستقل راست است. این باینری انتظار چیزهای مختلفی را دارد، به عنوان مثال با فراخوانی تابع `start_` یک پشته مقداردهی اولیه می‌شود. **بنابراین برای هر گونه استفاده واقعی از چنین باینری، مراحل بیشتری لازم است**.

## بعدی چیست؟

[پست بعدی] مراحل مورد نیاز برای تبدیل باینری مستقل به حداقل هسته سیستم‌عامل را توضیح می‌دهد. که شامل ایجاد یک هدف سفارشی، ترکیب اجرایی ما با بوت‌لودر و یادگیری نحوه چاپ چیزی در صفحه است.

[پست بعدی]: @/edition-2/posts/02-minimal-rust-kernel/index.fa.md
