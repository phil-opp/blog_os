#![feature(abi_efiapi)]
#![feature(alloc_error_handler)]
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use core::{alloc::Layout, fmt::Write, mem, panic::PanicInfo, slice};
use uefi::{
    prelude::entry,
    proto::console::gop::GraphicsOutput,
    table::{
        boot::{MemoryDescriptor, MemoryType},
        cfg,
    },
};

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

    let mut config_entries = system_table.config_table().iter();
    let rsdp_addr = config_entries
        .find(|entry| matches!(entry.guid, cfg::ACPI_GUID | cfg::ACPI2_GUID))
        .map(|entry| entry.address);
    writeln!(stdout, "rsdp addr: {:?}", rsdp_addr).unwrap();

    let protocol = system_table
        .boot_services()
        .locate_protocol::<GraphicsOutput>()
        .unwrap()
        .unwrap();
    let gop = unsafe { &mut *protocol.get() };
    writeln!(stdout, "current gop mode: {:?}", gop.current_mode_info()).unwrap();
    writeln!(
        stdout,
        "framebuffer at: {:#p}",
        gop.frame_buffer().as_mut_ptr()
    )
    .unwrap();

    let mmap_storage = {
        let max_mmap_size =
            system_table.boot_services().memory_map_size() + 8 * mem::size_of::<MemoryDescriptor>();
        let ptr = system_table
            .boot_services()
            .allocate_pool(MemoryType::LOADER_DATA, max_mmap_size)?
            .unwrap();
        unsafe { slice::from_raw_parts_mut(ptr, max_mmap_size) }
    };

    uefi::alloc::exit_boot_services();
    let (system_table, memory_map) = system_table
        .exit_boot_services(image, mmap_storage)
        .unwrap()
        .unwrap();

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
