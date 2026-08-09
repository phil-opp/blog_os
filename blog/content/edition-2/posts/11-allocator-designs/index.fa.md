+++
title = "طراحی تخصیص‌دهنده‌ها"
weight = 11
path = "fa/allocator-designs"
date = 2020-01-20

[extra]
# Please update this when updating the translation
translation_based_on_commit = "c850d4237ddf040e062a46404dee7dbae8c96b1c"
rtl = true
+++

این پست توضیح می‌دهد که چگونه می‌توان تخصیص‌دهنده‌های هیپ را از صفر پیاده‌سازی کرد. در این پست طراحی‌های مختلف تخصیص‌دهنده، از جمله تخصیص افزایشی (bump allocation)، تخصیص با لیست پیوندی و تخصیص بلوک با اندازه ثابت، معرفی و بررسی می‌شوند. برای هر یک از این سه طراحی، یک پیاده‌سازی پایه ایجاد می‌کنیم که می‌توان از آن برای هسته خود استفاده کرد.

<!-- more -->

این بلاگ بصورت آزاد روی [گیت‌هاب] توسعه داده شده است. اگر شما مشکل یا سوالی دارید، لطفاً آن‌جا یک ایشو باز کنید. شما همچنین می‌توانید [در زیر] این پست کامنت بگذارید. منبع کد کامل این پست را می‌توانید در بِرَنچ [`post-11`][post branch] پیدا کنید.

[گیت‌هاب]: https://github.com/phil-opp/blog_os
[در زیر]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-11

<!-- toc -->

## مقدمه

در [پست قبلی]، پشتیبانی پایه از تخصیص‌های هیپ را به هسته خود اضافه کردیم. برای این کار، در جدول‌های صفحه [یک ناحیه حافظه جدید ایجاد کردیم][map-heap] و برای مدیریت آن حافظه [از کِرِیت `linked_list_allocator` استفاده کردیم][use-alloc-crate]. اگرچه اکنون یک هیپ کارآمد داریم، بیشتر کار را به کِرِیت تخصیص‌دهنده واگذار کردیم، بدون این‌که تلاش کنیم بفهمیم چگونه کار می‌کند.

[پست قبلی]: @/edition-2/posts/10-heap-allocation/index.fa.md
[map-heap]: @/edition-2/posts/10-heap-allocation/index.md#creating-a-kernel-heap
[use-alloc-crate]: @/edition-2/posts/10-heap-allocation/index.md#using-an-allocator-crate

در این پست نشان می‌دهیم که چگونه به جای تکیه بر یک کِرِیت تخصیص‌دهنده موجود، تخصیص‌دهنده هیپ خودمان را از صفر بسازیم. طراحی‌های مختلف تخصیص‌دهنده، از جمله یک _تخصیص‌دهنده افزایشی_ ساده و یک _تخصیص‌دهنده بلوک با اندازه ثابت_ پایه را بررسی می‌کنیم و از این دانش برای پیاده‌سازی تخصیص‌دهنده‌ای با کارایی بهتر (در مقایسه با کِرِیت `linked_list_allocator`) استفاده می‌کنیم.

### اهداف طراحی

وظیفه یک تخصیص‌دهنده، مدیریت حافظه هیپ در دسترس است. تخصیص‌دهنده باید در فراخوانی‌های `alloc` حافظه استفاده‌نشده را برگرداند و حافظه‌ای را که با `dealloc` آزاد می‌شود پیگیری کند تا بتوان دوباره از آن استفاده کرد. از همه مهم‌تر، هرگز نباید حافظه‌ای را تحویل دهد که هم‌اکنون در جای دیگری در حال استفاده است، زیرا این کار باعث رفتار تعریف‌نشده می‌شود.

جدا از درستی، اهداف طراحی ثانویه زیادی نیز وجود دارد. برای مثال، تخصیص‌دهنده باید از حافظه در دسترس به‌طور مؤثر استفاده کند و [_تکه‌تکه شدن_] را کم نگه دارد. علاوه بر این، باید برای برنامه‌های همروند به‌خوبی کار کند و به هر تعداد پردازنده مقیاس‌پذیر باشد. برای رسیدن به حداکثر کارایی، حتی می‌تواند چیدمان حافظه را با توجه به حافظه‌های پنهان CPU بهینه کند تا [محلیت حافظه پنهان] بهبود یابد و از [اشتراک‌گذاری کاذب] جلوگیری شود.

[محلیت حافظه پنهان]: https://www.geeksforgeeks.org/locality-of-reference-and-cache-operation-in-cache-memory/
[_تکه‌تکه شدن_]: https://en.wikipedia.org/wiki/Fragmentation_(computing)
[اشتراک‌گذاری کاذب]: https://mechanical-sympathy.blogspot.de/2011/07/false-sharing.html

این نیازمندی‌ها می‌توانند تخصیص‌دهنده‌های خوب را بسیار پیچیده کنند. برای مثال، [jemalloc] بیش از 30.000 خط کد دارد. این پیچیدگی اغلب در کد هسته نامطلوب است، جایی که یک باگ می‌تواند به آسیب‌پذیری‌های امنیتی شدید منجر شود. خوشبختانه، الگوهای تخصیص در کد هسته اغلب بسیار ساده‌تر از کد فضای کاربر هستند، بنابراین طراحی‌های نسبتاً ساده تخصیص‌دهنده اغلب کفایت می‌کنند.

[jemalloc]: http://jemalloc.net/

در ادامه، سه طراحی ممکن برای تخصیص‌دهنده هسته را ارائه می‌دهیم و مزایا و معایب آن‌ها را توضیح می‌دهیم.

## تخصیص‌دهنده افزایشی

ساده‌ترین طراحی تخصیص‌دهنده، _تخصیص‌دهنده افزایشی_ (bump allocator) است که با نام _تخصیص‌دهنده پشته‌ای_ نیز شناخته می‌شود. این تخصیص‌دهنده حافظه را به‌صورت خطی تخصیص می‌دهد و تنها تعداد بایت‌های تخصیص‌داده‌شده و تعداد تخصیص‌ها را دنبال می‌کند. این طراحی تنها در موارد استفاده بسیار خاصی مفید است، زیرا محدودیت جدی‌ای دارد: تنها می‌تواند همه حافظه را یک‌جا آزاد کند.

### ایده

ایده پشت تخصیص‌دهنده افزایشی این است که حافظه به‌صورت خطی و با افزایش دادن (_«bump کردن»_) متغیری به نام `next` که به ابتدای حافظه استفاده‌نشده اشاره می‌کند، تخصیص داده شود. در ابتدا، `next` برابر با آدرس شروع هیپ است. در هر تخصیص، `next` به اندازه حجم تخصیص افزایش می‌یابد تا همیشه به مرز بین حافظه استفاده‌شده و استفاده‌نشده اشاره کند:

![The heap memory area at three points in time:
 1: A single allocation exists at the start of the heap; the `next` pointer points to its end.
 2: A second allocation was added right after the first; the `next` pointer points to the end of the second allocation.
 3: A third allocation was added right after the second one; the `next` pointer points to the end of the third allocation.](bump-allocation.svg)

اشاره‌گر `next` تنها در یک جهت حرکت می‌کند و بنابراین هرگز یک ناحیه حافظه را دو بار تحویل نمی‌دهد. وقتی به انتهای هیپ برسد، دیگر حافظه‌ای نمی‌توان تخصیص داد و در تخصیص بعدی خطای کمبود حافظه رخ می‌دهد.

تخصیص‌دهنده افزایشی اغلب با یک شمارنده تخصیص پیاده‌سازی می‌شود که در هر فراخوانی `alloc` یک واحد افزایش و در هر فراخوانی `dealloc` یک واحد کاهش می‌یابد. وقتی شمارنده تخصیص به صفر برسد، به این معنی است که همه تخصیص‌های روی هیپ آزاد شده‌اند. در این حالت، می‌توان اشاره‌گر `next` را به آدرس شروع هیپ بازنشانی کرد تا کل حافظه هیپ دوباره برای تخصیص در دسترس باشد.

### پیاده‌سازی

پیاده‌سازی خود را با تعریف یک زیرماژول جدید به نام `allocator::bump` آغاز می‌کنیم:

```rust
// in src/allocator.rs

pub mod bump;
```

محتوای این زیرماژول در فایل جدید `src/allocator/bump.rs` قرار می‌گیرد که آن را با محتوای زیر ایجاد می‌کنیم:

```rust
// in src/allocator/bump.rs

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    /// Creates a new empty bump allocator.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    /// Initializes the bump allocator with the given heap bounds.
    ///
    /// This method is unsafe because the caller must ensure that the given
    /// memory range is unused. Also, this method must be called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}
```

فیلدهای `heap_start` و `heap_end` مرزهای پایینی و بالایی ناحیه حافظه هیپ را دنبال می‌کنند. فراخوانی‌کننده باید اطمینان حاصل کند که این آدرس‌ها معتبر هستند، در غیر این‌صورت تخصیص‌دهنده حافظه نامعتبر برمی‌گرداند. به همین دلیل، فراخوانی تابع `init` باید `unsafe` باشد.

هدف از فیلد `next` این است که همیشه به اولین بایت استفاده‌نشده هیپ، یعنی آدرس شروع تخصیص بعدی، اشاره کند. این فیلد در تابع `init` برابر با `heap_start` قرار می‌گیرد، زیرا در ابتدا کل هیپ استفاده‌نشده است. در هر تخصیص، این فیلد به اندازه حجم تخصیص افزایش می‌یابد (_«bump می‌شود»_) تا مطمئن شویم یک ناحیه حافظه را دو بار برنمی‌گردانیم.

فیلد `allocations` یک شمارنده ساده برای تخصیص‌های فعال است، با این هدف که پس از آزاد شدن آخرین تخصیص، تخصیص‌دهنده بازنشانی شود. مقدار اولیه آن 0 است.

ما تصمیم گرفتیم به جای انجام مقداردهی اولیه به‌طور مستقیم در `new`، یک تابع `init` جداگانه ایجاد کنیم تا رابط آن با تخصیص‌دهنده‌ای که کِرِیت `linked_list_allocator` فراهم می‌کند یکسان بماند. به این ترتیب، می‌توان تخصیص‌دهنده‌ها را بدون تغییرات اضافی در کد جابه‌جا کرد.

### پیاده‌سازی `GlobalAlloc`

همان‌طور که [در پست قبلی توضیح داده شد][global-alloc]، همه تخصیص‌دهنده‌های هیپ باید تِرِیت [`GlobalAlloc`] را پیاده‌سازی کنند که به این صورت تعریف شده است:

[global-alloc]: @/edition-2/posts/10-heap-allocation/index.md#the-allocator-interface
[`GlobalAlloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html

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

تنها متدهای `alloc` و `dealloc` الزامی هستند؛ دو متد دیگر پیاده‌سازی پیش‌فرض دارند و می‌توان از آن‌ها صرف‌نظر کرد.

#### اولین تلاش برای پیاده‌سازی

بیایید متد `alloc` را برای `BumpAllocator` خود پیاده‌سازی کنیم:

```rust
// in src/allocator/bump.rs

use alloc::alloc::{GlobalAlloc, Layout};

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // TODO alignment and bounds check
        let alloc_start = self.next;
        self.next = alloc_start + layout.size();
        self.allocations += 1;
        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        todo!();
    }
}
```

ابتدا از فیلد `next` به عنوان آدرس شروع تخصیص خود استفاده می‌کنیم. سپس فیلد `next` را به‌روزرسانی می‌کنیم تا به آدرس پایان تخصیص اشاره کند که همان آدرس استفاده‌نشده بعدی روی هیپ است. پیش از برگرداندن آدرس شروع تخصیص به‌صورت یک اشاره‌گر `*mut u8`، شمارنده `allocations` را یک واحد افزایش می‌دهیم.

توجه کنید که هیچ بررسی مرزی یا تنظیم ترازی انجام نمی‌دهیم، بنابراین این پیاده‌سازی هنوز امن نیست. این موضوع اهمیت چندانی ندارد، چون در هر صورت با خطای زیر کامپایل نمی‌شود:

```
error[E0594]: cannot assign to `self.next` which is behind a `&` reference
  --> src/allocator/bump.rs:29:9
   |
29 |         self.next = alloc_start + layout.size();
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `self` is a `&` reference, so the data it refers to cannot be written
```

(همین خطا برای خط `self.allocations += 1` نیز رخ می‌دهد. برای اختصار آن را در اینجا حذف کرده‌ایم.)

این خطا به این دلیل رخ می‌دهد که متدهای [`alloc`] و [`dealloc`] تِرِیت `GlobalAlloc` تنها روی یک ارجاع تغییرناپذیر `&self` کار می‌کنند، بنابراین به‌روزرسانی فیلدهای `next` و `allocations` ممکن نیست. این موضوع مشکل‌ساز است، زیرا به‌روزرسانی `next` در هر تخصیص، اصل بنیادی یک تخصیص‌دهنده افزایشی است.

[`alloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.alloc
[`dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc

#### `GlobalAlloc` و تغییرپذیری {#globalalloc-and-mutability}

پیش از آن‌که به یک راه‌حل ممکن برای این مشکل تغییرپذیری نگاه کنیم، بیایید بفهمیم چرا متدهای تِرِیت `GlobalAlloc` با آرگومان `&self` تعریف شده‌اند: همان‌طور که [در پست قبلی][global-allocator] دیدیم، تخصیص‌دهنده سراسری هیپ با افزودن ویژگی `#[global_allocator]` به یک `static` که تِرِیت `GlobalAlloc` را پیاده‌سازی می‌کند تعریف می‌شود. متغیرهای استاتیک در Rust تغییرناپذیر هستند، بنابراین هیچ راهی برای فراخوانی متدی که `&mut self` می‌گیرد روی تخصیص‌دهنده استاتیک وجود ندارد. به همین دلیل، همه متدهای `GlobalAlloc` تنها یک ارجاع تغییرناپذیر `&self` می‌گیرند.

[global-allocator]:  @/edition-2/posts/10-heap-allocation/index.md#the-global-allocator-attribute

خوشبختانه راهی برای گرفتن یک ارجاع `&mut self` از ارجاع `&self` وجود دارد: می‌توانیم با پیچیدن تخصیص‌دهنده در یک قفل چرخشی [`spin::Mutex`] از [تغییرپذیری درونی] همگام‌شده استفاده کنیم. این نوع، متدی به نام `lock` فراهم می‌کند که [انحصار متقابل] را اعمال می‌کند و بنابراین به‌صورت امن یک ارجاع `&self` را به ارجاع `&mut self` تبدیل می‌کند. ما پیش از این چندین بار از این نوع پوشش‌دهنده در هسته خود استفاده کرده‌ایم، برای مثال برای [بافر متنی VGA][vga-mutex].

[تغییرپذیری درونی]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
[vga-mutex]: @/edition-2/posts/03-vga-text-buffer/index.md#spinlocks
[`spin::Mutex`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html
[انحصار متقابل]: https://en.wikipedia.org/wiki/Mutual_exclusion

#### یک نوع پوشش‌دهنده `Locked`

با کمک نوع پوشش‌دهنده `spin::Mutex` می‌توانیم تِرِیت `GlobalAlloc` را برای تخصیص‌دهنده افزایشی خود پیاده‌سازی کنیم. ترفند کار این است که تِرِیت را نه به‌طور مستقیم برای `BumpAllocator`، بلکه برای نوع پیچیده‌شده `spin::Mutex<BumpAllocator>` پیاده‌سازی کنیم:

```rust
unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {…}
```

متأسفانه این کار هنوز جواب نمی‌دهد، زیرا کامپایلر Rust اجازه پیاده‌سازی تِرِیت‌ها را برای نوع‌هایی که در کِرِیت‌های دیگر تعریف شده‌اند نمی‌دهد:

```
error[E0117]: only traits defined in the current crate can be implemented for arbitrary types
  --> src/allocator/bump.rs:28:1
   |
28 | unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^--------------------------
   | |                           |
   | |                           `spin::mutex::Mutex` is not defined in the current crate
   | impl doesn't use only types from inside the current crate
   |
   = note: define and implement a trait or new type instead
```

برای رفع این مشکل، باید نوع پوشش‌دهنده خودمان را حول `spin::Mutex` بسازیم:

```rust
// in src/allocator.rs

/// A wrapper around spin::Mutex to permit trait implementations.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}
```

این نوع یک پوشش‌دهنده جنریک حول `spin::Mutex<A>` است. هیچ محدودیتی روی نوع پیچیده‌شده `A` اعمال نمی‌کند، بنابراین می‌توان از آن برای پیچیدن هر نوعی استفاده کرد، نه فقط تخصیص‌دهنده‌ها. این نوع یک تابع سازنده ساده به نام `new` فراهم می‌کند که مقدار داده‌شده را می‌پیچد. برای راحتی، تابعی به نام `lock` نیز فراهم می‌کند که `lock` را روی `Mutex` پیچیده‌شده فراخوانی می‌کند. از آن‌جا که نوع `Locked` به‌اندازه کافی عمومی است تا برای پیاده‌سازی‌های دیگر تخصیص‌دهنده نیز مفید باشد، آن را در ماژول والد `allocator` قرار می‌دهیم.

#### پیاده‌سازی برای `Locked<BumpAllocator>`

نوع `Locked` در کِرِیت خودمان تعریف شده است (برخلاف `spin::Mutex`)، بنابراین می‌توانیم از آن برای پیاده‌سازی `GlobalAlloc` برای تخصیص‌دهنده افزایشی خود استفاده کنیم. پیاده‌سازی کامل به این شکل است:

```rust
// in src/allocator/bump.rs

use super::{align_up, Locked};
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock(); // get a mutable reference

        let alloc_start = align_up(bump.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            ptr::null_mut() // out of memory
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock(); // get a mutable reference

        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}
```

اولین گام در هر دو متد `alloc` و `dealloc` این است که متد [`Mutex::lock`] را از طریق فیلد `inner` فراخوانی کنیم تا یک ارجاع تغییرپذیر به نوع تخصیص‌دهنده پیچیده‌شده به دست آوریم. این نمونه تا پایان متد قفل می‌ماند تا هیچ رقابت داده‌ای (data race) در بسترهای چندنخی رخ ندهد (به‌زودی پشتیبانی از نخ‌ها را اضافه خواهیم کرد).

[`Mutex::lock`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html#method.lock

در مقایسه با نمونه اولیه قبلی، پیاده‌سازی `alloc` اکنون الزامات تراز را رعایت می‌کند و یک بررسی مرزی انجام می‌دهد تا مطمئن شود تخصیص‌ها درون ناحیه حافظه هیپ باقی می‌مانند. اولین گام این است که آدرس `next` را به تراز مشخص‌شده توسط آرگومان `Layout` به سمت بالا گرد کنیم. کد تابع `align_up` را کمی بعدتر می‌بینیم. سپس اندازه تخصیص درخواست‌شده را به `alloc_start` اضافه می‌کنیم تا آدرس پایان تخصیص را به دست آوریم. برای جلوگیری از سرریز عدد صحیح در تخصیص‌های بزرگ، از متد [`checked_add`] استفاده می‌کنیم. اگر سرریزی رخ دهد یا آدرس پایانیِ حاصل از تخصیص بزرگ‌تر از آدرس پایان هیپ باشد، یک اشاره‌گر تهی برمی‌گردانیم تا وضعیت کمبود حافظه را اعلام کنیم. در غیر این‌صورت، آدرس `next` را به‌روزرسانی می‌کنیم و مانند قبل شمارنده `allocations` را یک واحد افزایش می‌دهیم. در نهایت، آدرس `alloc_start` را که به اشاره‌گر `*mut u8` تبدیل شده است برمی‌گردانیم.

[`checked_add`]: https://doc.rust-lang.org/std/primitive.usize.html#method.checked_add
[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html

تابع `dealloc` آرگومان‌های اشاره‌گر و `Layout` داده‌شده را نادیده می‌گیرد. در عوض، تنها شمارنده `allocations` را کاهش می‌دهد. اگر شمارنده دوباره به `0` برسد، یعنی همه تخصیص‌ها دوباره آزاد شده‌اند. در این حالت، آدرس `next` را به آدرس `heap_start` بازنشانی می‌کند تا کل حافظه هیپ دوباره در دسترس قرار گیرد.

#### هم‌ترازسازی آدرس

تابع `align_up` به‌اندازه کافی عمومی است که بتوانیم آن را در ماژول والد `allocator` قرار دهیم. یک پیاده‌سازی ساده به این شکل است:

```rust
// in src/allocator.rs

/// Align the given address `addr` upwards to alignment `align`.
fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr // addr already aligned
    } else {
        addr - remainder + align
    }
}
```

این تابع ابتدا [باقی‌مانده] تقسیم `addr` بر `align` را محاسبه می‌کند. اگر باقی‌مانده `0` باشد، آدرس از پیش با تراز داده‌شده هم‌تراز است. در غیر این‌صورت، آدرس را با کم کردن باقی‌مانده (تا باقی‌مانده جدید 0 شود) و سپس افزودن مقدار تراز (تا آدرس از آدرس اصلی کوچک‌تر نشود) هم‌تراز می‌کنیم.

[باقی‌مانده]: https://en.wikipedia.org/wiki/Euclidean_division

توجه کنید که این کارآمدترین روش برای پیاده‌سازی این تابع نیست. یک پیاده‌سازی بسیار سریع‌تر به این شکل است:

```rust
/// Align the given address `addr` upwards to alignment `align`.
///
/// Requires that `align` is a power of two.
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
```

این روش نیاز دارد که `align` توانی از دو باشد، که با بهره‌گیری از تِرِیت `GlobalAlloc` (و پارامتر [`Layout`] آن) قابل تضمین است. این موضوع امکان ساختن یک [ماسک بیتی] را فراهم می‌کند تا آدرس به شکلی بسیار کارآمد هم‌تراز شود. برای این‌که بفهمیم چگونه کار می‌کند، بیایید گام‌به‌گام و از سمت راست آن را بررسی کنیم:

[`Layout`]: https://doc.rust-lang.org/alloc/alloc/struct.Layout.html
[ماسک بیتی]: https://en.wikipedia.org/wiki/Mask_(computing)

- از آن‌جا که `align` توانی از دو است، [نمایش دودویی] آن تنها یک بیت روشن دارد (برای مثال `0b000100000`). این یعنی `align - 1` همه بیت‌های پایین‌تر را روشن دارد (برای مثال `0b00011111`).
- با ساختن [`NOT` بیتی] از طریق عملگر `!`، عددی به دست می‌آوریم که همه بیت‌هایش روشن است، به‌جز بیت‌های پایین‌تر از `align` (برای مثال `0b…111111111100000`).
- با انجام یک [`AND` بیتی] روی آدرس و `!(align - 1)`، آدرس را _به سمت پایین_ هم‌تراز می‌کنیم. این کار با پاک کردن همه بیت‌هایی که پایین‌تر از `align` هستند انجام می‌شود.
- از آن‌جا که می‌خواهیم به‌جای پایین به سمت بالا هم‌تراز کنیم، پیش از انجام `AND` بیتی، `addr` را به اندازه `align - 1` افزایش می‌دهیم. به این ترتیب، آدرس‌هایی که از پیش هم‌تراز هستند بدون تغییر می‌مانند، در حالی که آدرس‌های هم‌تراز نشده به مرز تراز بعدی گرد می‌شوند.

[نمایش دودویی]: https://en.wikipedia.org/wiki/Binary_number#Representation
[`NOT` بیتی]: https://en.wikipedia.org/wiki/Bitwise_operation#NOT
[`AND` بیتی]: https://en.wikipedia.org/wiki/Bitwise_operation#AND

این‌که کدام گونه را انتخاب کنید به خودتان بستگی دارد. هر دو نتیجه یکسانی را محاسبه می‌کنند، فقط با روش‌های متفاوت.

### استفاده از آن

برای استفاده از تخصیص‌دهنده افزایشی به‌جای کِرِیت `linked_list_allocator`، باید استاتیک `ALLOCATOR` را در `allocator.rs` به‌روزرسانی کنیم:

```rust
// in src/allocator.rs

use bump::BumpAllocator;

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());
```

در اینجا اهمیت پیدا می‌کند که `BumpAllocator::new` و `Locked::new` را به‌صورت [توابع `const`] تعریف کردیم. اگر آن‌ها توابع معمولی بودند، خطای کامپایل رخ می‌داد، زیرا عبارت مقداردهی اولیه یک `static` باید در زمان کامپایل قابل ارزیابی باشد.

[توابع `const`]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

نیازی به تغییر فراخوانی `ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE)` در تابع `init_heap` خود نداریم، زیرا تخصیص‌دهنده افزایشی همان رابطی را فراهم می‌کند که تخصیص‌دهنده `linked_list_allocator` فراهم می‌کرد.

اکنون هسته ما از تخصیص‌دهنده افزایشی خودمان استفاده می‌کند! همه چیز باید همچنان کار کند، از جمله [تست‌های `heap_allocation`] که در پست قبلی ایجاد کردیم:

[تست‌های `heap_allocation`]: @/edition-2/posts/10-heap-allocation/index.md#adding-a-test

```
> cargo test --test heap_allocation
[…]
Running 3 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
```

### بحث

مزیت بزرگ تخصیص افزایشی این است که بسیار سریع است. در مقایسه با دیگر طراحی‌های تخصیص‌دهنده (که در ادامه می‌بینیم) که باید فعالانه به دنبال یک بلوک حافظه مناسب بگردند و کارهای دفترداری گوناگونی را در `alloc` و `dealloc` انجام دهند، یک تخصیص‌دهنده افزایشی [می‌تواند تا حد تنها چند دستور اسمبلی بهینه شود][bump downwards]. همین موضوع تخصیص‌دهنده‌های افزایشی را برای بهینه‌سازی کارایی تخصیص مفید می‌کند، برای مثال هنگام ساختن یک [کتابخانه DOM مجازی].

[bump downwards]: https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
[کتابخانه DOM مجازی]: https://hacks.mozilla.org/2019/03/fast-bump-allocated-virtual-doms-with-rust-and-wasm/

اگرچه تخصیص‌دهنده افزایشی به‌ندرت به‌عنوان تخصیص‌دهنده سراسری استفاده می‌شود، اصل تخصیص افزایشی اغلب در قالب [تخصیص آرِنا] به کار می‌رود که در اصل تخصیص‌های جداگانه را دسته‌بندی می‌کند تا کارایی بهبود یابد. نمونه‌ای از یک تخصیص‌دهنده آرِنا برای Rust در کِرِیت [`toolshed`] وجود دارد.

[تخصیص آرِنا]: https://mgravell.github.io/Pipelines.Sockets.Unofficial/docs/arenas.html
[`toolshed`]: https://docs.rs/toolshed/0.8.1/toolshed/index.html

#### ایراد تخصیص‌دهنده افزایشی

محدودیت اصلی یک تخصیص‌دهنده افزایشی این است که تنها پس از آزاد شدن همه تخصیص‌ها می‌تواند از حافظه آزادشده دوباره استفاده کند. این یعنی تنها یک تخصیص بلندعمر کافی است تا از استفاده مجدد حافظه جلوگیری شود. این موضوع را زمانی می‌بینیم که گونه‌ای از تست `many_boxes` را اضافه کنیم:

```rust
// in tests/heap_allocation.rs

#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1); // new
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1); // new
}
```

مانند تست `many_boxes`، این تست تعداد زیادی تخصیص ایجاد می‌کند تا اگر تخصیص‌دهنده از حافظه آزادشده دوباره استفاده نکند، خطای کمبود حافظه را برانگیزد. علاوه بر این، تست یک تخصیص `long_lived` ایجاد می‌کند که در تمام مدت اجرای حلقه زنده می‌ماند.

وقتی تلاش می‌کنیم تست جدید خود را اجرا کنیم، می‌بینیم که واقعاً شکست می‌خورد:

```
> cargo test --test heap_allocation
Running 4 tests
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [failed]

Error: panicked at 'allocation error: Layout { size_: 8, align_: 8 }', src/lib.rs:86:5
```

بیایید با جزئیات بفهمیم چرا این شکست رخ می‌دهد: ابتدا تخصیص `long_lived` در ابتدای هیپ ایجاد می‌شود و در نتیجه شمارنده `allocations` را یک واحد افزایش می‌دهد. در هر تکرار حلقه، یک تخصیص کوتاه‌عمر ایجاد و بلافاصله پیش از شروع تکرار بعدی دوباره آزاد می‌شود. این یعنی شمارنده `allocations` در ابتدای هر تکرار به‌طور موقت به 2 افزایش و در پایان آن به 1 کاهش می‌یابد. مشکل اینجاست که تخصیص‌دهنده افزایشی تنها پس از آزاد شدن _همه_ تخصیص‌ها می‌تواند از حافظه دوباره استفاده کند، یعنی وقتی شمارنده `allocations` به 0 برسد. از آن‌جا که این اتفاق پیش از پایان حلقه رخ نمی‌دهد، هر تکرار حلقه ناحیه جدیدی از حافظه را تخصیص می‌دهد و پس از تعدادی تکرار به خطای کمبود حافظه منجر می‌شود.

#### رفع مشکل تست؟

دو ترفند بالقوه وجود دارد که می‌توانیم برای رفع این تست در تخصیص‌دهنده افزایشی خود از آن‌ها بهره بگیریم:

- می‌توانیم `dealloc` را به‌گونه‌ای به‌روزرسانی کنیم که با مقایسه آدرس پایانِ تخصیص آزادشده با اشاره‌گر `next`، بررسی کند آیا آن تخصیص همان آخرین تخصیصی بوده که `alloc` برگردانده است یا نه. اگر برابر باشند، می‌توانیم با اطمینان `next` را به آدرس شروع تخصیص آزادشده بازگردانیم. به این ترتیب، هر تکرار حلقه از همان بلوک حافظه دوباره استفاده می‌کند.
- می‌توانیم متدی به نام `alloc_back` اضافه کنیم که با استفاده از فیلد اضافی `next_back` حافظه را از _انتهای_ هیپ تخصیص دهد. سپس می‌توانیم این روش تخصیص را به‌صورت دستی برای همه تخصیص‌های بلندعمر به کار ببریم و به این ترتیب تخصیص‌های کوتاه‌عمر و بلندعمر را روی هیپ از هم جدا کنیم. توجه کنید که این جداسازی تنها زمانی کار می‌کند که از پیش مشخص باشد هر تخصیص چه مدت زنده خواهد ماند. ایراد دیگر این رویکرد این است که انجام دستی تخصیص‌ها دشوار و بالقوه ناامن است.

اگرچه هر دو این رویکردها برای رفع تست کار می‌کنند، اما راه‌حلی عمومی نیستند، زیرا تنها در موارد بسیار خاصی می‌توانند حافظه را دوباره استفاده کنند. سوال این است: آیا راه‌حلی عمومی وجود دارد که از _همه_ حافظه آزادشده دوباره استفاده کند؟

#### استفاده مجدد از تمام حافظه آزادشده؟

همان‌طور که [در پست قبلی][heap-intro] آموختیم، تخصیص‌ها می‌توانند به‌طور دلخواه طولانی زنده بمانند و به ترتیب دلخواه آزاد شوند. این یعنی باید تعداد بالقوه نامحدودی از ناحیه‌های حافظه استفاده‌نشده و ناپیوسته را دنبال کنیم، همان‌طور که مثال زیر نشان می‌دهد:

[heap-intro]: @/edition-2/posts/10-heap-allocation/index.md#dynamic-memory

![](allocation-fragmentation.svg)

این تصویر هیپ را در گذر زمان نشان می‌دهد. در ابتدا کل هیپ استفاده‌نشده است و آدرس `next` برابر با `heap_start` است (خط 1). سپس اولین تخصیص رخ می‌دهد (خط 2). در خط 3، بلوک حافظه دومی تخصیص داده می‌شود و تخصیص اول آزاد می‌شود. در خط 4 تخصیص‌های بسیار بیشتری اضافه می‌شوند. نیمی از آن‌ها بسیار کوتاه‌عمر هستند و در خط 5 آزاد می‌شوند، جایی که یک تخصیص جدید دیگر نیز اضافه می‌شود.

خط 5 مشکل بنیادی را نشان می‌دهد: پنج ناحیه حافظه استفاده‌نشده با اندازه‌های متفاوت داریم، اما اشاره‌گر `next` تنها می‌تواند به ابتدای آخرین ناحیه اشاره کند. اگرچه برای این مثال می‌توانستیم آدرس‌های شروع و اندازه‌های دیگر ناحیه‌های حافظه استفاده‌نشده را در آرایه‌ای به اندازه 4 ذخیره کنیم، اما این راه‌حلی عمومی نیست، زیرا به‌راحتی می‌توان مثالی با 8، 16 یا 1000 ناحیه حافظه استفاده‌نشده ساخت.

معمولاً وقتی تعداد بالقوه نامحدودی آیتم داریم، می‌توانیم به‌سادگی از یک مجموعه تخصیص‌یافته روی هیپ استفاده کنیم. این کار در مورد ما واقعاً ممکن نیست، زیرا تخصیص‌دهنده هیپ نمی‌تواند به خودش وابسته باشد (این کار باعث بازگشت بی‌پایان یا بن‌بست می‌شود). بنابراین باید راه‌حل دیگری پیدا کنیم.

## تخصیص‌دهنده لیست پیوندی

یک ترفند رایج برای دنبال کردن تعداد دلخواهی از نواحی حافظه آزاد هنگام پیاده‌سازی تخصیص‌دهنده‌ها این است که از خود این نواحی به‌عنوان فضای ذخیره‌سازی پشتیبان استفاده کنیم. این کار از این واقعیت بهره می‌برد که این ناحیه‌ها هنوز به یک آدرس مجازی نگاشت شده‌اند و یک قاب فیزیکی پشتیبانشان می‌کند، اما به اطلاعات ذخیره‌شده در آن‌ها دیگر نیازی نیست. با ذخیره کردن اطلاعات مربوط به ناحیه آزادشده در خودِ آن ناحیه، می‌توانیم تعداد نامحدودی از ناحیه‌های آزادشده را بدون نیاز به حافظه اضافی دنبال کنیم.

رایج‌ترین رویکرد پیاده‌سازی این است که یک لیست پیوندی یک‌طرفه در حافظه آزادشده بسازیم که هر گره آن یک ناحیه حافظه آزادشده است:

![](linked-list-allocation.svg)

هر گره لیست شامل دو فیلد است: اندازه ناحیه حافظه و یک اشاره‌گر به ناحیه حافظه استفاده‌نشده بعدی. با این رویکرد، تنها به یک اشاره‌گر به اولین ناحیه استفاده‌نشده (که `head` نامیده می‌شود) نیاز داریم تا همه ناحیه‌های استفاده‌نشده را بدون توجه به تعدادشان دنبال کنیم. ساختار داده حاصل اغلب [_لیست آزاد_] نامیده می‌شود.

[_لیست آزاد_]: https://en.wikipedia.org/wiki/Free_list

همان‌طور که ممکن است از نام آن حدس بزنید، این همان تکنیکی است که کِرِیت `linked_list_allocator` از آن استفاده می‌کند. تخصیص‌دهنده‌هایی که از این تکنیک استفاده می‌کنند اغلب _تخصیص‌دهنده‌های استخری_ نیز نامیده می‌شوند.

### پیاده‌سازی

در ادامه، نوع ساده `LinkedListAllocator` خودمان را می‌سازیم که از رویکرد بالا برای دنبال کردن ناحیه‌های حافظه آزادشده استفاده می‌کند. این بخش از پست برای پست‌های بعدی لازم نیست، بنابراین اگر دوست دارید می‌توانید از جزئیات پیاده‌سازی صرف‌نظر کنید.

#### نوع تخصیص‌دهنده {#the-allocator-type}

کار را با ساختن یک ساختار خصوصی `ListNode` در زیرماژول جدید `allocator::linked_list` آغاز می‌کنیم:

```rust
// in src/allocator.rs

pub mod linked_list;
```

```rust
// in src/allocator/linked_list.rs

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}
```

مانند تصویر، هر گره لیست یک فیلد `size` و یک اشاره‌گر اختیاری به گره بعدی دارد که با نوع `Option<&'static mut ListNode>` نمایش داده می‌شود. نوع `&'static mut` از نظر معنایی یک شیء [تحت مالکیت] پشت یک اشاره‌گر را توصیف می‌کند. در اصل، این یک [`Box`] بدون تخریب‌کننده است؛ تخریب‌کننده‌ای که شیء را در پایان دامنه آزاد کند.

[تحت مالکیت]: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html
[`Box`]: https://doc.rust-lang.org/alloc/boxed/index.html

مجموعه متدهای زیر را برای `ListNode` پیاده‌سازی می‌کنیم:

```rust
// in src/allocator/linked_list.rs

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}
```

این نوع یک تابع سازنده ساده به نام `new` و متدهایی برای محاسبه آدرس‌های شروع و پایان ناحیه‌ای که نمایش می‌دهد دارد. تابع `new` را یک [تابع const] می‌کنیم که بعداً هنگام ساختن یک تخصیص‌دهنده لیست پیوندی استاتیک لازم خواهد بود.

[تابع const]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

با داشتن ساختار `ListNode` به‌عنوان بلوک سازنده، اکنون می‌توانیم ساختار `LinkedListAllocator` را ایجاد کنیم:

```rust
// in src/allocator/linked_list.rs

pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    /// Creates an empty LinkedListAllocator.
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    /// Adds the given memory region to the front of the list.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        todo!();
    }
}
```

این ساختار شامل یک گره `head` است که به اولین ناحیه هیپ اشاره می‌کند. ما تنها به مقدار اشاره‌گر `next` علاقه‌مندیم، بنابراین در تابع `ListNode::new` مقدار `size` را 0 قرار می‌دهیم. این‌که `head` را به‌جای صرفاً یک `&'static mut ListNode` از نوع `ListNode` بگیریم، این مزیت را دارد که پیاده‌سازی متد `alloc` ساده‌تر خواهد بود.

مانند تخصیص‌دهنده افزایشی، تابع `new` تخصیص‌دهنده را با مرزهای هیپ مقداردهی اولیه نمی‌کند. علاوه بر حفظ سازگاری API، دلیل این کار آن است که روال مقداردهی اولیه نیازمند نوشتن یک گره در حافظه هیپ است که تنها در زمان اجرا امکان‌پذیر است. با این حال، تابع `new` باید یک [تابع `const`] باشد که در زمان کامپایل قابل ارزیابی است، زیرا برای مقداردهی اولیه استاتیک `ALLOCATOR` به کار می‌رود. به همین دلیل، دوباره یک متد `init` جداگانه و غیرثابت فراهم می‌کنیم.

[تابع `const`]: https://doc.rust-lang.org/reference/items/functions.html#const-functions

متد `init` از متدی به نام `add_free_region` استفاده می‌کند که پیاده‌سازی آن را کمی بعدتر می‌بینیم. فعلاً از ماکروی [`todo!`] استفاده می‌کنیم تا یک پیاده‌سازی موقت فراهم کنیم که همیشه پنیک می‌کند.

[`todo!`]: https://doc.rust-lang.org/core/macro.todo.html

#### متد `add_free_region`

متد `add_free_region` عملیات بنیادی _push_ را روی لیست پیوندی فراهم می‌کند. در حال حاضر این متد را تنها از `init` فراخوانی می‌کنیم، اما متد محوری در پیاده‌سازی `dealloc` ما نیز خواهد بود. به یاد داشته باشید که متد `dealloc` زمانی فراخوانی می‌شود که یک ناحیه حافظه تخصیص‌یافته دوباره آزاد شود. برای دنبال کردن این ناحیه حافظه آزادشده، می‌خواهیم آن را به لیست پیوندی push کنیم.

پیاده‌سازی متد `add_free_region` به این شکل است:

```rust
// in src/allocator/linked_list.rs

use super::align_up;
use core::mem;

impl LinkedListAllocator {
    /// Adds the given memory region to the front of the list.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // ensure that the freed region is capable of holding ListNode
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // create a new list node and append it at the start of the list
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        unsafe {
            node_ptr.write(node);
            self.head.next = Some(&mut *node_ptr)
        }
    }
}
```

این متد آدرس و اندازه یک ناحیه حافظه را به‌عنوان آرگومان می‌گیرد و آن را به ابتدای لیست اضافه می‌کند. ابتدا اطمینان حاصل می‌کند که ناحیه داده‌شده اندازه و ترازِ لازم برای ذخیره یک `ListNode` را دارد. سپس گره را ایجاد کرده و طی گام‌های زیر آن را در لیست درج می‌کند:

![](linked-list-allocator-push.svg)

گام 0 وضعیت هیپ را پیش از فراخوانی `add_free_region` نشان می‌دهد. در گام 1، این متد با ناحیه حافظه‌ای که در تصویر با برچسب `freed` مشخص شده فراخوانی می‌شود. پس از بررسی‌های اولیه، متد یک `node` جدید با اندازه ناحیه آزادشده روی پشته خود می‌سازد. سپس از متد [`Option::take`] استفاده می‌کند تا اشاره‌گر `next` گره را به اشاره‌گر `head` فعلی تنظیم کند و در نتیجه اشاره‌گر `head` را به `None` بازنشانی کند.

[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

در گام 2، متد `node` تازه‌ساخته‌شده را با استفاده از متد [`write`] در ابتدای ناحیه حافظه آزادشده می‌نویسد. سپس اشاره‌گر `head` را به گره جدید اشاره می‌دهد. ساختار اشاره‌گرهای حاصل کمی آشفته به نظر می‌رسد، زیرا ناحیه آزادشده همیشه در ابتدای لیست درج می‌شود، اما اگر اشاره‌گرها را دنبال کنیم، می‌بینیم که هر ناحیه آزاد همچنان از اشاره‌گر `head` قابل دسترسی است.

[`write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write

#### متد `find_region`

دومین عملیات بنیادی روی یک لیست پیوندی، یافتن یک ورودی و حذف آن از لیست است. این همان عملیات محوری است که برای پیاده‌سازی متد `alloc` لازم داریم. این عملیات را به‌صورت متدی به نام `find_region` و به شکل زیر پیاده‌سازی می‌کنیم:

```rust
// in src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// Looks for a free region with the given size and alignment and removes
    /// it from the list.
    ///
    /// Returns a tuple of the list node and the start address of the allocation.
    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut ListNode, usize)>
    {
        // reference to current list node, updated for each iteration
        let mut current = &mut self.head;
        // look for a large enough memory region in linked list
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // region suitable for allocation -> remove node from list
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // region not suitable -> continue with next region
                current = current.next.as_mut().unwrap();
            }
        }

        // no suitable region found
        None
    }
}
```

این متد از متغیری به نام `current` و یک [حلقه `while let`] برای پیمایش عناصر لیست استفاده می‌کند. در ابتدا، `current` برابر با گره (ساختگی) `head` قرار می‌گیرد. سپس در هر تکرار (در بلوک `else`) به فیلد `next` گره فعلی به‌روزرسانی می‌شود. اگر ناحیه برای تخصیصی با اندازه و ترازِ داده‌شده مناسب باشد، آن ناحیه از لیست حذف و همراه با آدرس `alloc_start` برگردانده می‌شود.

[حلقه `while let`]: https://doc.rust-lang.org/reference/expressions/loop-expr.html#while-let-patterns

وقتی اشاره‌گر `current.next` برابر با `None` شود، حلقه پایان می‌یابد. این یعنی کل لیست را پیمایش کرده‌ایم اما هیچ ناحیه مناسبی برای تخصیص نیافته‌ایم. در آن صورت، `None` برمی‌گردانیم. این‌که یک ناحیه مناسب است یا نه، توسط تابع `alloc_from_region` بررسی می‌شود که پیاده‌سازی آن را کمی بعدتر می‌بینیم.

بیایید با جزئیات بیشتری ببینیم که چگونه یک ناحیه مناسب از لیست حذف می‌شود:

![](linked-list-allocator-remove-region.svg)

گام 0 وضعیت را پیش از هرگونه تنظیم اشاره‌گر نشان می‌دهد. ناحیه‌های `region` و `current` و اشاره‌گرهای `region.next` و `current.next` در تصویر مشخص شده‌اند. در گام 1، هر دو اشاره‌گر `region.next` و `current.next` با استفاده از متد [`Option::take`] به `None` بازنشانی می‌شوند. اشاره‌گرهای اصلی در متغیرهای محلی‌ای به نام `next` و `ret` ذخیره می‌شوند.

در گام 2، اشاره‌گر `current.next` برابر با اشاره‌گر محلی `next` قرار می‌گیرد که همان اشاره‌گر اصلی `region.next` است. نتیجه این است که اکنون `current` مستقیماً به ناحیه پس از `region` اشاره می‌کند، بنابراین `region` دیگر عضوی از لیست پیوندی نیست. سپس تابع اشاره‌گر به `region` را که در متغیر محلی `ret` ذخیره شده است برمی‌گرداند.

##### تابع `alloc_from_region`

تابع `alloc_from_region` مشخص می‌کند که آیا یک ناحیه برای تخصیصی با اندازه و ترازِ داده‌شده مناسب است یا نه. این تابع به این شکل تعریف شده است:

```rust
// in src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// Try to use the given region for an allocation with given size and
    /// alignment.
    ///
    /// Returns the allocation start address on success.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
        -> Result<usize, ()>
    {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            // region too small
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            // rest of region too small to hold a ListNode (required because the
            // allocation splits the region in a used and a free part)
            return Err(());
        }

        // region suitable for allocation
        Ok(alloc_start)
    }
}
```

ابتدا، تابع با استفاده از تابع `align_up` که پیش‌تر تعریف کردیم و متد [`checked_add`]، آدرس‌های شروع و پایان یک تخصیص بالقوه را محاسبه می‌کند. اگر سرریزی رخ دهد یا آدرس پایان از آدرس پایان ناحیه فراتر برود، تخصیص در آن ناحیه جا نمی‌شود و یک خطا برمی‌گردانیم.

پس از آن، تابع بررسی کمتر بدیهی‌ای انجام می‌دهد. این بررسی لازم است، زیرا در بیشتر مواقع یک تخصیص دقیقاً در ناحیه مناسب جا نمی‌گیرد و بخشی از ناحیه پس از تخصیص همچنان قابل استفاده باقی می‌ماند. این بخش از ناحیه باید پس از تخصیص، `ListNode` مخصوص خود را ذخیره کند، بنابراین باید به‌اندازه کافی بزرگ باشد. این بررسی دقیقاً همین را وارسی می‌کند: یا تخصیص کاملاً جا می‌شود (`excess_size == 0`) یا اندازه مازاد آن‌قدر بزرگ هست که بتوان یک `ListNode` را در آن ذخیره کرد.

#### پیاده‌سازی `GlobalAlloc`

با عملیات بنیادی‌ای که متدهای `add_free_region` و `find_region` فراهم می‌کنند، اکنون سرانجام می‌توانیم تِرِیت `GlobalAlloc` را پیاده‌سازی کنیم. مانند تخصیص‌دهنده افزایشی، این تِرِیت را مستقیماً برای `LinkedListAllocator` پیاده‌سازی نمی‌کنیم، بلکه تنها برای نوع پیچیده‌شده `Locked<LinkedListAllocator>` این کار را انجام می‌دهیم. [پوشش‌دهنده `Locked`] از طریق یک قفل چرخشی تغییرپذیری درونی را اضافه می‌کند که به ما اجازه می‌دهد نمونه تخصیص‌دهنده را تغییر دهیم، حتی با وجود این‌که متدهای `alloc` و `dealloc` تنها ارجاع `&self` می‌گیرند.

[پوشش‌دهنده `Locked`]: @/edition-2/posts/11-allocator-designs/index.md#a-locked-wrapper-type

پیاده‌سازی به این شکل است:

```rust
// in src/allocator/linked_list.rs

use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // perform layout adjustments
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                unsafe {
                    allocator.add_free_region(alloc_end, excess_size);
                }
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // perform layout adjustments
        let (size, _) = LinkedListAllocator::size_align(layout);

        unsafe { self.lock().add_free_region(ptr as usize, size) }
    }
}
```

بیایید با متد `dealloc` شروع کنیم، چون ساده‌تر است: ابتدا برخی تنظیمات چیدمان را انجام می‌دهد که کمی بعدتر توضیح می‌دهیم. سپس با فراخوانی تابع [`Mutex::lock`] روی [پوشش‌دهنده `Locked`]، یک ارجاع `&mut LinkedListAllocator` به دست می‌آورد. در نهایت، تابع `add_free_region` را فراخوانی می‌کند تا ناحیه آزادشده را به لیست آزاد اضافه کند.

متد `alloc` کمی پیچیده‌تر است. با همان تنظیمات چیدمان آغاز می‌شود و تابع [`Mutex::lock`] را نیز فراخوانی می‌کند تا یک ارجاع تغییرپذیر به تخصیص‌دهنده دریافت کند. سپس از متد `find_region` استفاده می‌کند تا ناحیه حافظه مناسبی برای تخصیص بیابد و آن را از لیست حذف کند. اگر این کار موفق نشود و `None` برگردانده شود، `null_mut` را برمی‌گرداند تا خطا را اعلام کند، چون هیچ ناحیه حافظه مناسبی وجود ندارد.

در حالت موفقیت، متد `find_region` یک تاپل شامل ناحیه مناسب (که دیگر در لیست نیست) و آدرس شروع تخصیص را برمی‌گرداند. با استفاده از `alloc_start`، اندازه تخصیص و آدرس پایان ناحیه، دوباره آدرس پایان تخصیص و اندازه مازاد را محاسبه می‌کند. اگر اندازه مازاد صفر نباشد، `add_free_region` را فراخوانی می‌کند تا بخش مازادِ ناحیه حافظه را دوباره به لیست آزاد اضافه کند. در نهایت، آدرس `alloc_start` را که به اشاره‌گر `*mut u8` تبدیل شده است برمی‌گرداند.

#### تنظیمات چیدمان

خب این تنظیمات چیدمان که در ابتدای هر دو متد `alloc` و `dealloc` انجام می‌دهیم چه هستند؟ آن‌ها تضمین می‌کنند که هر بلوک تخصیص‌یافته توانایی ذخیره یک `ListNode` را داشته باشد. این موضوع مهم است، زیرا آن بلوک حافظه در نقطه‌ای آزاد خواهد شد و در آن هنگام می‌خواهیم یک `ListNode` در آن بنویسیم. اگر بلوک کوچک‌تر از یک `ListNode` باشد یا تراز درستی نداشته باشد، رفتار تعریف‌نشده می‌تواند رخ دهد.

تنظیمات چیدمان توسط تابع `size_align` انجام می‌شود که به این شکل تعریف شده است:

```rust
// in src/allocator/linked_list.rs

impl LinkedListAllocator {
    /// Adjust the given layout so that the resulting allocated memory
    /// region is also capable of storing a `ListNode`.
    ///
    /// Returns the adjusted size and alignment as a (size, align) tuple.
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}
```

ابتدا، تابع از متد [`align_to`] روی [`Layout`] ارسال‌شده استفاده می‌کند تا در صورت لزوم تراز را تا تراز یک `ListNode` افزایش دهد. سپس از متد [`pad_to_align`] استفاده می‌کند تا اندازه را به مضربی از تراز گرد کند و اطمینان یابد که آدرس شروع بلوک حافظه بعدی نیز تراز درستی برای ذخیره یک `ListNode` خواهد داشت.
در گام دوم، از متد [`max`] استفاده می‌کند تا حداقل اندازه تخصیص برابر با `mem::size_of::<ListNode>` اعمال شود. به این ترتیب، تابع `dealloc` می‌تواند با اطمینان یک `ListNode` را در بلوک حافظه آزادشده بنویسد.

[`align_to`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align_to
[`pad_to_align`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.pad_to_align
[`max`]: https://doc.rust-lang.org/std/cmp/trait.Ord.html#method.max

### استفاده از آن

اکنون می‌توانیم استاتیک `ALLOCATOR` را در ماژول `allocator` به‌روزرسانی کنیم تا از `LinkedListAllocator` جدیدمان استفاده کند:

```rust
// in src/allocator.rs

use linked_list::LinkedListAllocator;

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> =
    Locked::new(LinkedListAllocator::new());
```

از آن‌جا که تابع `init` برای تخصیص‌دهنده افزایشی و تخصیص‌دهنده لیست پیوندی رفتار یکسانی دارد، نیازی به تغییر فراخوانی `init` در `init_heap` نداریم.

اکنون وقتی دوباره تست‌های `heap_allocation` خود را اجرا می‌کنیم، می‌بینیم که همه تست‌ها پاس می‌شوند، از جمله تست `many_boxes_long_lived` که با تخصیص‌دهنده افزایشی شکست می‌خورد:

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

این نشان می‌دهد که تخصیص‌دهنده لیست پیوندی ما می‌تواند از حافظه آزادشده برای تخصیص‌های بعدی دوباره استفاده کند.

### بحث

برخلاف تخصیص‌دهنده افزایشی، تخصیص‌دهنده لیست پیوندی به‌عنوان یک تخصیص‌دهنده همه‌منظوره بسیار مناسب‌تر است، عمدتاً به این دلیل که می‌تواند مستقیماً از حافظه آزادشده دوباره استفاده کند. با این حال، ایرادهایی نیز دارد. برخی از آن‌ها تنها ناشی از پیاده‌سازی ساده ما هستند، اما ایرادهای بنیادی مربوط به خودِ این طراحی تخصیص‌دهنده نیز وجود دارد.

#### ادغام بلوک‌های آزادشده {#merging-freed-blocks}

مشکل اصلی پیاده‌سازی ما این است که تنها هیپ را به بلوک‌های کوچک‌تر تقسیم می‌کند اما هرگز آن‌ها را دوباره با هم ادغام نمی‌کند. این مثال را در نظر بگیرید:

![](linked-list-allocator-fragmentation-on-dealloc.svg)

در خط اول، سه تخصیص روی هیپ ایجاد می‌شوند. دو تای آن‌ها در خط 2 دوباره آزاد می‌شوند و سومی در خط 3 آزاد می‌شود. اکنون کل هیپ دوباره استفاده‌نشده است، اما همچنان به چهار بلوک جداگانه تقسیم شده است. در این نقطه، ممکن است دیگر یک تخصیص بزرگ ممکن نباشد، زیرا هیچ‌یک از این چهار بلوک به‌اندازه کافی بزرگ نیست. با گذر زمان این روند ادامه می‌یابد و هیپ به بلوک‌های کوچک‌تر و کوچک‌تر تقسیم می‌شود. در نقطه‌ای، هیپ چنان تکه‌تکه می‌شود که حتی تخصیص‌های با اندازه معمولی هم شکست می‌خورند.

برای رفع این مشکل، باید بلوک‌های آزادشده مجاور را دوباره با هم ادغام کنیم. برای مثال بالا، این یعنی:

![](linked-list-allocator-merge-on-dealloc.svg)

مانند قبل، دو تا از سه تخصیص در خط `2` آزاد می‌شوند. به‌جای نگه داشتن هیپ تکه‌تکه‌شده، اکنون در خط `2a` گام اضافی‌ای انجام می‌دهیم تا دو بلوک سمت راست دوباره با هم ادغام شوند. در خط `3`، تخصیص سوم (مانند قبل) آزاد می‌شود که نتیجه آن یک هیپ کاملاً استفاده‌نشده است که با سه بلوک مجزا نمایش داده می‌شود. سپس در گام ادغام اضافیِ خط `3a`، سه بلوک مجاور را دوباره با هم ادغام می‌کنیم.

کِرِیت `linked_list_allocator` این راهبرد ادغام را به این شکل پیاده‌سازی می‌کند: به‌جای درج بلوک‌های حافظه آزادشده در ابتدای لیست پیوندی هنگام `deallocate`، همیشه لیست را بر اساس آدرس شروع مرتب نگه می‌دارد. به این ترتیب، ادغام می‌تواند مستقیماً در فراخوانی `deallocate` و با بررسی آدرس‌ها و اندازه‌های دو بلوک همسایه در لیست انجام شود. البته عملیات آزادسازی به این شکل کندتر است، اما از تکه‌تکه شدن هیپ که در بالا دیدیم جلوگیری می‌کند.

#### کارایی

همان‌طور که در بالا آموختیم، تخصیص‌دهنده افزایشی بی‌نهایت سریع است و می‌توان آن را تا حد تنها چند عملیات اسمبلی بهینه کرد. تخصیص‌دهنده لیست پیوندی در این زمینه بسیار بدتر عمل می‌کند. مشکل این است که یک درخواست تخصیص ممکن است ناچار شود کل لیست پیوندی را پیمایش کند تا بلوک مناسبی بیابد.

از آن‌جا که طول لیست به تعداد بلوک‌های حافظه استفاده‌نشده بستگی دارد، کارایی می‌تواند برای برنامه‌های مختلف بسیار متفاوت باشد. برنامه‌ای که تنها چند تخصیص ایجاد می‌کند، کارایی تخصیصِ نسبتاً سریعی را تجربه خواهد کرد. اما برای برنامه‌ای که با تخصیص‌های فراوان هیپ را تکه‌تکه می‌کند، کارایی تخصیص بسیار بد خواهد بود، زیرا لیست پیوندی بسیار طولانی می‌شود و بیشتر شامل بلوک‌های خیلی کوچک است.

شایان ذکر است که این مسئله کارایی، مشکلی نیست که پیاده‌سازی ساده ما ایجاد کرده باشد، بلکه مشکلی بنیادی در رویکرد لیست پیوندی است. از آن‌جا که کارایی تخصیص می‌تواند برای کد سطح هسته بسیار مهم باشد، در ادامه طراحی سومی از تخصیص‌دهنده را بررسی می‌کنیم که کارایی بهتر را در ازای بهره‌وری کمتر حافظه به دست می‌آورد.

## تخصیص‌دهنده بلوک با اندازه ثابت

در ادامه، طراحی تخصیص‌دهنده‌ای را ارائه می‌دهیم که برای برآورده کردن درخواست‌های تخصیص از بلوک‌های حافظه با اندازه ثابت استفاده می‌کند. به این ترتیب، تخصیص‌دهنده اغلب بلوک‌هایی بزرگ‌تر از آنچه برای تخصیص لازم است برمی‌گرداند که به‌دلیل [تکه‌تکه شدن داخلی] به هدر رفتن حافظه منجر می‌شود. از سوی دیگر، زمان لازم برای یافتن یک بلوک مناسب را (در مقایسه با تخصیص‌دهنده لیست پیوندی) به‌شدت کاهش می‌دهد که کارایی تخصیص بسیار بهتری به دنبال دارد.

### مقدمه

ایده پشت _تخصیص‌دهنده بلوک با اندازه ثابت_ چنین است: به‌جای تخصیص دقیقاً همان مقدار حافظه‌ای که درخواست شده، تعداد کمی اندازه بلوک تعریف می‌کنیم و هر تخصیص را به اندازه بلوک بعدی گرد می‌کنیم. برای مثال، با اندازه بلوک‌های 16، 64 و 512 بایت، تخصیص 4 بایت یک بلوک 16 بایتی، تخصیص 48 بایت یک بلوک 64 بایتی و تخصیص 128 بایت یک بلوک 512 بایتی برمی‌گرداند.

مانند تخصیص‌دهنده لیست پیوندی، حافظه استفاده‌نشده را با ساختن یک لیست پیوندی در خودِ آن حافظه دنبال می‌کنیم. با این حال، به‌جای استفاده از یک لیست واحد با اندازه بلوک‌های متفاوت، برای هر رده اندازه یک لیست جداگانه می‌سازیم. در این صورت هر لیست تنها بلوک‌هایی با یک اندازه مشخص را نگه می‌دارد. برای مثال، با اندازه بلوک‌های 16، 64 و 512، سه لیست پیوندی جداگانه در حافظه خواهیم داشت:

![](fixed-size-block-example.svg).

به‌جای یک اشاره‌گر `head` واحد، سه اشاره‌گر سر لیست به نام‌های `head_16`، `head_64` و `head_512` داریم که هر یک به اولین بلوک استفاده‌نشده با اندازه متناظر اشاره می‌کنند. همه گره‌های یک لیست اندازه یکسانی دارند. برای مثال، لیستی که با اشاره‌گر `head_16` آغاز می‌شود تنها شامل بلوک‌های 16 بایتی است. این یعنی دیگر لازم نیست اندازه را در هر گره لیست ذخیره کنیم، چون از پیش با نام اشاره‌گر سر لیست مشخص شده است.

از آن‌جا که هر عنصر در یک لیست اندازه یکسانی دارد، همه عناصر لیست به یک اندازه برای یک درخواست تخصیص مناسب هستند. این یعنی می‌توانیم یک تخصیص را با گام‌های زیر بسیار کارآمد انجام دهیم:

- اندازه تخصیص درخواست‌شده را به اندازه بلوک بعدی گرد کنید. برای مثال، وقتی تخصیص 12 بایت درخواست می‌شود، در مثال بالا اندازه بلوک 16 را انتخاب می‌کنیم.
- اشاره‌گر سر لیست را به دست آورید؛ برای مثال، برای اندازه بلوک 16 باید از `head_16` استفاده کنیم.
- اولین بلوک را از لیست حذف کرده و آن را برگردانید.

مهم‌تر از همه این‌که همیشه می‌توانیم اولین عنصر لیست را برگردانیم و دیگر نیازی به پیمایش کل لیست نیست. بنابراین، تخصیص‌ها بسیار سریع‌تر از تخصیص‌دهنده لیست پیوندی انجام می‌شوند.

#### اندازه بلوک‌ها و حافظه هدررفته

بسته به اندازه بلوک‌ها، با گرد کردن به بالا مقدار زیادی حافظه از دست می‌دهیم. برای مثال، وقتی برای تخصیص 128 بایتی یک بلوک 512 بایتی برگردانده می‌شود، سه‌چهارم حافظه تخصیص‌یافته استفاده‌نشده می‌ماند. با تعریف اندازه بلوک‌های معقول، می‌توان مقدار حافظه هدررفته را تا حدی محدود کرد. برای مثال، هنگام استفاده از توان‌های 2 (4، 8، 16، 32، 64، 128، …) به‌عنوان اندازه بلوک‌ها، می‌توانیم هدررفت حافظه را در بدترین حالت به نصف اندازه تخصیص و در حالت میانگین به یک‌چهارم اندازه تخصیص محدود کنیم.

همچنین رایج است که اندازه بلوک‌ها بر اساس اندازه‌های رایجِ تخصیص در یک برنامه بهینه شوند. برای مثال، می‌توانیم اندازه بلوک 24 را نیز اضافه کنیم تا مصرف حافظه برای برنامه‌هایی که اغلب تخصیص‌های 24 بایتی انجام می‌دهند بهبود یابد. به این ترتیب، اغلب می‌توان مقدار حافظه هدررفته را بدون از دست دادن مزایای کارایی کاهش داد.

#### آزادسازی

آزادسازی نیز مانند تخصیص بسیار کارآمد است و شامل گام‌های زیر می‌شود:

- اندازه تخصیص آزادشده را به اندازه بلوک بعدی گرد کنید. این کار لازم است، زیرا کامپایلر تنها اندازه تخصیص درخواست‌شده را به `dealloc` می‌دهد، نه اندازه بلوکی را که `alloc` برگردانده است. با استفاده از یک تابع تنظیم اندازه یکسان در هر دو متد `alloc` و `dealloc`، می‌توانیم مطمئن شویم که همیشه مقدار درستی از حافظه را آزاد می‌کنیم.
- اشاره‌گر سر لیست را به دست آورید.
- بلوک آزادشده را با به‌روزرسانی اشاره‌گر سر لیست به ابتدای لیست اضافه کنید.

مهم‌تر از همه این‌که برای آزادسازی نیز هیچ پیمایشی از لیست لازم نیست. این یعنی زمان لازم برای یک فراخوانی `dealloc` بدون توجه به طول لیست ثابت می‌ماند.

#### تخصیص‌دهنده جایگزین

با توجه به این‌که تخصیص‌های بزرگ (بیش از 2&nbsp;KB) اغلب نادر هستند، به‌ویژه در هسته سیستم‌عامل‌ها، ممکن است منطقی باشد که برای این تخصیص‌ها به تخصیص‌دهنده دیگری رجوع کنیم. برای مثال، می‌توانیم برای تخصیص‌های بزرگ‌تر از 2048 بایت به یک تخصیص‌دهنده لیست پیوندی رجوع کنیم تا هدررفت حافظه کاهش یابد. از آن‌جا که انتظار می‌رود تنها تعداد بسیار کمی تخصیص در آن اندازه وجود داشته باشد، لیست پیوندی کوچک می‌ماند و تخصیص و آزادسازی همچنان به‌قدر کافی سریع خواهند بود.

#### ایجاد بلوک‌های جدید {#creating-new-blocks}

در بالا همیشه فرض کردیم که همواره بلوک‌های کافی از یک اندازه مشخص در لیست وجود دارد تا همه درخواست‌های تخصیص برآورده شوند. با این حال، در نقطه‌ای لیست پیوندی مربوط به یک اندازه بلوک خالی می‌شود. در این نقطه، دو راه برای ساختن بلوک‌های استفاده‌نشده جدید با اندازه‌ای مشخص جهت برآورده کردن یک درخواست تخصیص وجود دارد:

- یک بلوک جدید از تخصیص‌دهنده جایگزین (در صورت وجود) تخصیص دهید.
- یک بلوک بزرگ‌تر را از لیستی دیگر تقسیم کنید. این کار زمانی بهترین نتیجه را می‌دهد که اندازه بلوک‌ها توانی از دو باشند. برای مثال، یک بلوک 32 بایتی را می‌توان به دو بلوک 16 بایتی تقسیم کرد.

در پیاده‌سازی خود، بلوک‌های جدید را از تخصیص‌دهنده جایگزین تخصیص می‌دهیم، چون پیاده‌سازی آن بسیار ساده‌تر است.

### پیاده‌سازی

اکنون که می‌دانیم یک تخصیص‌دهنده بلوک با اندازه ثابت چگونه کار می‌کند، می‌توانیم پیاده‌سازی خود را آغاز کنیم. ما به پیاده‌سازی تخصیص‌دهنده لیست پیوندی که در بخش قبل ساختیم وابسته نخواهیم بود، بنابراین حتی اگر از پیاده‌سازی تخصیص‌دهنده لیست پیوندی صرف‌نظر کرده باشید، می‌توانید این بخش را دنبال کنید.

#### گره لیست

پیاده‌سازی خود را با ساختن یک نوع `ListNode` در ماژول جدید `allocator::fixed_size_block` آغاز می‌کنیم:

```rust
// in src/allocator.rs

pub mod fixed_size_block;
```

```rust
// in src/allocator/fixed_size_block.rs

struct ListNode {
    next: Option<&'static mut ListNode>,
}
```

این نوع شبیه به نوع `ListNode` در [پیاده‌سازی تخصیص‌دهنده لیست پیوندی] ماست، با این تفاوت که فیلد `size` نداریم. این فیلد لازم نیست، زیرا در طراحی تخصیص‌دهنده بلوک با اندازه ثابت، هر بلوک در یک لیست اندازه یکسانی دارد.

[پیاده‌سازی تخصیص‌دهنده لیست پیوندی]: #the-allocator-type

#### اندازه بلوک‌ها

سپس یک اسلایس ثابت به نام `BLOCK_SIZES` با اندازه بلوک‌هایی که در پیاده‌سازی خود استفاده می‌کنیم تعریف می‌کنیم:

```rust
// in src/allocator/fixed_size_block.rs

/// The block sizes to use.
///
/// The sizes must each be power of 2 because they are also used as
/// the block alignment (alignments must be always powers of 2).
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];
```

به‌عنوان اندازه بلوک‌ها از توان‌های 2 استفاده می‌کنیم، از 8 آغاز می‌شود و تا 2048 ادامه می‌یابد. هیچ اندازه بلوکی کوچک‌تر از 8 تعریف نمی‌کنیم، زیرا هر بلوک باید بتواند هنگام آزاد شدن یک اشاره‌گر 64 بیتی به بلوک بعدی را ذخیره کند. برای تخصیص‌های بزرگ‌تر از 2048 بایت، به یک تخصیص‌دهنده لیست پیوندی رجوع می‌کنیم.

برای ساده کردن پیاده‌سازی، اندازه هر بلوک را به‌عنوان ترازِ لازم آن در حافظه تعریف می‌کنیم. بنابراین یک بلوک 16 بایتی همیشه روی مرز 16 بایتی هم‌تراز است و یک بلوک 512 بایتی روی مرز 512 بایتی. از آن‌جا که ترازها همیشه باید توانی از 2 باشند، این موضوع هر اندازه بلوک دیگری را کنار می‌گذارد. اگر در آینده به اندازه بلوک‌هایی نیاز داشتیم که توانی از 2 نیستند، همچنان می‌توانیم پیاده‌سازی خود را برای این کار تنظیم کنیم (برای مثال با تعریف آرایه دومی به نام `BLOCK_ALIGNMENTS`).

#### نوع تخصیص‌دهنده

با استفاده از نوع `ListNode` و اسلایس `BLOCK_SIZES`، اکنون می‌توانیم نوع تخصیص‌دهنده خود را تعریف کنیم:

```rust
// in src/allocator/fixed_size_block.rs

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}
```

فیلد `list_heads` آرایه‌ای از اشاره‌گرهای `head` است، یکی برای هر اندازه بلوک. این کار با استفاده از `len()` اسلایس `BLOCK_SIZES` به‌عنوان طول آرایه پیاده‌سازی می‌شود. به‌عنوان تخصیص‌دهنده جایگزین برای تخصیص‌هایی بزرگ‌تر از بزرگ‌ترین اندازه بلوک، از تخصیص‌دهنده‌ای که `linked_list_allocator` فراهم می‌کند استفاده می‌کنیم. می‌توانستیم به‌جای آن از `LinkedListAllocator` که خودمان پیاده‌سازی کردیم نیز استفاده کنیم، اما این ایراد را دارد که [بلوک‌های آزادشده را ادغام نمی‌کند].

[بلوک‌های آزادشده را ادغام نمی‌کند]: #merging-freed-blocks

برای ساختن یک `FixedSizeBlockAllocator`، همان توابع `new` و `init` را فراهم می‌کنیم که برای نوع‌های دیگر تخصیص‌دهنده نیز پیاده‌سازی کردیم:

```rust
// in src/allocator/fixed_size_block.rs

impl FixedSizeBlockAllocator {
    /// Creates an empty FixedSizeBlockAllocator.
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe { self.fallback_allocator.init(heap_start, heap_size); }
    }
}
```

تابع `new` تنها آرایه `list_heads` را با گره‌های خالی مقداردهی اولیه می‌کند و یک تخصیص‌دهنده لیست پیوندی [خالی][`empty`] به‌عنوان `fallback_allocator` می‌سازد. ثابت `EMPTY` لازم است تا به کامپایلر Rust بگوییم می‌خواهیم آرایه را با یک مقدار ثابت مقداردهی اولیه کنیم. مقداردهی اولیه مستقیمِ آرایه به‌صورت `[None; BLOCK_SIZES.len()]` کار نمی‌کند، زیرا در آن صورت کامپایلر لازم می‌داند که `Option<&'static mut ListNode>` تِرِیت `Copy` را پیاده‌سازی کند، در حالی که این‌طور نیست. این محدودیتی فعلی در کامپایلر Rust است که ممکن است در آینده برطرف شود.

[`empty`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.empty

تابع ناامن `init` تنها تابع [`init`] مربوط به `fallback_allocator` را فراخوانی می‌کند و هیچ مقداردهی اولیه اضافی روی آرایه `list_heads` انجام نمی‌دهد. در عوض، لیست‌ها را به‌صورت تنبل و در فراخوانی‌های `alloc` و `dealloc` مقداردهی اولیه می‌کنیم.

[`init`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.init

برای راحتی، متد خصوصی‌ای به نام `fallback_alloc` نیز می‌سازیم که با استفاده از `fallback_allocator` تخصیص انجام می‌دهد:

```rust
// in src/allocator/fixed_size_block.rs

use alloc::alloc::Layout;
use core::ptr;

impl FixedSizeBlockAllocator {
    /// Allocates using the fallback allocator.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}
```

نوع [`Heap`] در کِرِیت `linked_list_allocator`، تِرِیت [`GlobalAlloc`] را پیاده‌سازی نمی‌کند (چون [بدون قفل کردن ممکن نیست]). در عوض، متدی به نام [`allocate_first_fit`] فراهم می‌کند که رابط کمی متفاوتی دارد. به‌جای برگرداندن یک `*mut u8` و استفاده از اشاره‌گر تهی برای اعلام خطا، یک `Result<NonNull<u8>, ()>` برمی‌گرداند. نوع [`NonNull`] انتزاعی برای یک اشاره‌گر خام است که تضمین می‌شود اشاره‌گر تهی نباشد. با نگاشت حالت `Ok` به متد [`NonNull::as_ptr`] و حالت `Err` به یک اشاره‌گر تهی، می‌توانیم به‌راحتی این را دوباره به نوع `*mut u8` ترجمه کنیم.

[`Heap`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html
[بدون قفل کردن ممکن نیست]: #globalalloc-and-mutability
[`allocate_first_fit`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.allocate_first_fit
[`NonNull`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html
[`NonNull::as_ptr`]: https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html#method.as_ptr

#### محاسبه اندیس لیست

پیش از پیاده‌سازی تِرِیت `GlobalAlloc`، تابع کمکی‌ای به نام `list_index` تعریف می‌کنیم که کوچک‌ترین اندازه بلوک ممکن را برای یک [`Layout`] داده‌شده برمی‌گرداند:

```rust
// in src/allocator/fixed_size_block.rs

/// Choose an appropriate block size for the given layout.
///
/// Returns an index into the `BLOCK_SIZES` array.
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}
```

بلوک باید دست‌کم اندازه و ترازی را داشته باشد که `Layout` داده‌شده لازم می‌داند. از آن‌جا که تعریف کردیم اندازه بلوک همان ترازِ آن نیز هست، این یعنی `required_block_size` برابر با [بیشینه] ویژگی‌های [`size()`] و [`align()`] چیدمان است. برای یافتن بلوک بزرگ‌تر بعدی در اسلایس `BLOCK_SIZES`، ابتدا از متد [`iter()`] برای گرفتن یک تکرارگر و سپس از متد [`position()`] برای یافتن اندیس اولین بلوکی که دست‌کم به بزرگی `required_block_size` است استفاده می‌کنیم.

[بیشینه]: https://doc.rust-lang.org/core/cmp/trait.Ord.html#method.max
[`size()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.size
[`align()`]: https://doc.rust-lang.org/core/alloc/struct.Layout.html#method.align
[`iter()`]: https://doc.rust-lang.org/std/primitive.slice.html#method.iter
[`position()`]:  https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.position

توجه کنید که خودِ اندازه بلوک را برنمی‌گردانیم، بلکه اندیس آن در اسلایس `BLOCK_SIZES` را برمی‌گردانیم. دلیلش این است که می‌خواهیم از اندیس برگردانده‌شده به‌عنوان اندیس آرایه `list_heads` استفاده کنیم.

#### پیاده‌سازی `GlobalAlloc`

آخرین گام، پیاده‌سازی تِرِیت `GlobalAlloc` است:

```rust
// in src/allocator/fixed_size_block.rs

use super::Locked;
use alloc::alloc::GlobalAlloc;

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        todo!();
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        todo!();
    }
}
```

مانند تخصیص‌دهنده‌های دیگر، تِرِیت `GlobalAlloc` را مستقیماً برای نوع تخصیص‌دهنده خود پیاده‌سازی نمی‌کنیم، بلکه از [پوشش‌دهنده `Locked`] استفاده می‌کنیم تا تغییرپذیری درونی همگام‌شده اضافه شود. از آن‌جا که پیاده‌سازی‌های `alloc` و `dealloc` نسبتاً بزرگ هستند، در ادامه آن‌ها را یکی‌یکی معرفی می‌کنیم.

##### `alloc`

پیاده‌سازی متد `alloc` به این شکل است:

```rust
// in `impl` block in src/allocator/fixed_size_block.rs

unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            match allocator.list_heads[index].take() {
                Some(node) => {
                    allocator.list_heads[index] = node.next.take();
                    node as *mut ListNode as *mut u8
                }
                None => {
                    // no block exists in list => allocate new block
                    let block_size = BLOCK_SIZES[index];
                    // only works if all block sizes are a power of 2
                    let block_align = block_size;
                    let layout = Layout::from_size_align(block_size, block_align)
                        .unwrap();
                    allocator.fallback_alloc(layout)
                }
            }
        }
        None => allocator.fallback_alloc(layout),
    }
}
```

بیایید گام‌به‌گام آن را بررسی کنیم:

ابتدا از متد `Locked::lock` استفاده می‌کنیم تا یک ارجاع تغییرپذیر به نمونه تخصیص‌دهنده پیچیده‌شده به دست آوریم. سپس تابع `list_index` را که همین الان تعریف کردیم فراخوانی می‌کنیم تا اندازه بلوک مناسب برای چیدمان داده‌شده را محاسبه کرده و اندیس متناظر در آرایه `list_heads` را بگیریم. اگر این اندیس `None` باشد، هیچ اندازه بلوکی برای این تخصیص مناسب نیست، بنابراین با استفاده از تابع `fallback_alloc` از `fallback_allocator` بهره می‌گیریم.

اگر اندیس لیست `Some` باشد، تلاش می‌کنیم اولین گره در لیست متناظری را که با `list_heads[index]` آغاز می‌شود، با استفاده از متد [`Option::take`] حذف کنیم. اگر لیست خالی نباشد، وارد شاخه `Some(node)` از دستور `match` می‌شویم، جایی که اشاره‌گر سر لیست را به جانشینِ `node` بیرون‌کشیده‌شده اشاره می‌دهیم (باز هم با استفاده از [`take`][`Option::take`]). در نهایت، اشاره‌گر `node` بیرون‌کشیده‌شده را به‌صورت `*mut u8` برمی‌گردانیم.

[`Option::take`]: https://doc.rust-lang.org/core/option/enum.Option.html#method.take

اگر سر لیست `None` باشد، نشان می‌دهد که لیست بلوک‌ها خالی است. این یعنی باید همان‌طور که [در بالا توضیح داده شد](#creating-new-blocks) یک بلوک جدید بسازیم. برای این کار، ابتدا اندازه بلوک فعلی را از اسلایس `BLOCK_SIZES` می‌گیریم و آن را هم به‌عنوان اندازه و هم به‌عنوان ترازِ بلوک جدید به کار می‌بریم. سپس یک `Layout` جدید از آن می‌سازیم و متد `fallback_alloc` را برای انجام تخصیص فراخوانی می‌کنیم. دلیل تنظیم چیدمان و تراز این است که آن بلوک هنگام آزادسازی به لیست بلوک‌ها اضافه خواهد شد.

#### `dealloc`

پیاده‌سازی متد `dealloc` به این شکل است:

```rust
// in src/allocator/fixed_size_block.rs

use core::{mem, ptr::NonNull};

// inside the `unsafe impl GlobalAlloc` block

unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            let new_node = ListNode {
                next: allocator.list_heads[index].take(),
            };
            // verify that block has size and alignment required for storing node
            assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
            assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
            let new_node_ptr = ptr as *mut ListNode;
            unsafe {
                new_node_ptr.write(new_node);
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
        }
        None => {
            let ptr = NonNull::new(ptr).unwrap();
            unsafe {
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}
```

مانند `alloc`، ابتدا از متد `lock` استفاده می‌کنیم تا یک ارجاع تغییرپذیر به تخصیص‌دهنده بگیریم و سپس از تابع `list_index` تا لیست بلوکِ متناظر با `Layout` داده‌شده را به دست آوریم. اگر اندیس `None` باشد، هیچ اندازه بلوک مناسبی در `BLOCK_SIZES` وجود ندارد که نشان می‌دهد آن تخصیص توسط تخصیص‌دهنده جایگزین ایجاد شده است. بنابراین از [`deallocate`][`Heap::deallocate`] آن استفاده می‌کنیم تا حافظه دوباره آزاد شود. این متد به‌جای `*mut u8` انتظار یک [`NonNull`] را دارد، بنابراین ابتدا باید اشاره‌گر را تبدیل کنیم. (فراخوانی `unwrap` تنها زمانی شکست می‌خورد که اشاره‌گر تهی باشد، که هنگام فراخوانی `dealloc` توسط کامپایلر هرگز نباید رخ دهد.)

[`Heap::deallocate`]: https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html#method.deallocate

اگر `list_index` یک اندیس بلوک برگرداند، باید بلوک حافظه آزادشده را به لیست اضافه کنیم. برای این کار، ابتدا یک `ListNode` جدید می‌سازیم که به سر لیست فعلی اشاره می‌کند (باز هم با استفاده از [`Option::take`]). پیش از نوشتن گره جدید در بلوک حافظه آزادشده، ابتدا وارسی می‌کنیم که اندازه بلوک فعلی که با `index` مشخص شده، اندازه و ترازِ لازم برای ذخیره یک `ListNode` را دارد. سپس نوشتن را با تبدیل اشاره‌گر `*mut u8` داده‌شده به یک اشاره‌گر `*mut ListNode` و سپس فراخوانی متد ناامن [`write`][`pointer::write`] روی آن انجام می‌دهیم. آخرین گام این است که اشاره‌گر سر لیست را، که در حال حاضر `None` است چون `take` را روی آن فراخوانی کرده‌ایم، به `ListNode` تازه‌نوشته‌شده خود تنظیم کنیم. برای این کار، اشاره‌گر خام `new_node_ptr` را به یک ارجاع تغییرپذیر تبدیل می‌کنیم.

[`pointer::write`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write

چند نکته شایان ذکر است:

- ما بین بلوک‌هایی که از یک لیست بلوک تخصیص یافته‌اند و بلوک‌هایی که از تخصیص‌دهنده جایگزین تخصیص یافته‌اند تفاوتی قائل نمی‌شویم. این یعنی بلوک‌های جدیدی که در `alloc` ساخته می‌شوند، هنگام `dealloc` به لیست بلوک‌ها اضافه می‌شوند و در نتیجه تعداد بلوک‌های آن اندازه افزایش می‌یابد.
- متد `alloc` تنها جایی در پیاده‌سازی ماست که بلوک‌های جدید در آن ساخته می‌شوند. این یعنی در ابتدا با لیست‌های بلوکِ خالی شروع می‌کنیم و این لیست‌ها را تنها به‌صورت تنبل و هنگام انجام تخصیص‌هایی با اندازه بلوک متناظرشان پر می‌کنیم.
- به بلوک‌های `unsafe` در `alloc` و `dealloc` نیازی نداریم، هرچند برخی عملیات `unsafe` انجام می‌دهیم. دلیلش این است که Rust در حال حاضر کل بدنه توابع ناامن را به‌عنوان یک بلوک `unsafe` بزرگ در نظر می‌گیرد. از آن‌جا که استفاده از بلوک‌های صریح `unsafe` این مزیت را دارد که آشکار می‌کند کدام عملیات ناامن هستند و کدام نیستند، یک [RFC پیشنهادی](https://github.com/rust-lang/rfcs/pull/2585) برای تغییر این رفتار وجود دارد.

### استفاده از آن

برای استفاده از `FixedSizeBlockAllocator` جدیدمان، باید استاتیک `ALLOCATOR` را در ماژول `allocator` به‌روزرسانی کنیم:

```rust
// in src/allocator.rs

use fixed_size_block::FixedSizeBlockAllocator;

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(
    FixedSizeBlockAllocator::new());
```

از آن‌جا که تابع `init` برای همه تخصیص‌دهنده‌هایی که پیاده‌سازی کردیم رفتار یکسانی دارد، نیازی به تغییر فراخوانی `init` در `init_heap` نداریم.

اکنون وقتی دوباره تست‌های `heap_allocation` خود را اجرا می‌کنیم، همه تست‌ها باید همچنان پاس شوند:

```
> cargo test --test heap_allocation
simple_allocation... [ok]
large_vec... [ok]
many_boxes... [ok]
many_boxes_long_lived... [ok]
```

به نظر می‌رسد تخصیص‌دهنده جدید ما کار می‌کند!

### بحث

اگرچه رویکرد بلوک با اندازه ثابت کارایی بسیار بهتری نسبت به رویکرد لیست پیوندی دارد، هنگام استفاده از توان‌های 2 به‌عنوان اندازه بلوک‌ها تا نیمی از حافظه را هدر می‌دهد. این‌که آیا این معاوضه ارزشش را دارد یا نه، به‌شدت به نوع برنامه بستگی دارد. برای هسته یک سیستم‌عامل که کارایی در آن حیاتی است، رویکرد بلوک با اندازه ثابت گزینه بهتری به نظر می‌رسد.

از منظر پیاده‌سازی، چیزهای گوناگونی هست که می‌توانیم در پیاده‌سازی فعلی خود بهبود دهیم:

- به‌جای این‌که بلوک‌ها را تنها به‌صورت تنبل و با استفاده از تخصیص‌دهنده جایگزین تخصیص دهیم، شاید بهتر باشد لیست‌ها را از پیش پر کنیم تا کارایی تخصیص‌های اولیه بهبود یابد.
- برای ساده کردن پیاده‌سازی، تنها اندازه بلوک‌هایی را مجاز دانستیم که توانی از 2 هستند تا بتوانیم از آن‌ها به‌عنوان تراز بلوک نیز استفاده کنیم. با ذخیره (یا محاسبه) تراز به روشی دیگر، می‌توانستیم اندازه بلوک‌های دلخواه دیگری را هم مجاز کنیم. به این ترتیب، می‌توانستیم اندازه بلوک‌های بیشتری اضافه کنیم، برای مثال برای اندازه‌های رایج تخصیص، تا حافظه هدررفته به کمترین میزان برسد.
- در حال حاضر تنها بلوک‌های جدید می‌سازیم، اما هرگز آن‌ها را دوباره آزاد نمی‌کنیم. این کار به تکه‌تکه شدن منجر می‌شود و ممکن است در نهایت باعث شکست تخصیص برای تخصیص‌های بزرگ شود. شاید منطقی باشد که برای هر اندازه بلوک یک حداکثر طول لیست اعمال کنیم. وقتی به حداکثر طول رسیدیم، آزادسازی‌های بعدی به‌جای افزوده شدن به لیست، با استفاده از تخصیص‌دهنده جایگزین آزاد می‌شوند.
- به‌جای رجوع به یک تخصیص‌دهنده لیست پیوندی، می‌توانستیم تخصیص‌دهنده ویژه‌ای برای تخصیص‌های بزرگ‌تر از 4&nbsp;KiB داشته باشیم. ایده این است که از [صفحه‌بندی] که روی صفحه‌های 4&nbsp;KiB کار می‌کند بهره بگیریم تا یک بلوک پیوسته از حافظه مجازی را به قاب‌های فیزیکی ناپیوسته نگاشت کنیم. به این ترتیب، تکه‌تکه شدن حافظه استفاده‌نشده دیگر برای تخصیص‌های بزرگ مشکل‌ساز نیست.
- با داشتن چنین تخصیص‌دهنده صفحه‌ای، شاید منطقی باشد که اندازه بلوک‌ها را تا 4&nbsp;KiB اضافه کنیم و تخصیص‌دهنده لیست پیوندی را کاملاً کنار بگذاریم. مزیت‌های اصلی این کار، کاهش تکه‌تکه شدن و بهبود پیش‌بینی‌پذیری کارایی، یعنی کارایی بهتر در بدترین حالت، خواهد بود.

[صفحه‌بندی]: @/edition-2/posts/08-paging-introduction/index.fa.md

توجه به این نکته مهم است که بهبودهای پیاده‌سازی که در بالا مطرح شد تنها پیشنهاد هستند. تخصیص‌دهنده‌هایی که در هسته سیستم‌عامل‌ها استفاده می‌شوند معمولاً به‌شدت برای بار کاری خاص آن هسته بهینه شده‌اند، که این کار تنها از طریق پروفایلینگ گسترده ممکن است.

### گونه‌های دیگر

گونه‌های بسیاری از طراحی تخصیص‌دهنده بلوک با اندازه ثابت نیز وجود دارد. دو نمونه محبوب، _تخصیص‌دهنده اسلب_ و _تخصیص‌دهنده بادی_ هستند که در هسته‌های محبوبی مانند لینوکس نیز استفاده می‌شوند. در ادامه، معرفی کوتاهی از این دو طراحی ارائه می‌دهیم.

#### تخصیص‌دهنده اسلب

ایده پشت [تخصیص‌دهنده اسلب] این است که از اندازه بلوک‌هایی استفاده شود که مستقیماً با نوع‌های منتخب در هسته متناظر هستند. به این ترتیب، تخصیص‌های آن نوع‌ها دقیقاً در یک اندازه بلوک جا می‌شوند و هیچ حافظه‌ای هدر نمی‌رود. گاهی حتی ممکن است بتوان نمونه‌های آن نوع‌ها را از پیش در بلوک‌های استفاده‌نشده مقداردهی اولیه کرد تا کارایی بیشتر بهبود یابد.

[تخصیص‌دهنده اسلب]: https://en.wikipedia.org/wiki/Slab_allocation

تخصیص اسلب اغلب با تخصیص‌دهنده‌های دیگر ترکیب می‌شود. برای مثال، می‌توان آن را همراه با یک تخصیص‌دهنده بلوک با اندازه ثابت به کار برد تا یک بلوک تخصیص‌یافته بیشتر تقسیم شود و هدررفت حافظه کاهش یابد. همچنین اغلب برای پیاده‌سازی [الگوی استخر شیء] روی یک تخصیص بزرگ واحد استفاده می‌شود.

[الگوی استخر شیء]: https://en.wikipedia.org/wiki/Object_pool_pattern

#### تخصیص‌دهنده بادی

به‌جای استفاده از یک لیست پیوندی برای مدیریت بلوک‌های آزادشده، طراحی [تخصیص‌دهنده بادی] از ساختار داده [درخت دودویی] همراه با اندازه بلوک‌هایی که توانی از 2 هستند استفاده می‌کند. وقتی بلوک جدیدی با اندازه‌ای مشخص لازم باشد، یک بلوک بزرگ‌تر را به دو نیمه تقسیم می‌کند و در نتیجه دو گره فرزند در درخت ایجاد می‌شود. هر زمان که بلوکی دوباره آزاد شود، بلوک همسایه آن در درخت تحلیل می‌شود. اگر همسایه نیز آزاد باشد، دو بلوک دوباره به هم می‌پیوندند تا بلوکی با دو برابر اندازه تشکیل شود.

مزیت این فرآیند ادغام این است که [تکه‌تکه شدن خارجی] کاهش می‌یابد، به‌طوری که بلوک‌های کوچکِ آزادشده می‌توانند برای یک تخصیص بزرگ دوباره استفاده شوند. همچنین از تخصیص‌دهنده جایگزین استفاده نمی‌کند، بنابراین کارایی آن پیش‌بینی‌پذیرتر است. بزرگ‌ترین ایراد این است که تنها اندازه بلوک‌هایی که توانی از 2 هستند ممکن‌اند، که می‌تواند به‌دلیل [تکه‌تکه شدن داخلی] به هدر رفتن مقدار زیادی حافظه منجر شود. به همین دلیل، تخصیص‌دهنده‌های بادی اغلب با یک تخصیص‌دهنده اسلب ترکیب می‌شوند تا یک بلوک تخصیص‌یافته بیشتر به چند بلوک کوچک‌تر تقسیم شود.

[تخصیص‌دهنده بادی]: https://en.wikipedia.org/wiki/Buddy_memory_allocation
[درخت دودویی]: https://en.wikipedia.org/wiki/Binary_tree
[تکه‌تکه شدن خارجی]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#External_fragmentation
[تکه‌تکه شدن داخلی]: https://en.wikipedia.org/wiki/Fragmentation_(computing)#Internal_fragmentation


## خلاصه

این پست مروری بر طراحی‌های مختلف تخصیص‌دهنده ارائه داد. یاد گرفتیم چگونه یک [تخصیص‌دهنده افزایشی] پایه پیاده‌سازی کنیم که با افزایش دادن یک اشاره‌گر `next` واحد، حافظه را به‌صورت خطی تحویل می‌دهد. اگرچه تخصیص افزایشی بسیار سریع است، تنها پس از آزاد شدن همه تخصیص‌ها می‌تواند از حافظه دوباره استفاده کند. به همین دلیل، به‌ندرت به‌عنوان تخصیص‌دهنده سراسری به کار می‌رود.

[تخصیص‌دهنده افزایشی]: @/edition-2/posts/11-allocator-designs/index.md#bump-allocator

سپس یک [تخصیص‌دهنده لیست پیوندی] ساختیم که از خودِ بلوک‌های حافظه آزادشده برای ایجاد یک لیست پیوندی، یعنی همان [لیست آزاد]، استفاده می‌کند. این لیست امکان ذخیره تعداد دلخواهی از بلوک‌های آزادشده با اندازه‌های مختلف را فراهم می‌کند. اگرچه هیچ حافظه‌ای هدر نمی‌رود، این رویکرد از کارایی ضعیف رنج می‌برد، زیرا یک درخواست تخصیص ممکن است نیازمند پیمایش کامل لیست باشد. پیاده‌سازی ما همچنین از [تکه‌تکه شدن خارجی] رنج می‌برد، چون بلوک‌های آزادشده مجاور را دوباره با هم ادغام نمی‌کند.

[تخصیص‌دهنده لیست پیوندی]: @/edition-2/posts/11-allocator-designs/index.md#linked-list-allocator
[لیست آزاد]: https://en.wikipedia.org/wiki/Free_list

برای رفع مشکلات کاراییِ رویکرد لیست پیوندی، یک [تخصیص‌دهنده بلوک با اندازه ثابت] ساختیم که مجموعه ثابتی از اندازه بلوک‌ها را از پیش تعریف می‌کند. برای هر اندازه بلوک یک [لیست آزاد] جداگانه وجود دارد، به‌طوری که تخصیص و آزادسازی تنها نیاز به درج/برداشت از ابتدای لیست دارند و بنابراین بسیار سریع هستند. از آن‌جا که هر تخصیص به اندازه بلوک بزرگ‌تر بعدی گرد می‌شود، مقداری حافظه به‌دلیل [تکه‌تکه شدن داخلی] هدر می‌رود.

[تخصیص‌دهنده بلوک با اندازه ثابت]: @/edition-2/posts/11-allocator-designs/index.md#fixed-size-block-allocator

طراحی‌های بسیار بیشتری از تخصیص‌دهنده با معاوضه‌های متفاوت وجود دارد. [تخصیص اسلب] برای بهینه‌سازی تخصیص ساختارهای رایج با اندازه ثابت به‌خوبی عمل می‌کند، اما در همه موقعیت‌ها قابل استفاده نیست. [تخصیص بادی] از یک درخت دودویی برای ادغام دوباره بلوک‌های آزادشده استفاده می‌کند، اما مقدار زیادی حافظه را هدر می‌دهد، چون تنها از اندازه بلوک‌هایی که توانی از 2 هستند پشتیبانی می‌کند. همچنین مهم است به یاد داشته باشیم که هر پیاده‌سازی هسته بار کاری منحصربه‌فردی دارد، بنابراین هیچ طراحی «بهترینِ» تخصیص‌دهنده وجود ندارد که برای همه موارد مناسب باشد.

[تخصیص اسلب]: @/edition-2/posts/11-allocator-designs/index.md#slab-allocator
[تخصیص بادی]: @/edition-2/posts/11-allocator-designs/index.md#buddy-allocator


## بعدی چیست؟

با این پست، فعلاً پیاده‌سازی مدیریت حافظه خود را به پایان می‌رسانیم. در ادامه، بررسی [_چندوظیفگی_] را آغاز می‌کنیم و کار را با چندوظیفگی تعاملی در قالب [_async/await_] شروع می‌کنیم. در پست‌های بعدی، سپس [_نخ‌ها_]، [_چندپردازشی_] و [_پروسه‌ها_] را بررسی خواهیم کرد.

[_چندوظیفگی_]: https://en.wikipedia.org/wiki/Computer_multitasking
[_نخ‌ها_]: https://en.wikipedia.org/wiki/Thread_(computing)
[_پروسه‌ها_]: https://en.wikipedia.org/wiki/Process_(computing)
[_چندپردازشی_]: https://en.wikipedia.org/wiki/Multiprocessing
[_async/await_]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html
