#![feature(abi_efiapi)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use uefi::prelude::entry;

#[entry]
fn efi_main(
    image: uefi::Handle,
    system_table: uefi::table::SystemTable<uefi::table::Boot>,
) -> uefi::Status {
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
