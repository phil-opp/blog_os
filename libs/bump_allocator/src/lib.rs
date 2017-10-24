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


struct LockedHeap {
    heap: Mutex<Heap>,
}


#[global_allocator]
static GLOBAL_ALLOC: LockedHeap = LockedHeap::empty();


pub unsafe fn init(start: usize, size: usize) {
    GLOBAL_ALLOC.init(start, size);
}

/// The heap is protected by the LockedHeap structure.
impl LockedHeap {
    /// Creates a protected empty heap. All allocate calls will return
    /// 'AllocErr`.
    pub const fn empty() -> LockedHeap {
        LockedHeap {
            heap : Mutex::new(Heap::empty())
        }
    }
    /// Initializes the heap. 
    unsafe fn init(&self, start: usize, size: usize)  {
        self.heap.lock().init(start, size);
    }
}

/// The interface used for all allocation of heap structures. 
unsafe impl<'a> Alloc for &'a LockedHeap {

    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        self.heap.lock().allocate(layout)
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        self.heap.lock().dealloc(ptr, layout)
    }
}



/// A fixed size heap with a reference to the beginning of free space.
pub struct Heap {
    start: usize,
    end: usize,
    next:  usize,
}

impl Heap {
    /// Creates an empty heap.
    ///
    /// All allocate calls will return `AllocErr`.
    pub const fn empty() -> Heap {
        Heap {
            start: 0,
            end: 0,
            next: 0,
        }
    }

    /// Initalizes the heap given start and size.
    ///
    /// # Safety
    ///
    /// This is unsafe, the start address must be valid and the memory
    /// in the `[start, start + size)` range must not be used for
    /// anything else. The function is unsafe because it can cause
    /// undefined behavior if the given address or size are invalid.
    unsafe fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.next = start;
    }

    /// Allocates a chunk of the given size with the given alignment.
    ///
    /// Returns a pointer to the beginning of that chunk if it was
    /// successful, else it returns an AllocErr.
    unsafe fn allocate(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        let alloc_start = align_up(self.next, layout.align());
        let alloc_end = alloc_start.saturating_add(layout.size());

        if alloc_end <= self.end {
            self.next =  alloc_end;
            Ok(alloc_start as *mut u8)
        } else {
            Err(AllocErr::Exhausted{request: layout})
        }        
    }

    /// Deallocates the block refered to by the given pointer and
    /// described by the layout.
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


