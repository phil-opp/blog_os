use bit_field::BitField;
use x86::bits64::task::TaskStateSegment;
use x86::shared::segmentation::SegmentSelector;
use x86::shared::PrivilegeLevel;

pub struct Gdt {
    table: [u64; 8],
    current_offset: usize,
}

impl Gdt {
    pub fn new() -> Gdt {
        Gdt {
            table: [0; 8],
            current_offset: 1,
        }
    }

    fn push(&mut self, value: u64) -> usize {
        if self.current_offset < self.table.len() {
            let offset = self.current_offset;
            self.table[offset] = value;
            self.current_offset += 1;
            offset
        } else {
            panic!("GDT full");
        }
    }

    pub fn add_entry(&mut self, entry: Descriptor) -> SegmentSelector {
        let index = match entry {
            Descriptor::UserSegment(value) => self.push(value),
            Descriptor::SystemSegment(value_low, value_high) => {
                let index = self.push(value_low);
                self.push(value_high);
                index
            }
        };
        SegmentSelector::new(index as u16, PrivilegeLevel::Ring0)
    }

    pub fn load(&'static self) {
        use x86::shared::dtables::{DescriptorTablePointer, lgdt};
        use core::mem::size_of;

        let ptr = DescriptorTablePointer {
            base: self.table.as_ptr() as *const ::x86::shared::segmentation::SegmentDescriptor,
            limit: (self.table.len() * size_of::<u64>() - 1) as u16,
        };

        unsafe { lgdt(&ptr) };
    }
}

pub enum Descriptor {
    UserSegment(u64),
    SystemSegment(u64, u64),
}

impl Descriptor {
    pub fn kernel_code_segment() -> Descriptor {
        let flags = USER_SEGMENT | PRESENT | EXECUTABLE | LONG_MODE;
        Descriptor::UserSegment(flags.bits())
    }

    pub fn tss_segment(tss: &'static TaskStateSegment) -> Descriptor {
        use core::mem::size_of;

        let ptr = tss as *const _ as u64;

        let mut low = PRESENT.bits();
        low.set_range(0..16, (size_of::<TaskStateSegment>() - 1) as u64);
        low.set_range(16..40, ptr.get_range(0..24));
        low.set_range(40..44, 0b1001); // type: available 64-bit tss

        let mut high = 0;
        high.set_range(0..32, ptr.get_range(32..64));

        Descriptor::SystemSegment(low, high)
    }
}

bitflags! {
    flags DescriptorFlags: u64 {
        const CONFORMING        = 1 << 42,
        const EXECUTABLE        = 1 << 43,
        const USER_SEGMENT      = 1 << 44,
        const PRESENT           = 1 << 47,
        const LONG_MODE         = 1 << 53,
    }
}
