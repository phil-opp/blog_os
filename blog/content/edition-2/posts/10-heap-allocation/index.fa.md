+++
title = "تخصیص هیپ"
weight = 10
path = "fa/heap-allocation"
date = 2019-06-26

[extra]
# Please update this when updating the translation
translation_based_on_commit = "c850d4237ddf040e062a46404dee7dbae8c96b1c"
rtl = true
+++

این پست پشتیبانی از تخصیص هیپ را به هسته ما اضافه می‌کند. ابتدا مقدمه‌ای بر حافظه پویا ارائه می‌دهد و نشان می‌دهد که چگونه بررسی‌کننده قرض (borrow checker) از خطاهای رایج تخصیص جلوگیری می‌کند. سپس رابط پایه‌ی تخصیص در Rust را پیاده‌سازی می‌کند، یک ناحیه حافظه هیپ ایجاد می‌کند و یک کِرِیت تخصیص‌دهنده را راه‌اندازی می‌کند. در پایان این پست، تمام نوع‌های تخصیص و مجموعه‌ی کِرِیت داخلی `alloc` برای هسته ما در دسترس خواهند بود.

<!-- more -->

این بلاگ بصورت آزاد روی [گیت‌هاب] توسعه داده شده است. اگر شما مشکل یا سوالی دارید، لطفاً آن‌جا یک ایشو باز کنید. شما همچنین می‌توانید [در زیر] این پست کامنت بگذارید. منبع کد کامل این پست را می‌توانید در بِرَنچ [`post-10`][post branch] پیدا کنید.

[گیت‌هاب]: https://github.com/phil-opp/blog_os
[در زیر]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-10

<!-- toc -->

## متغیرهای محلی و استاتیک

ما در حال حاضر از دو نوع متغیر در هسته خود استفاده می‌کنیم: متغیرهای محلی و متغیرهای `static`. متغیرهای محلی روی [پشته فراخوانی] ذخیره می‌شوند و تنها تا زمانی که تابع دربرگیرنده بازگردد معتبر هستند. متغیرهای استاتیک در یک مکان ثابت حافظه ذخیره می‌شوند و همیشه در تمام طول عمر برنامه زنده می‌مانند.

### متغیرهای محلی

متغیرهای محلی روی [پشته فراخوانی] ذخیره می‌شوند، که یک [ساختار داده پشته] است و از عملیات `push` و `pop` پشتیبانی می‌کند. در هر ورود به تابع، پارامترها، آدرس بازگشت و متغیرهای محلیِ تابعِ فراخوانی‌شده توسط کامپایلر روی پشته قرار داده می‌شوند:

[پشته فراخوانی]: https://en.wikipedia.org/wiki/Call_stack
[ساختار داده پشته]: https://en.wikipedia.org/wiki/Stack_(abstract_data_type)

![An `outer()` and an `inner(i: usize)` function, where `outer` calls `inner(1)`. Both have some local variables. The call stack contains the following slots: the local variables of outer, then the argument `i = 1`, then the return address, then the local variables of inner.](call-stack.svg)

مثال بالا پشته فراخوانی را پس از آن‌که تابع `outer` تابع `inner` را فراخوانی کرده است نشان می‌دهد. می‌بینیم که پشته فراخوانی ابتدا شامل متغیرهای محلی `outer` است. هنگام فراخوانی `inner`، پارامتر `1` و آدرس بازگشتِ تابع روی پشته قرار گرفتند. سپس کنترل به `inner` منتقل شد، که متغیرهای محلی خود را روی پشته قرار داد.

پس از بازگشت تابع `inner`، بخش مربوط به آن از پشته فراخوانی دوباره برداشته می‌شود و تنها متغیرهای محلی `outer` باقی می‌مانند:

![The call stack containing only the local variables of `outer`](call-stack-return.svg)

می‌بینیم که متغیرهای محلی `inner` تنها تا زمان بازگشت تابع زنده می‌مانند. کامپایلر Rust این طول عمرها را اعمال می‌کند و هنگامی که یک مقدار را برای مدتی طولانی‌تر از حد مجاز استفاده کنیم خطا می‌دهد، برای مثال وقتی سعی می‌کنیم یک ارجاع به یک متغیر محلی را برگردانیم:

```rust
fn inner(i: usize) -> &'static u32 {
    let z = [1, 2, 3];
    &z[i]
}
```

([این مثال را در playground اجرا کنید](https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=6186a0f3a54f468e1de8894996d12819))

اگرچه برگرداندن یک ارجاع در این مثال معنایی ندارد، اما مواردی وجود دارد که می‌خواهیم یک متغیر بیشتر از تابع زنده بماند. ما پیش‌تر چنین موردی را در هسته خود دیدیم، هنگامی که سعی کردیم [یک جدول توصیف‌کننده وقفه را بارگذاری کنیم] و مجبور شدیم برای گسترش طول عمر از یک متغیر `static` استفاده کنیم.

[یک جدول توصیف‌کننده وقفه را بارگذاری کنیم]: @/edition-2/posts/05-cpu-exceptions/index.md#loading-the-idt

### متغیرهای استاتیک

متغیرهای استاتیک در یک مکان ثابت حافظه، جدا از پشته، ذخیره می‌شوند. این مکان حافظه در زمان کامپایل توسط لینکر تعیین شده و در فایل اجرایی کدگذاری می‌شود. استاتیک‌ها در تمام مدت اجرای برنامه زنده هستند، بنابراین طول عمر `'static` دارند و همیشه می‌توان از متغیرهای محلی به آن‌ها ارجاع داد:

![The same outer/inner example, except that inner has a `static Z: [u32; 3] = [1,2,3];` and returns a `&Z[i]` reference](call-stack-static.svg)

هنگامی که تابع `inner` در مثال بالا بازمی‌گردد، بخش مربوط به آن از پشته فراخوانی از بین می‌رود. متغیرهای استاتیک در یک محدوده حافظه جداگانه قرار دارند که هرگز از بین نمی‌رود، بنابراین ارجاع `&Z[1]` پس از بازگشت هم همچنان معتبر است.

جدا از طول عمر `'static`، متغیرهای استاتیک این ویژگی مفید را نیز دارند که مکانشان در زمان کامپایل مشخص است، بنابراین برای دسترسی به آن‌ها به هیچ ارجاعی نیاز نیست. ما از این ویژگی برای ماکرو `println` خود بهره بردیم: با استفاده از یک [`Writer` استاتیک] در داخل آن، برای فراخوانی ماکرو به هیچ ارجاع `&mut Writer` نیازی نیست، که این در [کنترل‌کننده‌های استثنا] بسیار مفید است، جایی که به هیچ متغیر اضافی دسترسی نداریم.

[`Writer` استاتیک]: @/edition-2/posts/03-vga-text-buffer/index.md#a-global-interface
[کنترل‌کننده‌های استثنا]: @/edition-2/posts/05-cpu-exceptions/index.md#implementation

با این حال، این ویژگیِ متغیرهای استاتیک یک ایراد اساسی به همراه دارد: آن‌ها به‌طور پیش‌فرض فقط‌خواندنی هستند. Rust این را اجبار می‌کند، زیرا اگر مثلاً دو نخ هم‌زمان یک متغیر استاتیک را تغییر دهند، یک [رقابت داده] رخ می‌دهد. تنها راه تغییر یک متغیر استاتیک، محصور کردن آن در نوع [`Mutex`] است، که تضمین می‌کند در هر لحظه تنها یک ارجاع `&mut` وجود دارد. ما پیش‌تر از یک `Mutex` برای [`Writer` استاتیکِ بافر VGA][vga mutex] استفاده کردیم.

[رقابت داده]: https://doc.rust-lang.org/nomicon/races.html
[`Mutex`]: https://docs.rs/spin/0.5.2/spin/struct.Mutex.html
[vga mutex]: @/edition-2/posts/03-vga-text-buffer/index.md#spinlocks

## حافظه پویا

متغیرهای محلی و استاتیک در کنار هم بسیار قدرتمند هستند و بیشتر موارد استفاده را ممکن می‌سازند. با این حال، دیدیم که هر دو محدودیت‌های خود را دارند:

- متغیرهای محلی تنها تا پایان تابع یا بلوک دربرگیرنده زنده می‌مانند. دلیلش این است که آن‌ها روی پشته فراخوانی قرار دارند و پس از بازگشت تابع دربرگیرنده از بین می‌روند.
- متغیرهای استاتیک همیشه در تمام مدت اجرای برنامه زنده هستند، بنابراین هیچ راهی برای بازپس‌گیری و استفاده مجدد از حافظه آن‌ها، وقتی دیگر مورد نیاز نیستند، وجود ندارد. همچنین، معنای مالکیت آن‌ها نامشخص است و از همه توابع قابل دسترسی هستند، بنابراین وقتی بخواهیم آن‌ها را تغییر دهیم باید توسط یک [`Mutex`] محافظت شوند.

محدودیت دیگر متغیرهای محلی و استاتیک این است که اندازه‌ای ثابت دارند. بنابراین نمی‌توانند مجموعه‌ای را ذخیره کنند که با افزوده شدن عناصر بیشتر به‌صورت پویا رشد کند. (پیشنهادهایی برای [مقادیر بدون اندازه] در Rust وجود دارد که متغیرهای محلی با اندازه پویا را ممکن می‌کنند، اما تنها در برخی موارد خاص کار می‌کنند.)

[مقادیر بدون اندازه]: https://github.com/rust-lang/rust/issues/48055

برای دور زدن این ایرادها، زبان‌های برنامه‌نویسی اغلب از یک ناحیه حافظه سوم برای ذخیره متغیرها پشتیبانی می‌کنند که **هیپ** نامیده می‌شود. هیپ از _تخصیص پویای حافظه_ در زمان اجرا از طریق دو تابع به نام‌های `allocate` و `deallocate` پشتیبانی می‌کند. به این شکل کار می‌کند: تابع `allocate` یک تکه حافظه آزاد با اندازه مشخص‌شده برمی‌گرداند که می‌توان از آن برای ذخیره یک متغیر استفاده کرد. این متغیر سپس تا زمانی که با فراخوانی تابع `deallocate` همراه با یک ارجاع به آن متغیر آزاد شود، زنده می‌ماند.

بیایید یک مثال را بررسی کنیم:

![The inner function calls `allocate(size_of([u32; 3]))`, writes `z.write([1,2,3]);`, and returns `(z as *mut u32).offset(i)`. On the returned value `y`, the outer function performs `deallocate(y, size_of(u32))`.](call-stack-heap.svg)

در اینجا تابع `inner` به‌جای متغیرهای استاتیک از حافظه هیپ برای ذخیره `z` استفاده می‌کند. ابتدا یک بلوک حافظه با اندازه لازم تخصیص می‌دهد، که یک [اشاره‌گر خام] از نوع `*mut u32` برمی‌گرداند. سپس از متد [`ptr::write`] برای نوشتن آرایه `[1,2,3]` در آن استفاده می‌کند. در گام آخر، از تابع [`offset`] برای محاسبه یک اشاره‌گر به عنصر `i`-اُم استفاده کرده و آن را برمی‌گرداند. (توجه کنید که برای اختصار، برخی از تبدیل‌های نوع و بلوک‌های unsafe لازم را در این تابع نمونه حذف کرده‌ایم.)

[اشاره‌گر خام]: https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`ptr::write`]: https://doc.rust-lang.org/core/ptr/fn.write.html
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

حافظه تخصیص‌داده‌شده تا زمانی که به‌صراحت با فراخوانی `deallocate` آزاد شود زنده می‌ماند. بنابراین، اشاره‌گر بازگردانده‌شده حتی پس از بازگشت `inner` و از بین رفتن بخش مربوط به آن از پشته فراخوانی همچنان معتبر است. مزیت استفاده از حافظه هیپ در مقایسه با حافظه استاتیک این است که حافظه پس از آزاد شدن می‌تواند دوباره استفاده شود، کاری که ما با فراخوانی `deallocate` در `outer` انجام می‌دهیم. پس از آن فراخوانی، وضعیت به این شکل است:

![The call stack contains the local variables of `outer`, the heap contains `z[0]` and `z[2]`, but no longer `z[1]`.](call-stack-heap-freed.svg)

می‌بینیم که جایگاه `z[1]` دوباره آزاد است و می‌تواند برای فراخوانی بعدی `allocate` استفاده شود. با این حال، همچنین می‌بینیم که `z[0]` و `z[2]` هرگز آزاد نمی‌شوند، زیرا ما هرگز آن‌ها را آزادسازی نمی‌کنیم. چنین باگی _نشت حافظه_ (memory leak) نامیده می‌شود و اغلب علت مصرف بیش از حد حافظه توسط برنامه‌هاست (فقط تصور کنید چه اتفاقی می‌افتد وقتی `inner` را به‌طور مکرر در یک حلقه فراخوانی کنیم). این ممکن است بد به نظر برسد، اما انواع بسیار خطرناک‌تری از باگ‌ها وجود دارند که می‌توانند با تخصیص پویا رخ دهند.

### خطاهای رایج

جدا از نشت حافظه، که ناخوشایند است اما برنامه را در برابر مهاجمان آسیب‌پذیر نمی‌کند، دو نوع باگ رایج با پیامدهای شدیدتر وجود دارد:

- وقتی به‌طور تصادفی پس از فراخوانی `deallocate` روی یک متغیر همچنان به استفاده از آن ادامه دهیم، یک آسیب‌پذیری به‌اصطلاح **استفاده-پس-از-آزادسازی** (use-after-free) داریم. چنین باگی باعث رفتار تعریف‌نشده می‌شود و اغلب می‌تواند توسط مهاجمان برای اجرای کد دلخواه مورد سوءاستفاده قرار گیرد.
- وقتی به‌طور تصادفی یک متغیر را دو بار آزاد کنیم، یک آسیب‌پذیری **آزادسازی-دوباره** (double-free) داریم. این مشکل‌ساز است زیرا ممکن است تخصیص دیگری را آزاد کند که پس از فراخوانی اولِ `deallocate` در همان مکان تخصیص داده شده است. بنابراین، می‌تواند دوباره به یک آسیب‌پذیری استفاده-پس-از-آزادسازی منجر شود.

این نوع آسیب‌پذیری‌ها به‌خوبی شناخته شده‌اند، بنابراین ممکن است انتظار داشته باشیم که تا الان مردم یاد گرفته باشند چگونه از آن‌ها اجتناب کنند. اما نه، چنین آسیب‌پذیری‌هایی هنوز هم به‌طور منظم پیدا می‌شوند، برای مثال این [آسیب‌پذیری استفاده-پس-از-آزادسازی در لینوکس][linux vulnerability] (سال 2019)، که اجرای کد دلخواه را ممکن می‌کرد. یک جست‌وجوی وب مانند `use-after-free linux {current year}` احتمالاً همیشه نتیجه خواهد داشت. این نشان می‌دهد که حتی بهترین برنامه‌نویسان هم همیشه قادر نیستند حافظه پویا را در پروژه‌های پیچیده به‌درستی مدیریت کنند.

[linux vulnerability]: https://securityboulevard.com/2019/02/linux-use-after-free-vulnerability-found-in-linux-2-6-through-4-20-11/

برای اجتناب از این مشکلات، بسیاری از زبان‌ها، مانند جاوا یا پایتون، حافظه پویا را به‌طور خودکار با تکنیکی به نام [_جمع‌آوری زباله_] مدیریت می‌کنند. ایده این است که برنامه‌نویس هرگز `deallocate` را به‌صورت دستی فراخوانی نکند. در عوض، برنامه به‌طور منظم متوقف شده و برای یافتن متغیرهای بدون استفاده هیپ پویش می‌شود، که سپس به‌طور خودکار آزادسازی می‌شوند. بنابراین، آسیب‌پذیری‌های بالا هرگز نمی‌توانند رخ دهند. ایرادهای این روش، سربار کارایی ناشی از پویش منظم و زمان‌های توقف احتمالاً طولانی است.

[_جمع‌آوری زباله_]: https://en.wikipedia.org/wiki/Garbage_collection_(computer_science)

Rust رویکرد متفاوتی به این مسئله دارد: از مفهومی به نام [_مالکیت_] استفاده می‌کند که می‌تواند درستی عملیات حافظه پویا را در زمان کامپایل بررسی کند. بنابراین، برای اجتناب از آسیب‌پذیری‌های ذکرشده نیازی به جمع‌آوری زباله نیست، که یعنی هیچ سربار کارایی وجود ندارد. مزیت دیگر این رویکرد این است که برنامه‌نویس همچنان کنترل دقیقی بر استفاده از حافظه پویا دارد، درست مانند C یا C++.

[_مالکیت_]: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html

### تخصیص‌ها در Rust

به‌جای این‌که برنامه‌نویس مجبور باشد `allocate` و `deallocate` را به‌صورت دستی فراخوانی کند، کتابخانه استاندارد Rust نوع‌های انتزاعی فراهم می‌کند که این توابع را به‌طور ضمنی فراخوانی می‌کنند. مهم‌ترین نوع [**`Box`**] است، که انتزاعی برای یک مقدار تخصیص‌یافته روی هیپ است. این نوع یک تابع سازنده [`Box::new`] فراهم می‌کند که یک مقدار می‌گیرد، `allocate` را با اندازه آن مقدار فراخوانی می‌کند و سپس مقدار را به جایگاه تازه تخصیص‌یافته روی هیپ منتقل می‌کند. برای آزاد کردن دوباره حافظه هیپ، نوع `Box` [تِرِیت `Drop`][`Drop` trait] را پیاده‌سازی می‌کند تا هنگام خارج شدن از دامنه، `deallocate` را فراخوانی کند:

[**`Box`**]: https://doc.rust-lang.org/std/boxed/index.html
[`Box::new`]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html#method.new
[`Drop` trait]: https://doc.rust-lang.org/book/ch15-03-drop.html

```rust
{
    let z = Box::new([1,2,3]);
    […]
} // z goes out of scope and `deallocate` is called
```

این الگو نام عجیبِ [_دریافت منبع، مقداردهی اولیه است_][_resource acquisition is initialization_] (یا به‌اختصار _RAII_) را دارد. این الگو از C++ سرچشمه گرفته است، جایی که برای پیاده‌سازی یک نوع انتزاعی مشابه به نام [`std::unique_ptr`] استفاده می‌شود.

[_resource acquisition is initialization_]: https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization
[`std::unique_ptr`]: https://en.cppreference.com/cpp/memory/unique_ptr

چنین نوعی به‌تنهایی برای جلوگیری از همه باگ‌های استفاده-پس-از-آزادسازی کافی نیست، زیرا برنامه‌نویسان همچنان می‌توانند پس از خارج شدن `Box` از دامنه و آزادسازی جایگاه متناظر حافظه هیپ، ارجاع‌ها را نگه دارند:

```rust
let x = {
    let z = Box::new([1,2,3]);
    &z[1]
}; // z goes out of scope and `deallocate` is called
println!("{}", x);
```

اینجاست که مالکیت Rust وارد عمل می‌شود. این سیستم به هر ارجاع یک [طول عمر] انتزاعی نسبت می‌دهد، که همان دامنه‌ای است که ارجاع در آن معتبر است. در مثال بالا، ارجاع `x` از آرایه `z` گرفته شده است، بنابراین پس از خارج شدن `z` از دامنه نامعتبر می‌شود. وقتی [مثال بالا را در playground اجرا کنید][playground-2]، می‌بینید که کامپایلر Rust واقعاً یک خطا می‌دهد:

[طول عمر]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
[playground-2]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=28180d8de7b62c6b4a681a7b1f745a48

```
error[E0597]: `z[_]` does not live long enough
 --> src/main.rs:4:9
  |
2 |     let x = {
  |         - borrow later stored here
3 |         let z = Box::new([1,2,3]);
  |             - binding `z` declared here
4 |         &z[1]
  |         ^^^^^ borrowed value does not live long enough
5 |     }; // z goes out of scope and `deallocate` is called
  |     - `z[_]` dropped here while still borrowed
```

این اصطلاحات ممکن است در ابتدا کمی گیج‌کننده باشند. گرفتن یک ارجاع به یک مقدار، _قرض گرفتن_ (borrowing) آن مقدار نامیده می‌شود، زیرا شبیه قرض گرفتن در زندگی واقعی است: شما دسترسی موقت به یک شیء دارید اما باید آن را در زمانی بازگردانید و نباید آن را نابود کنید. کامپایلر Rust با بررسی این‌که همه قرض‌ها پیش از نابود شدن یک شیء پایان می‌یابند، می‌تواند تضمین کند که هیچ وضعیت استفاده-پس-از-آزادسازی نمی‌تواند رخ دهد.

سیستم مالکیت Rust حتی فراتر می‌رود و نه‌تنها از باگ‌های استفاده-پس-از-آزادسازی جلوگیری می‌کند، بلکه [_ایمنی حافظه_] کامل را نیز فراهم می‌کند، همان‌طور که زبان‌های دارای جمع‌آوری زباله مانند جاوا یا پایتون این کار را می‌کنند. علاوه بر این، [_ایمنی نخ_] را تضمین می‌کند و بنابراین در کد چندنخی حتی از آن زبان‌ها هم ایمن‌تر است. و از همه مهم‌تر، تمام این بررسی‌ها در زمان کامپایل انجام می‌شوند، بنابراین در مقایسه با مدیریت حافظه دست‌نویس در C هیچ سربار زمان اجرا وجود ندارد.

[_ایمنی حافظه_]: https://en.wikipedia.org/wiki/Memory_safety
[_ایمنی نخ_]: https://en.wikipedia.org/wiki/Thread_safety

### موارد استفاده

اکنون اصول تخصیص پویای حافظه در Rust را می‌دانیم، اما چه زمانی باید از آن استفاده کنیم؟ ما با هسته خود بدون تخصیص پویای حافظه واقعاً راه زیادی آمده‌ایم، پس چرا اکنون به آن نیاز داریم؟

اول این‌که، تخصیص پویای حافظه همیشه کمی سربار کارایی به همراه دارد، زیرا برای هر تخصیص باید یک جایگاه آزاد روی هیپ پیدا کنیم. به همین دلیل، متغیرهای محلی عموماً ترجیح داده می‌شوند، به‌ویژه در کد حساس به کاراییِ هسته. با این حال، مواردی وجود دارد که تخصیص پویای حافظه بهترین انتخاب است.

به‌عنوان یک قاعده کلی، حافظه پویا برای متغیرهایی لازم است که طول عمر پویا یا اندازه متغیر دارند. مهم‌ترین نوع با طول عمر پویا [**`Rc`**] است، که ارجاع‌ها به مقدار بسته‌بندی‌شده‌ی خود را می‌شمارد و پس از خارج شدن همه ارجاع‌ها از دامنه، آن را آزادسازی می‌کند. نمونه‌هایی از نوع‌های با اندازه متغیر عبارت‌اند از [**`Vec`**]، [**`String`**] و دیگر [نوع‌های مجموعه] که با افزوده شدن عناصر بیشتر به‌صورت پویا رشد می‌کنند. این نوع‌ها به این شکل کار می‌کنند که وقتی پر می‌شوند مقدار بیشتری حافظه تخصیص می‌دهند، همه عناصر را در آن کپی می‌کنند و سپس تخصیص قدیمی را آزادسازی می‌کنند.

[**`Rc`**]: https://doc.rust-lang.org/alloc/rc/index.html
[**`Vec`**]: https://doc.rust-lang.org/alloc/vec/index.html
[**`String`**]: https://doc.rust-lang.org/alloc/string/index.html
[نوع‌های مجموعه]: https://doc.rust-lang.org/alloc/collections/index.html

برای هسته ما، بیشتر به نوع‌های مجموعه نیاز خواهیم داشت، برای مثال برای ذخیره فهرستی از تسک‌های فعال هنگام پیاده‌سازی چندوظیفگی در پست‌های آینده.

## رابط تخصیص‌دهنده

اولین گام در پیاده‌سازی یک تخصیص‌دهنده هیپ، افزودن وابستگی به کِرِیت داخلی [`alloc`] است. مانند کِرِیت [`core`]، این هم زیرمجموعه‌ای از کتابخانه استاندارد است که علاوه بر آن شامل نوع‌های تخصیص و مجموعه نیز می‌شود. برای افزودن وابستگی به `alloc`، موارد زیر را به `lib.rs` خود اضافه می‌کنیم:

[`alloc`]: https://doc.rust-lang.org/alloc/
[`core`]: https://doc.rust-lang.org/core/

```rust
// in src/lib.rs

extern crate alloc;
```

برخلاف وابستگی‌های معمولی، نیازی به تغییر `Cargo.toml` نداریم. دلیلش این است که کِرِیت `alloc` به‌عنوان بخشی از کتابخانه استاندارد همراه کامپایلر Rust عرضه می‌شود، بنابراین کامپایلر از پیش این کِرِیت را می‌شناسد. با افزودن این دستور `extern crate`، مشخص می‌کنیم که کامپایلر باید تلاش کند آن را وارد کند. (از نظر تاریخی، همه وابستگی‌ها به یک دستور `extern crate` نیاز داشتند، که اکنون اختیاری است).

از آن‌جا که برای یک هدف سفارشی کامپایل می‌کنیم، نمی‌توانیم از نسخه از پیش کامپایل‌شده‌ی `alloc` که همراه نصب Rust ارائه می‌شود استفاده کنیم. در عوض، باید به کارگو بگوییم که این کِرِیت را از روی کد منبع دوباره کامپایل کند. می‌توانیم این کار را با افزودن آن به آرایه `unstable.build-std` در فایل `.cargo/config.toml` خود انجام دهیم:

```toml
# in .cargo/config.toml

[unstable]
build-std = ["core", "compiler_builtins", "alloc"]
```

اکنون کامپایلر کِرِیت `alloc` را دوباره کامپایل کرده و در هسته ما وارد می‌کند.

دلیل این‌که کِرِیت `alloc` به‌طور پیش‌فرض در کِرِیت‌های `#[no_std]` غیرفعال است، این است که پیش‌نیازهای اضافی دارد. وقتی اکنون سعی کنیم پروژه خود را کامپایل کنیم، این پیش‌نیازها را به‌صورت خطا خواهیم دید:

```
error: no global memory allocator found but one is required; link to std or add
       #[global_allocator] to a static item that implements the GlobalAlloc trait.
```

این خطا رخ می‌دهد زیرا کِرِیت `alloc` به یک تخصیص‌دهنده هیپ نیاز دارد، که شیئی است که توابع `allocate` و `deallocate` را فراهم می‌کند. در Rust، تخصیص‌دهنده‌های هیپ توسط تِرِیت [`GlobalAlloc`] توصیف می‌شوند، که در پیام خطا به آن اشاره شده است. برای تعیین تخصیص‌دهنده هیپ برای کِرِیت، باید ویژگی `#[global_allocator]` روی یک متغیر `static` که تِرِیت `GlobalAlloc` را پیاده‌سازی می‌کند اعمال شود.

[`GlobalAlloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html

### تِرِیت `GlobalAlloc`

تِرِیت [`GlobalAlloc`] توابعی را تعریف می‌کند که یک تخصیص‌دهنده هیپ باید فراهم کند. این تِرِیت ویژه است، زیرا تقریباً هرگز مستقیماً توسط برنامه‌نویس استفاده نمی‌شود. در عوض، کامپایلر هنگام استفاده از نوع‌های تخصیص و مجموعه‌ی `alloc` به‌طور خودکار فراخوانی‌های مناسب متدهای این تِرِیت را درج می‌کند.

از آن‌جا که باید این تِرِیت را برای همه نوع‌های تخصیص‌دهنده خود پیاده‌سازی کنیم، ارزش دارد نگاه دقیق‌تری به تعریف آن بیندازیم:

```rust
pub unsafe trait GlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 { ... }
    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize
    ) -> *mut u8 { ... }
}
```

این تِرِیت دو متد الزامی [`alloc`] و [`dealloc`] را تعریف می‌کند، که با توابع `allocate` و `deallocate` که در مثال‌هایمان استفاده کردیم متناظرند:
- متد [`alloc`] یک نمونه از [`Layout`] را به‌عنوان آرگومان می‌گیرد، که اندازه و ترازبندی مورد نظری را که حافظه تخصیص‌یافته باید داشته باشد توصیف می‌کند. این متد یک [اشاره‌گر خام] به اولین بایتِ بلوک حافظه تخصیص‌یافته برمی‌گرداند. به‌جای یک مقدار خطای صریح، متد `alloc` برای اعلام خطای تخصیص یک اشاره‌گر تهی (null) برمی‌گرداند. این کمی غیراصولی است، اما این مزیت را دارد که بسته‌بندی تخصیص‌دهنده‌های موجودِ سیستم آسان می‌شود، زیرا آن‌ها از همین قرارداد استفاده می‌کنند.
- متد [`dealloc`] نقطه مقابل آن است و مسئول آزاد کردن دوباره یک بلوک حافظه است. این متد دو آرگومان دریافت می‌کند: اشاره‌گری که `alloc` برگردانده و `Layout`ای که برای آن تخصیص استفاده شده است.

[`alloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.alloc
[`dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc
[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html

این تِرِیت علاوه بر این، دو متد [`alloc_zeroed`] و [`realloc`] را با پیاده‌سازی‌های پیش‌فرض تعریف می‌کند:

- متد [`alloc_zeroed`] معادل فراخوانی `alloc` و سپس صفر کردن بلوک حافظه تخصیص‌یافته است، که دقیقاً همان کاری است که پیاده‌سازی پیش‌فرضِ ارائه‌شده انجام می‌دهد. یک پیاده‌سازی تخصیص‌دهنده می‌تواند در صورت امکان، پیاده‌سازی‌های پیش‌فرض را با یک پیاده‌سازی سفارشیِ کارآمدتر بازنویسی کند.
- متد [`realloc`] امکان بزرگ یا کوچک کردن یک تخصیص را فراهم می‌کند. پیاده‌سازی پیش‌فرض یک بلوک حافظه جدید با اندازه مورد نظر تخصیص می‌دهد و تمام محتوای تخصیص قبلی را در آن کپی می‌کند. باز هم، یک پیاده‌سازی تخصیص‌دهنده احتمالاً می‌تواند پیاده‌سازی کارآمدتری از این متد ارائه دهد، برای مثال با بزرگ/کوچک کردن تخصیص در همان محل، در صورت امکان.

[`alloc_zeroed`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#method.alloc_zeroed
[`realloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#method.realloc

#### ناایمنی

نکته‌ای که باید به آن توجه کرد این است که هم خودِ تِرِیت و هم تمام متدهای آن به‌صورت `unsafe` تعریف شده‌اند:

- دلیل تعریف تِرِیت به‌صورت `unsafe` این است که برنامه‌نویس باید تضمین کند که پیاده‌سازی تِرِیت برای یک نوع تخصیص‌دهنده درست است. برای مثال، متد `alloc` هرگز نباید بلوک حافظه‌ای را برگرداند که پیش‌تر جای دیگری استفاده شده است، زیرا این باعث رفتار تعریف‌نشده می‌شود.
- به‌همین ترتیب، دلیل `unsafe` بودن متدها این است که فراخواننده باید هنگام فراخوانی متدها ناوردایی‌های مختلفی را تضمین کند، برای مثال این‌که `Layout`ای که به `alloc` داده می‌شود اندازه‌ای غیرصفر مشخص کند. این در عمل چندان موضوعیت ندارد، زیرا این متدها معمولاً مستقیماً توسط کامپایلر فراخوانی می‌شوند، که خودش تضمین می‌کند این شرایط برآورده شوند.

### یک `DummyAllocator`

اکنون که می‌دانیم یک نوع تخصیص‌دهنده باید چه چیزی فراهم کند، می‌توانیم یک تخصیص‌دهنده ساختگی (dummy) ساده ایجاد کنیم. برای این کار، یک ماژول `allocator` جدید ایجاد می‌کنیم:

```rust
// in src/lib.rs

pub mod allocator;
```

تخصیص‌دهنده ساختگی ما حداقل کار ممکن را برای پیاده‌سازی تِرِیت انجام می‌دهد و هنگام فراخوانی `alloc` همیشه یک خطا برمی‌گرداند. به این شکل است:

```rust
// in src/allocator.rs

use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should be never called")
    }
}
```

این ساختار به هیچ فیلدی نیاز ندارد، بنابراین آن را به‌صورت یک [نوع با اندازه صفر] ایجاد می‌کنیم. همان‌طور که در بالا اشاره شد، ما همیشه اشاره‌گر تهی را از `alloc` برمی‌گردانیم، که متناظر با یک خطای تخصیص است. از آن‌جا که این تخصیص‌دهنده هرگز حافظه‌ای برنمی‌گرداند، فراخوانی `dealloc` هرگز نباید رخ دهد. به همین دلیل، در متد `dealloc` صرفاً پنیک می‌کنیم. متدهای `alloc_zeroed` و `realloc` پیاده‌سازی‌های پیش‌فرض دارند، بنابراین نیازی نیست برای آن‌ها پیاده‌سازی ارائه دهیم.

[نوع با اندازه صفر]: https://doc.rust-lang.org/nomicon/exotic-sizes.html#zero-sized-types-zsts

اکنون یک تخصیص‌دهنده ساده داریم، اما هنوز باید به کامپایلر Rust بگوییم که باید از این تخصیص‌دهنده استفاده کند. اینجاست که ویژگی `#[global_allocator]` وارد عمل می‌شود.

### ویژگی `#[global_allocator]`

ویژگی `#[global_allocator]` به کامپایلر Rust می‌گوید که باید از کدام نمونه تخصیص‌دهنده به‌عنوان تخصیص‌دهنده سراسری هیپ استفاده کند. این ویژگی تنها روی یک `static` که تِرِیت `GlobalAlloc` را پیاده‌سازی می‌کند قابل اعمال است. بیایید یک نمونه از تخصیص‌دهنده `Dummy` خود را به‌عنوان تخصیص‌دهنده سراسری ثبت کنیم:

```rust
// in src/allocator.rs

#[global_allocator]
static ALLOCATOR: Dummy = Dummy;
```

از آن‌جا که تخصیص‌دهنده `Dummy` یک [نوع با اندازه صفر] است، نیازی نیست در عبارت مقداردهی اولیه هیچ فیلدی مشخص کنیم.

با این استاتیک، خطاهای کامپایل باید برطرف شده باشند. اکنون می‌توانیم از نوع‌های تخصیص و مجموعه‌ی `alloc` استفاده کنیم. برای مثال، می‌توانیم از یک [`Box`] برای تخصیص یک مقدار روی هیپ استفاده کنیم:

[`Box`]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html

```rust
// in src/main.rs

extern crate alloc;

use alloc::boxed::Box;

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] print "Hello World!", call `init`, create `mapper` and `frame_allocator`

    let x = Box::new(41);

    // […] call `test_main` in test mode

    println!("It did not crash!");
    blog_os::hlt_loop();
}

```

توجه کنید که باید دستور `extern crate alloc` را در `main.rs` خود نیز مشخص کنیم. این کار لازم است زیرا بخش‌های `lib.rs` و `main.rs` به‌عنوان کِرِیت‌های جداگانه در نظر گرفته می‌شوند. با این حال، نیازی به ایجاد یک استاتیک `#[global_allocator]` دیگر نداریم، زیرا تخصیص‌دهنده سراسری برای همه کِرِیت‌های پروژه اعمال می‌شود. در واقع، مشخص کردن یک تخصیص‌دهنده اضافی در کِرِیتی دیگر یک خطا خواهد بود.

وقتی کد بالا را اجرا می‌کنیم، می‌بینیم که یک پنیک رخ می‌دهد:

![QEMU printing "panicked at `allocation error: Layout { size_: 4, align_: 4 }, src/lib.rs:89:5"](qemu-dummy-output.png)

این پنیک رخ می‌دهد زیرا تابع `Box::new` به‌طور ضمنی تابع `alloc` تخصیص‌دهنده سراسری را فراخوانی می‌کند. تخصیص‌دهنده ساختگی ما همیشه یک اشاره‌گر تهی برمی‌گرداند، بنابراین هر تخصیصی شکست می‌خورد. برای رفع این مشکل، باید تخصیص‌دهنده‌ای ایجاد کنیم که واقعاً حافظه‌ی قابل استفاده برگرداند.

## ایجاد یک هیپ برای هسته

پیش از آن‌که بتوانیم یک تخصیص‌دهنده واقعی ایجاد کنیم، ابتدا باید یک ناحیه حافظه هیپ بسازیم که تخصیص‌دهنده بتواند از آن حافظه تخصیص دهد. برای این کار، باید یک محدوده حافظه مجازی برای ناحیه هیپ تعریف کنیم و سپس این ناحیه را به قاب‌های فیزیکی نگاشت کنیم. برای مروری بر حافظه مجازی و جدول‌های صفحه، به پست [_«مقدمه‌ای بر صفحه‌بندی»_][_"Introduction To Paging"_] مراجعه کنید.

[_"Introduction To Paging"_]: @/edition-2/posts/08-paging-introduction/index.fa.md

گام اول تعریف یک ناحیه حافظه مجازی برای هیپ است. می‌توانیم هر محدوده آدرس مجازی که دوست داریم انتخاب کنیم، تا زمانی که از قبل برای ناحیه حافظه دیگری استفاده نشده باشد. بیایید آن را به‌صورت حافظه‌ای تعریف کنیم که از آدرس `0x_4444_4444_0000` شروع می‌شود، تا بعداً بتوانیم به‌راحتی یک اشاره‌گر هیپ را تشخیص دهیم:

```rust
// in src/allocator.rs

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB
```

فعلاً اندازه هیپ را روی 100&nbsp;KiB تنظیم می‌کنیم. اگر در آینده به فضای بیشتری نیاز داشتیم، می‌توانیم به‌سادگی آن را افزایش دهیم.

اگر اکنون سعی کنیم از این ناحیه هیپ استفاده کنیم، یک خطای صفحه رخ می‌دهد، زیرا این ناحیه حافظه مجازی هنوز به حافظه فیزیکی نگاشت نشده است. برای حل این مشکل، یک تابع `init_heap` ایجاد می‌کنیم که صفحه‌های هیپ را با استفاده از [API `Mapper`][`Mapper` API] که در پست [_«پیاده‌سازی صفحه‌بندی»_][_"Paging Implementation"_] معرفی کردیم نگاشت می‌کند:

[`Mapper` API]: @/edition-2/posts/09-paging-implementation/index.md#using-offsetpagetable
[_"Paging Implementation"_]: @/edition-2/posts/09-paging-implementation/index.fa.md

```rust
// in src/allocator.rs

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        };
    }

    Ok(())
}
```

این تابع ارجاع‌های قابل تغییر به یک نمونه [`Mapper`] و یک نمونه [`FrameAllocator`] می‌گیرد، که هر دو با استفاده از [`Size4KiB`] به‌عنوان پارامتر جنریک به صفحه‌های 4&nbsp;KiB محدود شده‌اند. مقدار بازگشتی تابع یک [`Result`] است که نوع واحد `()` را به‌عنوان حالت موفقیت و یک [`MapToError`] را به‌عنوان حالت خطا دارد، که همان نوع خطایی است که متد [`Mapper::map_to`] برمی‌گرداند. استفاده مجدد از این نوع خطا در اینجا منطقی است، زیرا متد `map_to` منبع اصلی خطاها در این تابع است.

[`Mapper`]:https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/mapper/trait.Mapper.html
[`FrameAllocator`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/trait.FrameAllocator.html
[`Size4KiB`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/page/enum.Size4KiB.html
[`Result`]: https://doc.rust-lang.org/core/result/enum.Result.html
[`MapToError`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/mapper/enum.MapToError.html
[`Mapper::map_to`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/mapper/trait.Mapper.html#method.map_to

پیاده‌سازی را می‌توان به دو بخش تقسیم کرد:

- **ایجاد محدوده صفحه‌ها:**: برای ساختن محدوده‌ای از صفحه‌هایی که می‌خواهیم نگاشت کنیم، اشاره‌گر `HEAP_START` را به نوع [`VirtAddr`] تبدیل می‌کنیم. سپس آدرس پایان هیپ را با افزودن `HEAP_SIZE` به آن محاسبه می‌کنیم. ما یک کرانِ شامل (آدرس آخرین بایت هیپ) می‌خواهیم، بنابراین 1 واحد کم می‌کنیم. در ادامه، آدرس‌ها را با استفاده از تابع [`containing_address`] به نوع [`Page`] تبدیل می‌کنیم. در نهایت، با استفاده از تابع [`Page::range_inclusive`] یک محدوده صفحه از صفحه‌های شروع و پایان می‌سازیم.

- **نگاشت صفحه‌ها:** گام دوم نگاشت کردن همه صفحه‌های محدوده‌ای است که تازه ساختیم. برای این کار، با یک حلقه `for` روی این صفحه‌ها پیمایش می‌کنیم. برای هر صفحه، کارهای زیر را انجام می‌دهیم:

    - یک قاب فیزیکی که صفحه باید به آن نگاشت شود را با استفاده از متد [`FrameAllocator::allocate_frame`] تخصیص می‌دهیم. این متد وقتی هیچ قابی باقی نمانده باشد [`None`] برمی‌گرداند. ما این حالت را با نگاشت آن به خطای [`MapToError::FrameAllocationFailed`] از طریق متد [`Option::ok_or`] و سپس اعمال [عملگر علامت سؤال] برای بازگشت زودهنگام در صورت بروز خطا مدیریت می‌کنیم.

    - پرچم لازمِ `PRESENT` و پرچم `WRITABLE` را برای صفحه تنظیم می‌کنیم. با این پرچم‌ها، هم دسترسی خواندن و هم دسترسی نوشتن مجاز است، که برای حافظه هیپ منطقی است.

    - از متد [`Mapper::map_to`] برای ایجاد نگاشت در جدول صفحه فعال استفاده می‌کنیم. این متد می‌تواند شکست بخورد، بنابراین دوباره از [عملگر علامت سؤال] استفاده می‌کنیم تا خطا را به فراخواننده منتقل کنیم. در صورت موفقیت، این متد یک نمونه [`MapperFlush`] برمی‌گرداند که می‌توانیم از آن برای به‌روزرسانی [_بافر جانبی ترجمه (TLB)_] با استفاده از متد [`flush`] بهره ببریم.

[`VirtAddr`]: https://docs.rs/x86_64/0.15.5/x86_64/addr/struct.VirtAddr.html
[`Page`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/page/struct.Page.html
[`containing_address`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/page/struct.Page.html#method.containing_address
[`Page::range_inclusive`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/page/struct.Page.html#method.range_inclusive
[`FrameAllocator::allocate_frame`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/trait.FrameAllocator.html#tymethod.allocate_frame
[`None`]: https://doc.rust-lang.org/core/option/enum.Option.html#variant.None
[`MapToError::FrameAllocationFailed`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/mapper/enum.MapToError.html#variant.FrameAllocationFailed
[`Option::ok_or`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.ok_or
[عملگر علامت سؤال]: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html
[`MapperFlush`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/mapper/struct.MapperFlush.html
[_بافر جانبی ترجمه (TLB)_]: @/edition-2/posts/08-paging-introduction/index.md#the-translation-lookaside-buffer
[`flush`]: https://docs.rs/x86_64/0.15.5/x86_64/structures/paging/mapper/struct.MapperFlush.html#method.flush

گام آخر، فراخوانی این تابع از `kernel_main` ماست:

```rust
// in src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::allocator; // new import
    use blog_os::memory::{self, BootInfoFrameAllocator};

    println!("Hello World{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    // new
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    let x = Box::new(41);

    // […] call `test_main` in test mode

    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

ما در اینجا تابع کامل را برای درک بهتر زمینه نشان می‌دهیم. تنها خطوط جدید، وارد کردن `blog_os::allocator` و فراخوانی تابع `allocator::init_heap` هستند. در صورتی که تابع `init_heap` خطایی برگرداند، با استفاده از متد [`Result::expect`] پنیک می‌کنیم، زیرا در حال حاضر راه معقولی برای مدیریت این خطا نداریم.

[`Result::expect`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.expect

اکنون یک ناحیه حافظه هیپِ نگاشت‌شده داریم که آماده استفاده است. فراخوانی `Box::new` هنوز از تخصیص‌دهنده قدیمی `Dummy` ما استفاده می‌کند، بنابراین هنگام اجرا همچنان خطای «کمبود حافظه» (out of memory) را خواهید دید. بیایید این را با استفاده از یک تخصیص‌دهنده واقعی برطرف کنیم.

## استفاده از یک کِرِیت تخصیص‌دهنده

از آن‌جا که پیاده‌سازی یک تخصیص‌دهنده تا حدی پیچیده است، کار را با استفاده از یک کِرِیت تخصیص‌دهنده خارجی آغاز می‌کنیم. در پست بعدی یاد می‌گیریم که چگونه تخصیص‌دهنده خودمان را پیاده‌سازی کنیم.

یک کِرِیت تخصیص‌دهنده ساده برای برنامه‌های `no_std`، کِرِیت [`linked_list_allocator`] است. نام آن از این واقعیت می‌آید که برای پیگیری ناحیه‌های حافظه آزادشده از یک ساختار داده لیست پیوندی استفاده می‌کند. برای توضیح مفصل‌تر این رویکرد به پست بعدی مراجعه کنید.

برای استفاده از این کِرِیت، ابتدا باید یک وابستگی به آن در `Cargo.toml` خود اضافه کنیم:

[`linked_list_allocator`]: https://github.com/phil-opp/linked-list-allocator/

```toml
# in Cargo.toml

[dependencies]
linked_list_allocator = "0.9.0"
```

سپس می‌توانیم تخصیص‌دهنده ساختگی خود را با تخصیص‌دهنده‌ای که این کِرِیت فراهم می‌کند جایگزین کنیم:

```rust
// in src/allocator.rs

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();
```

این ساختار `LockedHeap` نام دارد، زیرا برای همگام‌سازی از نوع [`spinning_top::Spinlock`] استفاده می‌کند. این کار لازم است زیرا چندین نخ می‌توانند هم‌زمان به استاتیک `ALLOCATOR` دسترسی داشته باشند. مثل همیشه، هنگام استفاده از spinlock یا mutex باید مراقب باشیم که به‌طور تصادفی باعث بن‌بست (deadlock) نشویم. این یعنی نباید در کنترل‌کننده‌های وقفه هیچ تخصیصی انجام دهیم، زیرا آن‌ها می‌توانند در هر زمان دلخواهی اجرا شوند و ممکن است یک تخصیصِ در حال انجام را قطع کنند.

[`spinning_top::Spinlock`]: https://docs.rs/spinning_top/0.1.0/spinning_top/type.Spinlock.html

تنظیم `LockedHeap` به‌عنوان تخصیص‌دهنده سراسری کافی نیست. دلیلش این است که ما از تابع سازنده [`empty`] استفاده می‌کنیم، که یک تخصیص‌دهنده بدون هیچ حافظه پشتیبانی ایجاد می‌کند. مانند تخصیص‌دهنده ساختگی ما، این هم همیشه در `alloc` خطا برمی‌گرداند. برای رفع این مشکل، باید پس از ایجاد هیپ، تخصیص‌دهنده را مقداردهی اولیه کنیم:

[`empty`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.LockedHeap.html#method.empty

```rust
// in src/allocator.rs

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // […] map all heap pages to physical frames

    // new
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
```

ما از متد [`lock`] روی spinlock داخلی نوع `LockedHeap` استفاده می‌کنیم تا یک ارجاع انحصاری به نمونه [`Heap`] بسته‌بندی‌شده به دست آوریم، و سپس متد [`init`] را با کران‌های هیپ به‌عنوان آرگومان روی آن فراخوانی می‌کنیم. از آن‌جا که تابع [`init`] همان‌جا تلاش می‌کند در حافظه هیپ بنویسد، باید هیپ را تنها _پس از_ نگاشت صفحه‌های هیپ مقداردهی اولیه کنیم.

[`lock`]: https://docs.rs/lock_api/0.3.3/lock_api/struct.Mutex.html#method.lock
[`Heap`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html
[`init`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.init

پس از مقداردهی اولیه هیپ، اکنون می‌توانیم از تمام نوع‌های تخصیص و مجموعه‌ی کِرِیت داخلی [`alloc`] بدون خطا استفاده کنیم:

```rust
// in src/main.rs

use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] initialize interrupts, mapper, frame_allocator, heap

    // allocate a number on the heap
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    // create a dynamically sized vector
    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    // create a reference counted vector -> will be freed when count reaches 0
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!("current reference count is {}", Rc::strong_count(&cloned_reference));
    core::mem::drop(reference_counted);
    println!("reference count is {} now", Rc::strong_count(&cloned_reference));

    // […] call `test_main` in test context
    println!("It did not crash!");
    blog_os::hlt_loop();
}
```

این نمونه کد چند کاربرد از نوع‌های [`Box`]، [`Vec`] و [`Rc`] را نشان می‌دهد. برای نوع‌های `Box` و `Vec`، اشاره‌گرهای زیرینِ هیپ را با استفاده از [مشخص‌کننده قالب‌بندی `{:p}`][`{:p}` formatting specifier] چاپ می‌کنیم. برای نمایش `Rc`، یک مقدار هیپ با شمارش ارجاع ایجاد می‌کنیم و از تابع [`Rc::strong_count`] برای چاپ شمار ارجاع فعلی، پیش و پس از دراپ کردن یک نمونه (با استفاده از [`core::mem::drop`])، بهره می‌بریم.

[`Vec`]: https://doc.rust-lang.org/alloc/vec/
[`Rc`]: https://doc.rust-lang.org/alloc/rc/
[`{:p}` formatting specifier]: https://doc.rust-lang.org/core/fmt/trait.Pointer.html
[`Rc::strong_count`]: https://doc.rust-lang.org/alloc/rc/struct.Rc.html#method.strong_count
[`core::mem::drop`]: https://doc.rust-lang.org/core/mem/fn.drop.html

وقتی آن را اجرا می‌کنیم، موارد زیر را می‌بینیم:

![QEMU printing `
heap_value at 0x444444440000
vec at 0x4444444408000
current reference count is 2
reference count is 1 now
](qemu-alloc-showcase.png)

همان‌طور که انتظار می‌رفت، می‌بینیم که مقادیر `Box` و `Vec` روی هیپ قرار دارند، که با اشاره‌گری که با پیشوند `0x_4444_4444_*` شروع می‌شود نشان داده می‌شود. مقدارِ دارای شمارش ارجاع نیز مطابق انتظار رفتار می‌کند: شمار ارجاع پس از فراخوانی `clone` برابر 2 است و پس از دراپ شدن یکی از نمونه‌ها دوباره 1 می‌شود.

دلیل این‌که بردار از آفست `0x800` شروع می‌شود این نیست که مقدار درون `Box` به اندازه `0x800` بایت است، بلکه [تخصیص‌های مجدد] هستند که هنگام نیاز بردار به افزایش ظرفیت خود رخ می‌دهند. برای مثال، وقتی ظرفیت بردار 32 است و سعی می‌کنیم عنصر بعدی را اضافه کنیم، بردار در پشت صحنه یک آرایه پشتیبان جدید با ظرفیت 64 تخصیص می‌دهد و همه عناصر را در آن کپی می‌کند. سپس تخصیص قدیمی را آزاد می‌کند.

[تخصیص‌های مجدد]: https://doc.rust-lang.org/alloc/vec/struct.Vec.html#capacity-and-reallocation

البته نوع‌های تخصیص و مجموعه‌ی بسیار بیشتری در کِرِیت `alloc` وجود دارند که اکنون می‌توانیم همه آن‌ها را در هسته خود به کار ببریم، از جمله:

- اشاره‌گر با شمارش ارجاعِ امن برای نخ‌ها، [`Arc`]
- نوع رشته‌ی مالک‌دار [`String`] و ماکرو [`format!`]
- [`LinkedList`]
- بافر حلقوی قابل رشد [`VecDeque`]
- صف اولویت [`BinaryHeap`]
- [`BTreeMap`] و [`BTreeSet`]

[`Arc`]: https://doc.rust-lang.org/alloc/sync/struct.Arc.html
[`String`]: https://doc.rust-lang.org/alloc/string/struct.String.html
[`format!`]: https://doc.rust-lang.org/alloc/macro.format.html
[`LinkedList`]: https://doc.rust-lang.org/alloc/collections/linked_list/struct.LinkedList.html
[`VecDeque`]: https://doc.rust-lang.org/alloc/collections/vec_deque/struct.VecDeque.html
[`BinaryHeap`]: https://doc.rust-lang.org/alloc/collections/binary_heap/struct.BinaryHeap.html
[`BTreeMap`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html
[`BTreeSet`]: https://doc.rust-lang.org/alloc/collections/btree_set/struct.BTreeSet.html

این نوع‌ها زمانی که بخواهیم فهرست‌های نخ، صف‌های زمان‌بندی یا پشتیبانی از async/await را پیاده‌سازی کنیم بسیار مفید خواهند بود.

## افزودن یک تست

برای اطمینان از این‌که به‌طور تصادفی کد تخصیص جدیدمان را خراب نمی‌کنیم، باید یک تست یکپارچه برای آن اضافه کنیم. کار را با ایجاد یک فایل جدید `tests/heap_allocation.rs` با محتوای زیر آغاز می‌کنیم:

```rust
// in tests/heap_allocation.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

ما از توابع `test_runner` و `test_panic_handler` موجود در `lib.rs` خود دوباره استفاده می‌کنیم. از آن‌جا که می‌خواهیم تخصیص‌ها را تست کنیم، کِرِیت `alloc` را از طریق دستور `extern crate alloc` فعال می‌کنیم. برای اطلاعات بیشتر درباره قالب آماده‌ی تست، پست [_تست کردن_][_Testing_] را ببینید.

[_Testing_]: @/edition-2/posts/04-testing/index.fa.md

پیاده‌سازی تابع `main` به این شکل است:

```rust
// in tests/heap_allocation.rs

fn main(boot_info: &'static BootInfo) -> ! {
    use blog_os::allocator;
    use blog_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    blog_os::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    test_main();
    loop {}
}
```

این تابع بسیار شبیه تابع `kernel_main` در `main.rs` ماست، با این تفاوت‌ها که `println` را فراخوانی نمی‌کنیم، هیچ تخصیص نمونه‌ای را وارد نمی‌کنیم و `test_main` را بدون شرط فراخوانی می‌کنیم.

اکنون آماده‌ایم چند مورد تست اضافه کنیم. ابتدا تستی اضافه می‌کنیم که با استفاده از [`Box`] چند تخصیص ساده انجام می‌دهد و مقادیر تخصیص‌یافته را بررسی می‌کند تا از کارکرد درست تخصیص‌های پایه اطمینان حاصل شود:

```rust
// in tests/heap_allocation.rs
use alloc::boxed::Box;

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}
```

از همه مهم‌تر، این تست تأیید می‌کند که هیچ خطای تخصیصی رخ نمی‌دهد.

در ادامه، به‌صورت تکرارشونده یک بردار بزرگ می‌سازیم تا هم تخصیص‌های بزرگ و هم تخصیص‌های متعدد (به‌دلیل تخصیص‌های مجدد) را تست کنیم:

```rust
// in tests/heap_allocation.rs

use alloc::vec::Vec;

#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}
```

مجموع را با مقایسه آن با فرمول [مجموع جزئی n-اُم] راستی‌آزمایی می‌کنیم. این تا حدی به ما اطمینان می‌دهد که همه مقادیر تخصیص‌یافته درست هستند.

[مجموع جزئی n-اُم]: https://en.wikipedia.org/wiki/1_%2B_2_%2B_3_%2B_4_%2B_%E2%8B%AF#Partial_sums

به‌عنوان تست سوم، ده هزار تخصیص را یکی پس از دیگری ایجاد می‌کنیم:

```rust
// in tests/heap_allocation.rs

use blog_os::allocator::HEAP_SIZE;

#[test_case]
fn many_boxes() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}
```

این تست تضمین می‌کند که تخصیص‌دهنده حافظه آزادشده را برای تخصیص‌های بعدی دوباره استفاده می‌کند، زیرا در غیر این‌صورت حافظه‌اش تمام می‌شد. این ممکن است پیش‌نیازی بدیهی برای یک تخصیص‌دهنده به نظر برسد، اما طراحی‌هایی از تخصیص‌دهنده وجود دارند که این کار را انجام نمی‌دهند. نمونه‌ای از آن، طراحی تخصیص‌دهنده bump است که در پست بعدی توضیح داده خواهد شد.

بیایید تست یکپارچه جدیدمان را اجرا کنیم:

```
> cargo test --test heap_allocation
[…]
Running 3 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
```

هر سه تست موفق شدند! همچنین می‌توانید `cargo test` را (بدون آرگومان `--test`) فراخوانی کنید تا همه تست‌های واحد و یکپارچه اجرا شوند.

## خلاصه

این پست مقدمه‌ای بر حافظه پویا ارائه داد و توضیح داد که چرا و کجا به آن نیاز است. دیدیم که چگونه بررسی‌کننده قرض Rust از آسیب‌پذیری‌های رایج جلوگیری می‌کند و یاد گرفتیم که API تخصیص در Rust چگونه کار می‌کند.

پس از ایجاد یک پیاده‌سازی مینیمال از رابط تخصیص‌دهنده Rust با استفاده از یک تخصیص‌دهنده ساختگی، یک ناحیه حافظه هیپ واقعی برای هسته خود ایجاد کردیم. برای این کار، یک محدوده آدرس مجازی برای هیپ تعریف کردیم و سپس همه صفحه‌های آن محدوده را با استفاده از `Mapper` و `FrameAllocator` از پست قبلی به قاب‌های فیزیکی نگاشت کردیم.

در نهایت، یک وابستگی به کِرِیت `linked_list_allocator` اضافه کردیم تا یک تخصیص‌دهنده واقعی به هسته خود بیفزاییم. با این تخصیص‌دهنده، توانستیم از `Box`، `Vec` و دیگر نوع‌های تخصیص و مجموعه‌ی کِرِیت `alloc` استفاده کنیم.

## بعدی چیست؟

اگرچه در این پست پشتیبانی از تخصیص هیپ را اضافه کردیم، اما بیشتر کار را به کِرِیت `linked_list_allocator` واگذار کردیم. پست بعدی با جزئیات نشان می‌دهد که چگونه می‌توان یک تخصیص‌دهنده را از صفر پیاده‌سازی کرد. این پست چندین طراحی ممکن برای تخصیص‌دهنده را ارائه می‌دهد، نشان می‌دهد که چگونه نسخه‌های ساده‌ای از آن‌ها را پیاده‌سازی کنیم، و مزایا و معایب آن‌ها را توضیح می‌دهد.
