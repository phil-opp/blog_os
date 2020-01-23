use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PhysFrame, Size4KiB,
        UnusedPhysFrame,mapper,
    },
    PhysAddr, VirtAddr,
};

/// Initialize a new OffsetPageTable.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackBounds {
    start: VirtAddr,
    end: VirtAddr,
}

impl StackBounds {
    pub fn start(&self) -> VirtAddr {
        self.start
    }

    pub fn end(&self) -> VirtAddr {
        self.end
    }
}

pub fn alloc_stack(
    size_in_pages: u64,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<StackBounds, mapper::MapToError> {
    use core::sync::atomic::{AtomicU64, Ordering};
    use x86_64::structures::paging::PageTableFlags as Flags;

    static STACK_ALLOC_NEXT: AtomicU64 = AtomicU64::new(0x_5555_5555_0000);

    let guard_page_start =
        STACK_ALLOC_NEXT.fetch_add((size_in_pages + 1) * Page::<Size4KiB>::SIZE, Ordering::SeqCst);
    let guard_page = Page::from_start_address(VirtAddr::new(guard_page_start))
        .expect("`STACK_ALLOC_NEXT` not page aligned");

    let stack_start = guard_page + 1;
    let stack_end = stack_start + size_in_pages;
    let flags = Flags::PRESENT | Flags::WRITABLE;
    for page in Page::range(stack_start, stack_end) {
        let frame = frame_allocator.allocate_frame().ok_or(mapper::MapToError::FrameAllocationFailed)?;
        mapper
            .map_to(page, frame, flags, frame_allocator)?.flush();
    }
    Ok(StackBounds {
        start: stack_start.start_address(),
        end: stack_end.start_address(),
    })
}

/// Creates an example mapping for the given page to frame `0xb8000`.
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    // FIXME: ONLY FOR TEMPORARY TESTING
    let unused_frame = unsafe { UnusedPhysFrame::new(frame) };
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = mapper.map_to(page, unused_frame, flags, frame_allocator);
    map_to_result.expect("map_to failed").flush();
}

/// A FrameAllocator that always returns `None`.
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<UnusedPhysFrame> {
        None
    }
}

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = UnusedPhysFrame> {
        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        let frames = frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)));
        // we know that the frames are really unused
        frames.map(|f| unsafe { UnusedPhysFrame::new(f) })
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<UnusedPhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
