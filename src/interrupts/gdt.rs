use bit_field::BitField;
use collections::vec::Vec;
use x86::bits64::task::TaskStateSegment;

pub struct Gdt(Vec<u64>);

impl Gdt {
    pub fn new() -> Gdt {
        let zero_entry = 0;
        Gdt(vec![zero_entry])
    }

    pub fn add_entry(&mut self, entry: Entry) -> Selector {
        use core::mem::size_of;
        let index = self.0.len() * size_of::<u64>();

        match entry {
            Entry::UserSegment(entry) => self.0.push(entry),
            Entry::SystemSegment(entry_low, entry_high) => {
                self.0.push(entry_low);
                self.0.push(entry_high);
            }
        }

        Selector(index as u16)
    }

    pub fn load(&'static self) {
        use x86::shared::dtables::{DescriptorTablePointer, lgdt};
        use core::mem::size_of;

        let ptr = DescriptorTablePointer {
            base: self.0.as_ptr() as *const ::x86::shared::segmentation::SegmentDescriptor,
            limit: (self.0.len() * size_of::<u64>() - 1) as u16,
        };

        unsafe { lgdt(&ptr) };
    }
}

pub enum Entry {
    UserSegment(u64),
    SystemSegment(u64, u64),
}

impl Entry {
    pub fn code_segment() -> Entry {
        let flags = DESCRIPTOR_TYPE | PRESENT | READ_WRITE | EXECUTABLE | LONG_MODE;
        Entry::UserSegment(flags.bits())
    }

    pub fn data_segment() -> Entry {
        let flags = DESCRIPTOR_TYPE | PRESENT | READ_WRITE;
        Entry::UserSegment(flags.bits())
    }

    pub fn tss_segment(tss: &'static TaskStateSegment) -> Entry {
        use core::mem::size_of;

        let ptr = tss as *const _ as u64;

        let mut low = PRESENT.bits();
        low.set_range(0..16, (size_of::<TaskStateSegment>() - 1) as u64);
        low.set_range(16..40, ptr.get_range(0..24));
        low.set_range(40..44, 0b1001); // type: available 64-bit tss

        let mut high = 0;
        high.set_range(0..32, ptr.get_range(32..64));

        Entry::SystemSegment(low, high)
    }
}

bitflags! {
    flags EntryFlags: u64 {
        const READ_WRITE        = 1 << 41,
        const CONFORMING        = 1 << 42,
        const EXECUTABLE        = 1 << 43,
        const DESCRIPTOR_TYPE   = 1 << 44,
        const PRESENT           = 1 << 47,
        const LONG_MODE         = 1 << 53,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Selector(u16);

impl Selector {
    pub fn new() -> Selector {
        Selector(0)
    }
}

pub fn reload_segment_registers(code_selector: Selector, data_selector: Selector) {

    let current_code_selector: u16;
    let current_data_selector: u16;

    unsafe {
        asm!("mov $0, cs" : "=r" (current_code_selector) ::: "intel");
        asm!("mov $0, ds" : "=r" (current_data_selector) ::: "intel");
    }
    assert_eq!(code_selector.0, current_code_selector);
    assert_eq!(data_selector.0, current_data_selector);

    // jmp ax:.new_code_segment // TODO
    // .new_code_segment:
    // unsafe { asm!("
    // mov ax, $1
    // mov ss, ax
    // mov ds, ax
    // mov es, ax
    // ":: "r" (code_selector.0), "r" (data_selector.0) :: "intel")};
    //
}

/// Load the task state register.
pub unsafe fn load_ltr(selector: Selector) {
    asm!("ltr $0" :: "r" (selector));
}
