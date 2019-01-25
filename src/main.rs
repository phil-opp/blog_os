#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(test, allow(unused_imports))]

use blog_os::println;
use core::panic::PanicInfo;

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    use blog_os::interrupts::PICS;

    let level_4_table_pointer = 0xffff_ffff_ffff_f000 as *const u64;
    for i in 0..10 {
        let entry = unsafe { *level_4_table_pointer.offset(i) };
        println!("Entry {}: {:#x}", i, entry);
    }

    println!("Hello World{}", "!");

    blog_os::gdt::init();
    blog_os::interrupts::init_idt();
    unsafe { PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();

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
