use x86::segmentation::{self, SegmentSelector};

pub struct Idt([Entry; 16]);

impl Idt {
    pub fn new() -> Idt {
        Idt([Entry::missing(); 16])
    }

    pub fn set_handler(&mut self, entry: u8, handler: extern "C" fn() -> !) -> &mut EntryOptions {
        self.0[entry as usize] = Entry::new(segmentation::cs(), handler);
        &mut self.0[entry as usize].options
    }

    pub fn load(&'static self) {
        use x86::dtables::{DescriptorTablePointer, lidt};
        use core::mem::size_of;

        let ptr = DescriptorTablePointer {
            base: self as *const _ as u64,
            limit: (size_of::<Self>() - 1) as u16,
        };

        unsafe { lidt(&ptr) };
    }
}

use bit_field::BitField;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Entry {
    pointer_low: u16,
    gdt_selector: SegmentSelector,
    options: EntryOptions,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}

impl Entry {
    fn missing() -> Self {
        Entry {
            gdt_selector: SegmentSelector::new(0),
            pointer_low: 0,
            pointer_middle: 0,
            pointer_high: 0,
            options: EntryOptions::minimal(),
            reserved: 0,
        }
    }

    fn new(gdt_selector: SegmentSelector, handler: extern "C" fn() -> !) -> Self {
        let pointer = handler as u64;
        Entry {
            gdt_selector: gdt_selector,
            pointer_low: pointer as u16,
            pointer_middle: (pointer >> 16) as u16,
            pointer_high: (pointer >> 32) as u32,
            options: EntryOptions::new(),
            reserved: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EntryOptions(BitField<u16>);

impl EntryOptions {
    fn minimal() -> Self {
        let mut options = BitField::new(0);
        options.set_range(9..12, 0b111); // required 'one' bits
        EntryOptions(options)
    }

    fn new() -> Self {
        let mut options = Self::minimal();
        options.set_present(true);
        options
    }

    pub fn set_present(&mut self, present: bool) -> &mut Self {
        self.0.set_bit(15, present);
        self
    }

    pub fn disable_interrupts(&mut self, disable: bool) -> &mut Self {
        self.0.set_bit(8, !disable);
        self
    }

    pub fn set_privilege_level(&mut self, dpl: u16) -> &mut Self {
        self.0.set_range(13..15, dpl);
        self
    }

    pub fn set_stack_index(&mut self, index: u16) -> &mut Self {
        self.0.set_range(0..3, index);
        self
    }
}
