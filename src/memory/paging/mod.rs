use memory::Frame;

pub const PAGE_SIZE: usize = 4096;
const ENTRY_SIZE: usize = 8;
const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;

// pub fn translate(virtual_address: usize) -> Option<PhysicalAddress> {
// let page = Page::containing_address(virtual_address);
// let offset = virtual_address % PAGE_SIZE;
//
// let p4_entry = page.p4_table().entry(page.p4_index());
// assert!(!p4_entry.flags().contains(HUGE_PAGE));
// if !p4_entry.flags().contains(PRESENT) {
// return None;
// }
//
// let p3_entry = page.p3_table().entry(page.p3_index());
// if !p3_entry.flags().contains(PRESENT) {
// return None;
// }
// if p3_entry.flags().contains(HUGE_PAGE) {
// 1GiB page (address must be 1GiB aligned)
// let start_frame_number = p3_entry.pointed_frame().number;
// assert!(start_frame_number % (ENTRY_COUNT * ENTRY_COUNT) == 0);
// let frame_number = start_frame_number + page.p2_index() * ENTRY_COUNT + page.p1_index();
// return Some(frame_number * PAGE_SIZE + offset);
// }
//
// let p2_entry = page.p2_table().entry(page.p2_index());
// if !p2_entry.flags().contains(PRESENT) {
// return None;
// }
// if p2_entry.flags().contains(HUGE_PAGE) {
// 2MiB page (address must be 2MiB aligned)
// let start_frame_number = p2_entry.pointed_frame().number;
// assert!(start_frame_number % ENTRY_COUNT == 0);
// let frame_number = start_frame_number + page.p1_index();
// return Some(frame_number * PAGE_SIZE + offset);
// }
//
// let p1_entry = page.p1_table().entry(page.p1_index());
// assert!(!p1_entry.flags().contains(HUGE_PAGE));
// if !p1_entry.flags().contains(PRESENT) {
// return None;
// }
// Some(p1_entry.pointed_frame().number * PAGE_SIZE + offset)
// }

pub fn translate(virtual_address: usize) -> Option<PhysicalAddress> {
    let page = Page::containing_address(virtual_address);
    let offset = virtual_address % PAGE_SIZE;

    let frame_number = {
        let p4_entry = page.p4_table().entry(page.p4_index());
        assert!(!p4_entry.flags().contains(HUGE_PAGE));
        if !p4_entry.flags().contains(PRESENT) {
            return None;
        }

        let p3_entry = unsafe { page.p3_table() }.entry(page.p3_index());
        if !p3_entry.flags().contains(PRESENT) {
            return None;
        }
        if p3_entry.flags().contains(HUGE_PAGE) {
            // 1GiB page (address must be 1GiB aligned)
            let start_frame_number = p3_entry.pointed_frame().number;
            assert!(start_frame_number % (ENTRY_COUNT * ENTRY_COUNT) == 0);
            start_frame_number + page.p2_index() * ENTRY_COUNT + page.p1_index()
        } else {
            // 2MiB or 4KiB page
            let p2_entry = unsafe { page.p2_table() }.entry(page.p2_index());
            if !p2_entry.flags().contains(PRESENT) {
                return None;
            }
            if p2_entry.flags().contains(HUGE_PAGE) {
                // 2MiB page (address must be 2MiB aligned)
                let start_frame_number = p2_entry.pointed_frame().number;
                assert!(start_frame_number % ENTRY_COUNT == 0);
                start_frame_number + page.p1_index()
            } else {
                // standard 4KiB page
                let p1_entry = unsafe { page.p1_table() }.entry(page.p1_index());
                assert!(!p1_entry.flags().contains(HUGE_PAGE));
                if !p1_entry.flags().contains(PRESENT) {
                    return None;
                }
                p1_entry.pointed_frame().number
            }
        }
    };
    Some(frame_number * PAGE_SIZE + offset)
}

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

    const fn p4_table(&self) -> Table {
        Table(Page { number: 0o_777_777_777_777 })
    }

    fn p3_table(&self) -> Table {
        Table(Page { number: 0o_777_777_777_000 | self.p4_index() })
    }

    fn p2_table(&self) -> Table {
        Table(Page { number: 0o_777_777_000_000 | (self.p4_index() << 9) | self.p3_index() })
    }

    fn p1_table(&self) -> Table {
        Table(Page {
            number: 0o_777_000_000_000 | (self.p4_index() << 18) | (self.p3_index() << 9) |
                    self.p2_index(),
        })
    }
}

struct Table(Page);

impl Table {
    fn entry(&self, index: usize) -> TableEntry {
        assert!(index < ENTRY_COUNT);
        let entry_address = self.0.start_address() + index * ENTRY_SIZE;
        unsafe { *(entry_address as *const _) }
    }
}

#[derive(Debug, Clone, Copy)]
struct TableEntry(u64);

impl TableEntry {
    fn is_unused(&self) -> bool {
        self.0 == 0
    }

    fn set_unused(&mut self) {
        self.0 = 0
    }

    fn set(&mut self, frame: Frame, flags: TableEntryFlags) {
        self.0 = (((frame.number as u64) << 12) & 0x000fffff_fffff000) | flags.bits();
    }

    fn flags(&self) -> TableEntryFlags {
        TableEntryFlags::from_bits_truncate(self.0)
    }

    fn pointed_frame(&self) -> Frame {
        Frame { number: ((self.0 & 0x000fffff_fffff000) >> 12) as usize }
    }
}

bitflags! {
    flags TableEntryFlags: u64 {
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
