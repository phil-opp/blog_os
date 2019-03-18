#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(test, allow(unused_imports))]
#![feature(alloc)]
#![feature(alloc_error_handler)]

extern crate alloc;

use alloc::vec::Vec;
use blog_os::{
    memory::allocator::{BumpAllocator, LinkedListAllocator, LockedAllocator, BucketAllocator},
    println,
};
use bootloader::{entry_point, BootInfo};
use core::alloc::Layout;
use core::panic::PanicInfo;

entry_point!(kernel_main);

#[cfg(not(test))]
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::interrupts::PICS;
    use blog_os::memory;
    use x86_64::{structures::paging::Page, VirtAddr};

    println!("Hello World{}", "!");

    blog_os::gdt::init();
    blog_os::interrupts::init_idt();
    unsafe { PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();

    let mut mapper = unsafe { memory::init(boot_info.physical_memory_offset) };
    let mut frame_allocator = memory::init_frame_allocator(&boot_info.memory_map);

    let heap_start = VirtAddr::new(HEAP_START);
    let heap_end = VirtAddr::new(HEAP_END);
    memory::map_heap(heap_start, heap_end, &mut mapper, &mut frame_allocator)
        .expect("map_heap failed");

    ALLOCATOR.lock().underlying().add_memory(heap_start, HEAP_END - HEAP_START);

    //let mut x = Vec::with_capacity(1000);
    let mut x = Vec::new();
    for i in 0..1000 {
        x.push(i);
    }
    println!("{:?}", *ALLOCATOR.lock());
    println!("with vec of size {}: {}", x.len(), x.iter().sum::<i32>());
    println!("with formular: {}", 999 * 1000 / 2);

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

const HEAP_START: u64 = 0o_001_000_000_0000;
const HEAP_END: u64 = HEAP_START + 10 * 0x1000;

#[global_allocator]
static ALLOCATOR: LockedAllocator<BucketAllocator<LinkedListAllocator>> =
    LockedAllocator::new(BucketAllocator::new(LinkedListAllocator::empty()));

#[alloc_error_handler]
fn out_of_memory(layout: Layout) -> ! {
    panic!("out of memory: allocation for {:?} failed", layout);
}
