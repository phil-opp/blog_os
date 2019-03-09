use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageTable, PhysFrame, RecursivePageTable, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

/// Creates a RecursivePageTable instance from the level 4 address.
///
/// This function is unsafe because it can break memory safety if an invalid
/// address is passed.
pub unsafe fn init(level_4_table_addr: usize) -> RecursivePageTable<'static> {
    /// Rust currently treats the whole body of unsafe functions as an unsafe
    /// block, which makes it difficult to see which operations are unsafe. To
    /// limit the scope of unsafe we use a safe inner function.
    fn init_inner(level_4_table_addr: usize) -> RecursivePageTable<'static> {
        let level_4_table_ptr = level_4_table_addr as *mut PageTable;
        let level_4_table = unsafe { &mut *level_4_table_ptr };
        RecursivePageTable::new(level_4_table).unwrap()
    }

    init_inner(level_4_table_addr)
}

/// Create a FrameAllocator from the passed memory map
pub fn init_frame_allocator(
    memory_map: &'static MemoryMap,
) -> BootInfoFrameAllocator<impl Iterator<Item = PhysFrame>> {
    // get usable regions from memory map
    let regions = memory_map
        .iter()
        .filter(|r| r.region_type == MemoryRegionType::Usable);
    // map each region to its address range
    let addr_ranges = regions.map(|r| r.range.start_addr()..r.range.end_addr());
    // transform to an iterator of frame start addresses
    let frame_addresses = addr_ranges.flat_map(|r| r.into_iter().step_by(4096));
    // create `PhysFrame` types from the start addresses
    let frames = frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)));

    BootInfoFrameAllocator { frames }
}

pub fn create_example_mapping(
    recursive_page_table: &mut RecursivePageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let page: Page = Page::containing_address(VirtAddr::new(0xdeadbeaf000));
    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe { recursive_page_table.map_to(page, frame, flags, frame_allocator) };
    map_to_result.expect("map_to failed").flush();
}

/// A FrameAllocator that always returns `None`.
pub struct EmptyFrameAllocator;

impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

pub struct BootInfoFrameAllocator<I>
where
    I: Iterator<Item = PhysFrame>,
{
    frames: I,
}

impl<I> FrameAllocator<Size4KiB> for BootInfoFrameAllocator<I>
where
    I: Iterator<Item = PhysFrame>,
{
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.frames.next()
    }
}
