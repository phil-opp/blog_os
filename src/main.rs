#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use blog_os::multitasking::{self, thread::Thread, with_scheduler};
use blog_os::{print, println};
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::allocator;
    use blog_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    println!("Hello World{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    // allocate a number on the heap
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    // create a dynamically sized vector
    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    // create a reference counted vector -> will be freed when count reaches 0
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!(
        "current reference count is {}",
        Rc::strong_count(&cloned_reference)
    );
    core::mem::drop(reference_counted);
    println!(
        "reference count is {} now",
        Rc::strong_count(&cloned_reference)
    );

    #[cfg(test)]
    test_main();

    let idle_thread = Thread::create(idle_thread, 2, &mut mapper, &mut frame_allocator).unwrap();
    with_scheduler(|s| s.set_idle_thread(idle_thread));

    for _ in 0..10 {
        let thread = Thread::create(thread_entry, 2, &mut mapper, &mut frame_allocator).unwrap();
        with_scheduler(|s| s.add_new_thread(thread));
    }
    let thread =
        Thread::create_from_closure(|| thread_entry(), 2, &mut mapper, &mut frame_allocator)
            .unwrap();
    with_scheduler(|s| s.add_new_thread(thread));

    println!("It did not crash!");
    thread_entry();
}

fn idle_thread() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

fn thread_entry() -> ! {
    let thread_id = with_scheduler(|s| s.current_thread_id()).as_u64();
    for _ in 0..=thread_id {
        print!("{}", thread_id);
        x86_64::instructions::hlt();
    }
    multitasking::exit_thread();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        blog_os::vga_buffer::WRITER.force_unlock();
    }
    println!("{}", info);
    blog_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
