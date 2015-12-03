mod entry;
mod table;
pub mod translate;
pub mod mapping;

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
        match address {
            addr if addr < 0o_400_000_000_000_0000 => Page { number: addr / PAGE_SIZE },
            addr if addr >= 0o177777_400_000_000_000_0000 => {
                Page { number: (address / PAGE_SIZE) & 0o_777_777_777_777 }
            }
            _ => panic!("invalid address: 0x{:x}", address),
        }
    }

    pub fn start_address(&self) -> VirtualAddress {
        if self.number >= 0x800000000 {
            // sign extension necessary
            (self.number << 12) | 0xffff_000000000000
        } else {
            self.number << 12
        }
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
