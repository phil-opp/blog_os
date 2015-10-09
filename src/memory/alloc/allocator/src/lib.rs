#![feature(no_std, allocator)]
#![no_std]
#![allocator]
#![allow(improper_ctypes)]

extern {
    fn __rust_allocate(size: usize, align: usize) -> *mut u8;
    fn __rust_deallocate(ptr: *mut u8, old_size: usize, align: usize);
    fn __rust_reallocate(ptr: *mut u8, old_size: usize, size: usize,
                         align: usize) -> *mut u8;
    fn __rust_reallocate_inplace(ptr: *mut u8, old_size: usize, size: usize,
                                 align: usize) -> usize;
    fn __rust_usable_size(size: usize, align: usize) -> usize;
}
