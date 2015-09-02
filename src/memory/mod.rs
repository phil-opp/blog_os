use multiboot2::Multiboot;
use core::iter::range_inclusive;
use core::cmp::max;

mod paging;

pub fn init(multiboot: &Multiboot) {
    // ATTENTION: we have a very small stack and no guard page

    let kernel_end = multiboot.elf_tag().unwrap().sections().map(|s| s.addr + s.size).max()
        .unwrap() as usize;
    let multiboot_end = multiboot as *const _ as usize + multiboot.total_size as usize;
    let mut allocator = FrameAllocator::new(max(kernel_end, multiboot_end));
    let mut c = unsafe{paging::Controller::new(allocator)};

    c.begin_new_table();
    c.identity_map(Page{number: 0xb8}, true, false);


    for section in multiboot.elf_tag().unwrap().sections() {
        let in_memory = section.flags & 0x2 != 0;
        let writable = section.flags & 0x1 != 0;
        let executable = section.flags & 0x4 != 0;
        if !in_memory {
            continue;
        }
        println!("section at 0x{:x}, allocated: {}, writable: {}, executable: {}", section.addr,
            in_memory, writable, executable);
        let start_page = Page::containing_address(section.addr as usize);
        let end_page = Page::containing_address((section.addr + section.size) as usize);
        for page in range_inclusive(start_page.number, end_page.number).map(|n| Page{number: n}) {
            c.identity_map(page, writable, executable);
        }
    }
    c.activate_new_table();
}

struct VirtualAddress(*const u8);

struct FrameAllocator {
    next_free_frame: usize,
}

impl FrameAllocator {
    fn new(kernel_end: usize) -> FrameAllocator {
        assert!(kernel_end > 0x100000);
        FrameAllocator {
            next_free_frame: ((kernel_end - 1) >> 12) + 1,
        }
    }

    fn allocate_frame(&mut self) -> Option<Frame> {
        let page_number = self.next_free_frame;
        self.next_free_frame += 1;
        Some(Frame {
            number: page_number
        })
    }
}

#[derive(PartialOrd, Ord, PartialEq, Eq, Clone, Copy)]
struct Frame {
    number: usize,
}

#[derive(PartialOrd, Ord, PartialEq, Eq, Clone, Copy)]
struct Page {
    number: usize,
}

impl Page {
    fn containing_address(address: usize) -> Page {
        Page {
            number: address >> 12,
        }
    }
}
