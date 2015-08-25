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

#![feature(no_std, lang_items)]
#![feature(core_slice_ext, core_str_ext, core_intrinsics)]
#![no_std]

extern crate rlibc;

use core::intrinsics::offset;

#[no_mangle]
pub extern fn main() {
    // ATTENTION: we have a very small stack and no guard page
    let x = ["Hello", " ", "World", "!"];
    let screen_pointer = 0xb8000 as *const u16;

    for (byte, i) in x.iter().flat_map(|s| s.bytes()).zip(0..) {
        let c = 0x1f00 | (byte as u16);
        unsafe {
            let screen_char = offset(screen_pointer, i) as *mut u16;
            *screen_char = c
        }
    }

    loop{}
}

#[lang = "eh_personality"] extern fn eh_personality() {}
#[lang = "panic_fmt"] extern fn panic_fmt() -> ! {loop{}}
