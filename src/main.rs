#![no_std]
#![no_main]

use core::panic::PanicInfo;

bootloader_api::entry_point!(kernel_main);

fn kernel_main(bootinfo: &'static mut bootloader_api::BootInfo) -> ! {
    loop {}
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
