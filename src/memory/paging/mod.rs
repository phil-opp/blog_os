mod entry;
mod table;
pub mod translate;
pub mod mapping;
pub mod lock;

pub fn test<A>(frame_allocator: &mut A)
    where A: super::FrameAllocator
{
    use self::entry::PRESENT;
    mapping::map(&Page::containing_address(0xdeadbeaa000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0xdeadbeab000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0xdeadbeac000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0xdeadbead000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0xcafebeaf000),
                 PRESENT,
                 frame_allocator);
    mapping::map(&Page::containing_address(0x0),
                 PRESENT,
                 frame_allocator);
}

pub const PAGE_SIZE: usize = 4096;
const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;

pub struct Page {
    number: usize,
}

impl Page {
    fn containing_address(address: VirtualAddress) -> Page {
        assert!(address < 0x0000_8000_0000_0000 || address >= 0xffff_8000_0000_0000,
            "invalid address: 0x{:x}", address);
        Page { number: address / PAGE_SIZE }
    }

    fn start_address(&self) -> VirtualAddress {
        self.number * PAGE_SIZE
    }

    fn p4_index(&self) -> usize {
        (self.number >> 27) & 0o777
    }
    fn p3_index(&self) -> usize {
        (self.number >> 18) & 0o777
    }
    fn p2_index(&self) -> usize {
        (self.number >> 9) & 0o777
    }
    fn p1_index(&self) -> usize {
        (self.number >> 0) & 0o777
    }
}
