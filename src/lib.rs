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
#![feature(const_fn, unique, core_str_ext, iter_cmp, optin_builtin_traits)]
#![feature(core_intrinsics, core_slice_ext)]
#![no_std]

extern crate rlibc;
extern crate spin;
extern crate multiboot2;
extern crate x86;
#[macro_use]
extern crate bitflags;

#[macro_use]
mod vga_buffer;
mod memory;

#[no_mangle]
pub extern fn rust_main(multiboot_information_address: usize) {
    // ATTENTION: we have a very small stack and no guard page
    vga_buffer::clear_screen();
    println!("Hello World{}", "!");

    let boot_info = unsafe{ multiboot2::load(multiboot_information_address) };
    let memory_map_tag = boot_info.memory_map_tag().expect("Memory map tag required");
    let elf_sections_tag = boot_info.elf_sections_tag().expect("Memory map tag required");

    println!("memory areas:");
    for area in memory_map_tag.memory_areas() {
        println!("    start: 0x{:x}, length: 0x{:x}", area.base_addr, area.length);
    }

    println!("kernel sections:");
    for section in elf_sections_tag.sections() {
        println!("    addr: 0x{:x}, size: 0x{:x}, flags: 0x{:x}",
            section.addr, section.size, section.flags);
    }

    let kernel_start = elf_sections_tag.sections().map(|s| s.addr).min().unwrap();
    let kernel_end = elf_sections_tag.sections().map(|s| s.addr + s.size).max().unwrap();

    let multiboot_start = multiboot_information_address;
    let multiboot_end = multiboot_start + (boot_info.total_size as usize);

    println!("kernel start: 0x{:x}, kernel end: 0x{:x}", kernel_start, kernel_end);
    println!("multiboot start: 0x{:x}, multiboot end: 0x{:x}", multiboot_start, multiboot_end);

    let mut frame_allocator = memory::AreaFrameAllocator::new(kernel_start as usize,
        kernel_end as usize, multiboot_start, multiboot_end, memory_map_tag.memory_areas());


    // println!("outer {}", {println!("inner"); "NO DEADLOCK"});
    /*println!("{:?}", memory::paging::translate::translate(0));*/

    println!("{:?}", memory::paging::translate::translate(0));
    println!("{:?}", memory::paging::translate::translate(0x40000000));
    println!("{:?}", memory::paging::translate::translate(0x40000000 - 1));
    println!("{:?}", memory::paging::translate::translate(0xdeadbeaa000));
    println!("{:?}", memory::paging::translate::translate(0xcafebeaf000));
    memory::paging::test(&mut frame_allocator);
    println!("{:x}", memory::paging::translate::translate(0xdeadbeaa000).unwrap());
    println!("{:x}", memory::paging::translate::translate(0xdeadbeab000).unwrap());
    println!("{:x}", memory::paging::translate::translate(0xdeadbeac000).unwrap());
    println!("{:x}", memory::paging::translate::translate(0xdeadbead000).unwrap());
    println!("{:x}", memory::paging::translate::translate(0xcafebeaf000).unwrap());



    loop{}
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern fn eh_personality() {}

#[cfg(not(test))]
#[lang = "panic_fmt"]
extern fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    println!("\n\nPANIC in {} at line {}:", file, line);
    println!("    {}", fmt);
    loop{}
}
