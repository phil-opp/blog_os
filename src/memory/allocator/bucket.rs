use super::MutGlobalAlloc;
use core::alloc::Layout;
use core::mem::size_of;
use core::fmt::{self, Debug};

#[derive(Debug)]
pub struct BucketAllocator<A> where A: MutGlobalAlloc + Debug {
    underlying: A,
    buckets: [Bucket; 10],
}

impl<A> BucketAllocator<A> where A: MutGlobalAlloc + Debug {
    pub const fn new(underlying: A) -> Self {
        Self {
            underlying,
            buckets: [
                Bucket::new(size_of::<Region>()),
                Bucket::new(16),
                Bucket::new(32),
                Bucket::new(64),
                Bucket::new(128),
                Bucket::new(256),
                Bucket::new(512),
                Bucket::new(1024),
                Bucket::new(2048),
                Bucket::new(4096),
            ]
        }
    }

    pub fn underlying(&mut self) -> &mut A {
        &mut self.underlying
    }
}

pub struct Bucket {
    size: usize,
    head: Option<&'static mut Region>,
}

impl Bucket {
    const fn new(size: usize) -> Self {
        Bucket {
            size,
            head: None,
        }
    }
}

impl fmt::Debug for Bucket {
    fn fmt(&self, f: &mut fmt::Formatter)-> fmt::Result {
        let mut regions = 0;
        let mut current = &self.head;
        while let Some(region) = current {
            current = &region.next;
            regions += 1;
        }
        f.debug_struct("Bucket").field("size", &self.size).field("regions", &regions).finish()
    }
}

#[derive(Debug)]
struct Region {
    next: Option<&'static mut Region>,
}

impl Region {
    fn new() -> Self {
        Self {
            next: None,
        }
    }

    fn as_mut_u8(&'static mut self) -> *mut u8 {
        self as *mut Region as *mut u8
    }

    unsafe fn from_mut_u8(ptr: *mut u8) -> &'static mut Self {
        (ptr as *mut Region).write(Region::new());
        &mut *(ptr as *mut Region)
    }
}

impl<A> BucketAllocator<A> where A: MutGlobalAlloc + Debug {
    fn get_bucket_index(&self, size: usize) -> Option<usize> {
        match self.buckets.binary_search_by(|bucket| bucket.size.cmp(&size)) {
            Ok(index) => Some(index),
            Err(index) if index < self.buckets.len() => Some(index),
            Err(_) => None,
        }
    }
}

impl<A> MutGlobalAlloc for BucketAllocator<A> where A: MutGlobalAlloc + Debug {
    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        if let Some(bucket_index) = self.get_bucket_index(layout.size()) {
            let bucket = &mut self.buckets[bucket_index];
            if let Some(head) = bucket.head.take() {
                let next = head.next.take();
                bucket.head = next;
                return head.as_mut_u8();
            }
        }
        self.underlying.alloc(layout)
    }

    fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        if let Some(bucket_index) = self.get_bucket_index(layout.size()) {
            let bucket = &mut self.buckets[bucket_index];
            let region = unsafe {Region::from_mut_u8(ptr)};
            region.next = bucket.head.take();
            bucket.head = Some(region);
        } else {
            self.underlying.dealloc(ptr, layout);
        }
    }
}