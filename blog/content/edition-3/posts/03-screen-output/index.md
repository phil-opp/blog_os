+++
title = "Screen Output"
weight = 3
path = "screen-output"
date = 0000-01-01
draft = true

[extra]
chapter = "Basic I/O"
icon = '''<svg xmlns="http://www.w3.org/2000/svg" fill="currentColor" class="bi bi-display" viewBox="0 0 16 16">
  <path d="M0 4s0-2 2-2h12s2 0 2 2v6s0 2-2 2h-4c0 .667.083 1.167.25 1.5H11a.5.5 0 0 1 0 1H5a.5.5 0 0 1 0-1h.75c.167-.333.25-.833.25-1.5H2s-2 0-2-2V4zm1.398-.855a.758.758 0 0 0-.254.302A1.46 1.46 0 0 0 1 4.01V10c0 .325.078.502.145.602.07.105.17.188.302.254a1.464 1.464 0 0 0 .538.143L2.01 11H14c.325 0 .502-.078.602-.145a.758.758 0 0 0 .254-.302 1.464 1.464 0 0 0 .143-.538L15 9.99V4c0-.325-.078-.502-.145-.602a.757.757 0 0 0-.302-.254A1.46 1.46 0 0 0 13.99 3H2c-.325 0-.502.078-.602.145z"/>
</svg>'''
+++

In the previous post, we learned how to draw shapes and pixels directly onto the framebuffer. That's fine and all, but how is one able to go from that to displaying text on the screen? Understanding this requires taking a deep dive into how characters are rendered behind the scenes.

When a key is pressed on the keyboard, it sends a character code to the CPU. It's the CPU's job at that point to then interpret the character code and match it with an image to draw on the screen. The image is then sent to either the GPU or the framebuffer (the latter in our case) to be drawn on the screen, and the user sees that image as a letter, number, CJK character, emoji, or whatever else he or she wanted to have displayed by pressing that key.

In most other programming languages, implementing this behind the scenes can be a daunting task. With Rust, however, we have a toolset at our disposal that can pave the way for setting up proper framebuffer logging using very little code of our own.

# The `log` crate

Rust developers used to writing user-mode code will recognize the `log` crate from a mile away:

```toml
# in Cargo.toml
[dependencies]
log = { version = "0.4.17", default-features = false }
```

This crate has both a set of macros for logging either to the console or to a log file for later reading and a trait — also called `Log` with a capital L — that can be implemented to provide a backend, called a `Logger` in Rust parlance. Loggers are provided by a myriad of crates for a wide variety of use cases, and some of them even run on bare metal. We already used one such extant logger in the UEFI booting module when we used the logger provided by the `uefi` crate to print text to the UEFI console. That won't work in the kernel, however, because UEFI boot services need to be active in order for the UEFI logger to be usable.

If you were paying attention to the post before that one, however, you may have noticed that the bootloader is itself able to log directly to the framebuffer as it did when we booted the barebones kernel for the first time, and unlike the UEFI console logger, this logger is usable long after UEFI boot services are exited. It's this logger, therefore, that provides the easiest means of implementation on our end.

## `bootloader-x86_64-common`

In version 0.11.x of the bootloader crate, each component is separate, unlike in 0.10.x where the bootloader was a huge monolith. This is fantastic as it means that a lot of the APIs that the bootloader uses behind the scenes are also free for kernels to use, including, of course, the logger. The set of APIs that the logger belongs to are in a crate called `bootloader-x86_64-common` which also contains some other useful abstractions that will come in handy later:

```toml
# in Cargo.toml
[dependencies]
bootloader-x86_64-common = "0.11.3"
```

For now, however, only the logger will be used. If you are curious as to how this logger is written behind the scenes, however, don't worry; a sub-module of this chapter will include a tutorial on how to write a custom logger from scratch.

# Putting it all together

Before we use the bootloader's logger, we first need to initialize it. This requires creating a static instance, since it needs to live for as long as the kernel lives — which would mean for as long as the computer is powered on. Unfortunately, this is easier said than done, as Rust statics can be rather finicky — understandably so for security reasons. Luckily, there's a crate for this too.

## The `conquer_once` crate

Those used to using the standard library know that it provides a `OnceCell` which is exactly what it sounds like: you write to it only once, and then after that it's just there to use whenever. We're in a kernel and don't have access to the standard library, however, so is there a crate on crates.io that provides a replacement? Ah, yes there is:

```toml
# in Cargo.toml
[dependencies]
conquer-once = { version = "0.4.0", default-features = false }
```

Note that we need to add `default-features = false` to our `conquer-once` dependency —that's because the [`conquer-once` crate](https://crates.io/crates/conquer-once) tries to pull in the standard library by default, which in the kernel will result in compilation errors.

Now that we've added our two dependencies, it's time to use them:

```rust
// in src/main.rs
use conquer_once::spin::OnceCell;
use bootloader_x86_64_common::logger::LockedLogger;
// ...
pub(crate) static LOGGER: OnceCell<LockedLogger> = OnceCell::uninit();
```

By setting the logger up as a static `OnceCell` it becomes much easier to initialize. We use `pub(crate)` to ensure that the kernel can see it but nothing else can.

After this, it's time to actually initialize it. To do that, we use a function:

```rust
// in src/main.rs
use bootloader_api::info::FrameBufferInfo;
// ...
pub(crate) fn init_logger(buffer: &'static mut [u8], info: FrameBufferInfo) {
    let logger = LOGGER.get_or_init(move || LockedLogger::new(buffer, info, true, false));
    log::set_logger(logger).expect("Logger already set");
    log::set_max_level(log::LevelFilter::Trace);
    log::info!("Hello, Kernel Mode!");
}
```

This function takes two parameters: a byte slice representing a raw framebuffer and a `FrameBufferInfo` structure containing information about the first parameter. Getting those parameters, however, requires jumping through some hoops to satisfy the borrow checker:

```rust
// in src/main.rs
fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    // ...
    // free the doubly wrapped framebuffer from the boot info struct
    let frame_buffer_optional = &mut boot_info.framebuffer;
    
    // free the wrapped framebuffer from the FFI-safe abstraction provided by bootloader_api
    let frame_buffer_option = frame_buffer_optional.as_mut();
    
    // unwrap the framebuffer
    let frame_buffer_struct = frame_buffer_option.unwrap();
    
    // extract the framebuffer info and, to satisfy the borrow checker, clone it
    let frame_buffer_info = frame_buffer.info().clone();
    
    // get the framebuffer's mutable raw byte slice
    let raw_frame_buffer = frame_buffer_struct.buffer_mut();
    
    // finally, initialize the logger using the last two variables
    init_logger(raw_frame_buffer, frame_buffer_info);
    // ...
}
```

Any one of these steps, if skipped, will cause the borrow checker to throw a hissy fit due to the use of the `move ||` closure by the initializer function. With this, however, you're done, and you'll know the logger has been initialized when you see "Hello, Kernel Mode!" printed on the screen.

<!-- more -->
<!-- toc -->

<!-- TODO: update relative link in 02-booting/uefi/index.md when this post is finished -->
