use memory::Frame;
use memory::paging::PhysicalAddress;

pub struct Entry(u64);

impl Entry {
    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    pub fn set_unused(&mut self) {
        self.0 = 0;
    }

    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn pointed_frame(&self) -> Option<Frame> {
        if self.flags().contains(PRESENT) {
            Some(Frame::containing_address(self.0 as usize & 0x000fffff_fffff000))
        } else {
            None
        }
    }

    pub fn set(&mut self, frame: Frame, flags: EntryFlags) {
        assert!(frame.start_address() & !0x000fffff_fffff000 == 0);
        self.0 = (frame.start_address() as u64) | flags.bits();
    }
}

bitflags! {
    flags EntryFlags: u64 {
        const PRESENT =         1 << 0,
        const WRITABLE =        1 << 1,
        const USER_ACCESSIBLE = 1 << 2,
        const WRITE_THROUGH =   1 << 3,
        const NO_CACHE =        1 << 4,
        const ACCESSED =        1 << 5,
        const DIRTY =           1 << 6,
        const HUGE_PAGE =       1 << 7,
        const GLOBAL =          1 << 8,
        const NO_EXECUTE =      1 << 63,
    }
}

impl Frame {
    fn start_address(&self) -> PhysicalAddress {
        self.number << 12
    }
}
