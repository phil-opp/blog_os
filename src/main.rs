#![feature(abi_efiapi)]
#![feature(alloc_error_handler)]
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use core::{alloc::Layout, fmt::Write, panic::PanicInfo};
use uefi::prelude::entry;

#[entry]
fn efi_main(
    image: uefi::Handle,
    system_table: uefi::table::SystemTable<uefi::table::Boot>,
) -> uefi::Status {
    let stdout = system_table.stdout();
    stdout.clear().unwrap().unwrap();
    writeln!(stdout, "Hello World!").unwrap();

    unsafe {
        uefi::alloc::init(system_table.boot_services());
    }

    writeln!(stdout, "alloc").unwrap();
    let mut v: Vec<u32> = Vec::new();
    v.push(1);
    v.push(2);
    writeln!(stdout, "v = {:?}", v).unwrap();

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    panic!("out of memory")
}
