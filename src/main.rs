#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(test, allow(unused_imports))]

use blog_os::println;
use core::panic::PanicInfo;

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    use blog_os::interrupts::PICS;
    use blog_os::memory::translate_addr;

    println!("Hello World{}", "!");

    blog_os::gdt::init();
    blog_os::interrupts::init_idt();
    unsafe { PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();

    const LEVEL_4_TABLE_ADDR: usize = 0o_177777_777_777_777_777_0000;

    // the identity-mapped vga buffer page
    println!("0xb8000 -> {:?}", translate_addr(0xb8000, LEVEL_4_TABLE_ADDR));
    // some code page
    println!("0x20010a -> {:?}", translate_addr(0x20010a, LEVEL_4_TABLE_ADDR));
    // some stack page
    println!("0x57ac001ffe48 -> {:?}", translate_addr(0x57ac001ffe48,
        LEVEL_4_TABLE_ADDR));

    println!("It did not crash!");
    blog_os::hlt_loop();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    blog_os::hlt_loop();
}
