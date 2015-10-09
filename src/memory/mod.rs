use multiboot2::Multiboot;
use self::paging::Page;

mod alloc;
mod paging;
mod frame_allocator;
mod tlb;

pub const PAGE_SIZE: u64 = 4096;

pub fn init(multiboot: &Multiboot) {
    // ATTENTION: we have a very small stack and no guard page
    use core::cmp::max;
    use self::frame_allocator::FrameAllocator;

    let kernel_end = multiboot.elf_tag().unwrap().sections().map(|s| s.addr + s.size).max()
        .unwrap() as usize;
    let multiboot_end = multiboot as *const _ as usize + multiboot.total_size as usize;
    let mut bump_pointer = BumpPointer::new(max(kernel_end, multiboot_end));

    let mut lock = unsafe{ paging::Lock::new() };
    let new_p4_frame = bump_pointer.allocate_frame(&mut lock).expect("failed allocating
        new_p4_frame");

    unsafe{lock.begin_new_table_on_identity_mapped_frame(new_p4_frame)};
    identity_map_kernel_sections(multiboot, lock.mapper(&mut bump_pointer));
    lock.activate_current_table();

    init_core_map(multiboot, &mut lock, bump_pointer);

    let maximal_memory = multiboot.memory_area_tag().unwrap().areas().map(
        |area| area.base_addr + area.length).max().unwrap();
    println!("maximal_memory: 0x{:x}", maximal_memory);

}


fn identity_map_kernel_sections<T>(multiboot: &Multiboot, mut mapper: paging::Mapper<T>)
    where T: frame_allocator::FrameAllocator,
{
    use core::iter::range_inclusive;

    for section in multiboot.elf_tag().expect("no section tag").sections() {
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
        for page in range_inclusive(start_page.number, end_page.number)
            .map(|n| Page{number: n})
        {
            unsafe{ mapper.identity_map(page, writable, executable) };
        }
    }

    // identity map VGA text buffer
    unsafe {
        mapper.identity_map(Page::containing_address(0xb8000), true, false);
    }

    // identity map Multiboot structure
    let multiboot_address = multiboot as *const _ as usize;
    let start_page = Page::containing_address(multiboot_address);
    let end_page = Page::containing_address(multiboot_address + multiboot.total_size as usize);
    for page in range_inclusive(start_page.number, end_page.number).map(|n| Page{number: n}) {
        unsafe{ mapper.identity_map(page, false, false) };
    }
}

fn init_core_map(multiboot: &Multiboot, lock: &mut paging::Lock, mut bump_pointer: BumpPointer) {
    use core::iter::range_inclusive;
    use self::frame_allocator::{FrameAllocator, DynamicFrameStack};


    const CORE_MAP_PAGE: Page = Page{number: 0o_001_000_000};

    lock.mapper(&mut bump_pointer).map(CORE_MAP_PAGE, true, false);
    let mut frame_stack = DynamicFrameStack::new(CORE_MAP_PAGE);

    for area in multiboot.memory_area_tag().expect("no memory tag").areas() {
        println!("area start {:x} length {:x}", area.base_addr, area.length);
        let start_frame = Frame::containing_address(area.base_addr as usize);
        let end_frame = Frame::containing_address((area.base_addr + area.length) as usize);
        for frame in range_inclusive(start_frame.number, end_frame.number)
            .map(|n| Frame{number:n})
        {
            let page = Page{number: frame.number};
            if page.is_unused() && !bump_pointer.has_allocated(frame) {
                frame_stack.deallocate_frame(lock, frame)
            }
        }
    }
}

#[derive(Debug)]
struct BumpPointer {
    first_free_frame: usize,
    next_free_frame: usize,
}

impl frame_allocator::FrameAllocator for BumpPointer {
    fn allocate_frame(&mut self, _: &mut paging::Lock) -> Option<Frame> {
        self.allocate_frames(1)
    }
    fn deallocate_frame(&mut self, _: &mut paging::Lock, _: Frame) {}
}

impl BumpPointer {
    fn new(kernel_end: usize) -> BumpPointer {
        assert!(kernel_end > 0x100000);
        let frame = ((kernel_end - 1) >> 12) + 1;
        BumpPointer {
            first_free_frame: frame,
            next_free_frame: frame,
        }
    }

    fn allocate_frames(&mut self, number: usize) -> Option<Frame> {
        let page_number = self.next_free_frame;
        self.next_free_frame += number;
        Some(Frame {
            number: page_number
        })
    }

    fn has_allocated(&self, frame: Frame) -> bool {
        frame.number >= self.first_free_frame && frame.number < self.next_free_frame
    }
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Clone, Copy)]
struct Frame {
    number: usize,
}

impl Frame {
    fn containing_address(address: usize) -> Frame {
        Frame {
            number: address >> 12,
        }
    }
}
