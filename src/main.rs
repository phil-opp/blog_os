#![feature(abi_efiapi)]

#![no_std]
#![no_main]

use core::ffi::c_void;
use core::panic::PanicInfo;

#[no_mangle]
pub extern "efiapi" fn efi_main(image: *mut c_void, system_table: *const c_void) -> usize {
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
