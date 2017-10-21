// Copyright 2016 Philipp Oppermann. See the README.md
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(const_fn)]
#![feature(allocator_api)]
#![feature(alloc)]
#![feature(global_allocator)]

#![no_std]

use alloc::heap::{Alloc, AllocErr, Layout};
use spin::Mutex;

extern crate alloc;
extern crate spin;


pub const HEAP_START: usize = 0o_000_001_000_000_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

static HEAP: Mutex<Heap> = Mutex::new(Heap::new(0, 0));

//Set up the heap
pub unsafe fn init(offset: usize, size: usize) {
    *HEAP.lock() = Heap::new(offset, size);
}

#[derive(Debug)]
struct Heap {
    start: usize,
    end: usize,
    next: usize,
}


impl Heap {
    /// Initialisation of the heap to use the 
    /// range [start, start + size).
    const fn new(start: usize, size: usize) -> Heap {
        Heap {
            start: start,
            end: start + size,
            next: start,
        }
    }
}

pub struct Allocator;

unsafe impl<'a> Alloc for &'a Allocator {

    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        let ref mut heap = HEAP.lock();
        let alloc_start = align_up(heap.next, layout.align());
        let alloc_end = alloc_start.saturating_add(layout.size());

        if alloc_end <= heap.end {
            heap.next =  alloc_end;
            Ok(alloc_start as *mut u8)
        } else {
            Err(AllocErr::Exhausted{request: layout})
        }        
    }

    unsafe fn dealloc(&mut self, _ptr: *mut u8, _layout: Layout) {
        // Sofar nothing - don't worry, RAM is cheap
    }
}


/// Align downwards. Returns the greatest x with alignment `align`
/// so that x <= addr. The alignment must be a power of 2.
pub fn align_down(addr: usize, align: usize) -> usize {
    if align.is_power_of_two() {
        addr & !(align - 1)
    } else if align == 0 {
        addr
    } else {
        panic!("`align` must be a power of 2");
    }
}

/// Align upwards. Returns the smallest x with alignment `align`
/// so that x >= addr. The alignment must be a power of 2.
pub fn align_up(addr: usize, align: usize) -> usize {
    align_down(addr + align - 1, align)
}

#[global_allocator]
static GLOBAL_ALLOC: Allocator = Allocator;

