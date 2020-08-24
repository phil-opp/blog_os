#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use blog_os::serial_println;
use core::{panic::PanicInfo, ptr, slice};

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut bootloader::boot_info::BootInfo) -> ! {
    #[cfg(test)]
    test_main();

    let mut framebuffer = {
        let ptr = boot_info.framebuffer.start_addr as *mut u8;
        let slice = unsafe { slice::from_raw_parts_mut(ptr, boot_info.framebuffer.len) };
        volatile::Volatile::new(slice)
    };

    //serial_println!("Hello World{}", "!");

    for i in 0..boot_info.framebuffer.len {
        framebuffer.index_mut(i).write(0x99);
    }

    loop {
        x86_64::instructions::hlt();
    }
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
