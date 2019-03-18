use super::MutGlobalAlloc;
use core::alloc::Layout;
use core::mem;
use x86_64::{align_up, VirtAddr};

#[derive(Debug)]
pub struct LinkedListAllocator {
    head: Region,
}

impl LinkedListAllocator {
    pub const fn empty() -> Self {
        let head = Region {
            size: 0,
            next: None,
        };
        Self { head }
    }

    pub unsafe fn new(heap_start: VirtAddr, heap_size: u64) -> Self {
        let mut allocator = Self::empty();
        allocator.add_memory(heap_start, heap_size);
        allocator
    }

    pub fn add_memory(&mut self, start: VirtAddr, size: u64) {
        let aligned = start.align_up(mem::size_of::<Region>() as u64);
        let mut region = Region {
            size: size - (aligned - start),
            next: None
        };
        mem::swap(&mut self.head.next, &mut region.next);

        let region_ptr: *mut Region = aligned.as_mut_ptr();
        unsafe { region_ptr.write(region) };
        self.head.next = Some(unsafe { &mut *region_ptr });
    }
}

impl MutGlobalAlloc for LinkedListAllocator {
    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let size = align_up(layout.size() as u64, mem::size_of::<Region>() as u64);

        let mut current = &mut self.head;
        loop {
            let next = match current.next {
                Some(ref mut next) => next,
                None => break,
            };
            let next_start = VirtAddr::new(*next as *mut Region as u64);
            let next_end = next_start + next.size;

            let alloc_start = next_start.align_up(layout.align() as u64);
            let alloc_end = alloc_start + size;

            // check if Region large enough
            if alloc_end <= next_end {
                // remove Region from list
                let next_next = next.next.take();
                current.next = next_next;
                // insert remaining Region to list
                self.add_memory(alloc_end, next_end - alloc_end);
                // return allocated memory
                return alloc_start.as_mut_ptr();
            }

            // continue with next element
            //
            // This is basically `current = next`, but we need a new `match` expression because
            // the compiler can't figure the lifetimes out when we use the `next` binding
            // from above.
            current = match current.next {
                Some(ref mut next) => next,
                None => unreachable!(),
            };
        }

        // no large enough Region found
        0 as *mut u8
    }

    fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let size = align_up(layout.size() as u64, mem::size_of::<Region>() as u64);
        self.add_memory(VirtAddr::new(ptr as u64), size);
    }
}

#[derive(Debug)]
struct Region {
    size: u64,
    next: Option<&'static mut Region>,
}


// TODO recycle alignment