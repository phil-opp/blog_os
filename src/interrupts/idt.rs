
pub struct Idt([Entry; 16]);

impl Idt {
    pub fn new() -> Idt {
        Idt([Entry::missing(); 16])
    }

    pub fn set_handler(&mut self, entry: usize, handler: extern "C" fn() -> !) {
        self.0[entry] = Entry::new(EntryType::InterruptGate, 0x8, handler);
    }

    pub fn set_interrupt_stack(&mut self, entry: usize, stack_index: u16) {
        self.0[entry].options.set_stack_index(stack_index);
    }

    pub unsafe fn load(&'static self) {
        use x86::dtables::{DescriptorTablePointer, lidt};
        use core::mem::size_of;

        let ptr = DescriptorTablePointer {
            base: self as *const _ as u64,
            limit: (size_of::<Self>() - 1) as u16,
        };

        lidt(&ptr);
    }
}

use bit_field::BitField;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Entry {
    target_low: u16,
    gdt_selector: u16,
    options: EntryOptions,
    target_middle: u16,
    target_high: u32,
    reserved: u32,
}

impl Entry {
    pub fn missing() -> Entry {
        Entry {
            gdt_selector: 0,
            target_low: 0,
            target_middle: 0,
            target_high: 0,
            options: MISSING,
            reserved: 0,
        }
    }

    pub fn new(ty: EntryType, gdt_selector: u16, handler: extern "C" fn() -> !) -> Entry {
        let target = handler as u64;

        Entry {
            gdt_selector: gdt_selector,
            target_low: target as u16,
            target_middle: (target >> 16) as u16,
            target_high: (target >> 32) as u32,
            options: EntryOptions::new(ty),
            reserved: 0,
        }
    }
}

const MISSING: EntryOptions = EntryOptions(BitField::new(0));

#[derive(Debug, Clone, Copy)]
pub struct EntryOptions(BitField<u16>);

impl EntryOptions {
    pub fn new(ty: EntryType) -> Self {
        let mut flags = BitField::new(0);
        match ty {
            EntryType::InterruptGate => flags.set_range(8..12, 0b1110),
            EntryType::TrapGate => flags.set_range(8..12, 0b1111),
        }
        // set present bit
        flags.set_bit(15);

        EntryOptions(flags)
    }

    pub fn set_privilege(&mut self, dpl: u16) {
        assert!(dpl < 4);
        self.0.set_range(13..15, dpl);
    }

    pub fn set_stack_index(&mut self, index: u16) {
        assert!(index < 8);
        self.0.set_range(0..3, index);
    }
}

#[allow(dead_code)]
pub enum EntryType {
    InterruptGate,
    TrapGate,
}
