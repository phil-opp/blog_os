#![feature(allocator)]
#![feature(const_fn)]

#![allocator]
#![no_std]

use spin::Mutex;
use linked_list_allocator::Heap;

extern crate spin;
extern crate linked_list_allocator;
#[macro_use]
extern crate once;

pub const HEAP_START: usize = 0o_000_001_000_000_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

lazy_static! {
    static ref HEAP: Mutex<Heap> = Mutex::new(unsafe {
        Heap::new(HEAP_START, HEAP_SIZE)
    });
}

#[no_mangle]
pub extern fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    HEAP.lock().allocate_first_fit(size, align).expect("out of memory")
}

#[no_mangle]
pub extern fn __rust_deallocate(ptr: *mut u8, size: usize, align: usize) {
    unsafe { HEAP.lock().deallocate(ptr, size, align) };
}

#[no_mangle]
pub extern fn __rust_usable_size(size: usize, _align: usize) -> usize {
    size
}

#[no_mangle]
pub extern fn __rust_reallocate_inplace(_ptr: *mut u8, size: usize,
    _new_size: usize, _align: usize) -> usize
{
    size
}

#[no_mangle]
pub extern fn __rust_reallocate(ptr: *mut u8, size: usize, new_size: usize,
                                align: usize) -> *mut u8 {
    use core::{ptr, cmp};

    // from: https://github.com/rust-lang/rust/blob/
    //     c66d2380a810c9a2b3dbb4f93a830b101ee49cc2/
    //     src/liballoc_system/lib.rs#L98-L101

    let new_ptr = __rust_allocate(new_size, align);
    unsafe { ptr::copy(ptr, new_ptr, cmp::min(size, new_size)) };
    __rust_deallocate(ptr, size, align);
    new_ptr
}
