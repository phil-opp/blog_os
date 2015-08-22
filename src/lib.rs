// Copyright 2015 Philipp Oppermann
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![feature(no_std, lang_items, asm)]
#![feature(core_str_ext, const_fn, range_inclusive)]
#![no_std]

extern crate rlibc;
extern crate multiboot2;
#[macro_use]
extern crate bitflags;

use core::fmt::Write;

#[macro_use]
mod vga_buffer;

mod memory;

#[no_mangle]
pub extern fn rust_main(multiboot_address: usize) {
    // ATTENTION: we have a very small stack and no guard page
    use vga_buffer::{Writer, Color};

    vga_buffer::clear_screen();
    let multiboot = unsafe{multiboot2::load(multiboot_address)};
    memory::init(multiboot);


    let mut writer = Writer::new(Color::Blue, Color::LightGreen);
    writer.write_byte(b'H');
    let _ = writer.write_str("ello! ");
    let _ = write!(writer, "The numbers are {} and {}", 42, 1.0/3.0);
    println!("");
    println!("{} {}", "line", 1);
    print!("line {}", 2);

    loop{}
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern fn eh_personality() {}

#[cfg(not(test))]
#[lang = "panic_fmt"]
extern fn panic_fmt() -> ! {loop{}}
