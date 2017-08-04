// Copyright 2016 Philipp Oppermann. See the README.md
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use memory::Frame;
use multiboot2::ElfSection;

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
            Some(Frame::containing_address(
                self.0 as usize & 0x000fffff_fffff000,
            ))
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
    pub struct EntryFlags: u64 {
        const PRESENT =         1 << 0;
        const WRITABLE =        1 << 1;
        const USER_ACCESSIBLE = 1 << 2;
        const WRITE_THROUGH =   1 << 3;
        const NO_CACHE =        1 << 4;
        const ACCESSED =        1 << 5;
        const DIRTY =           1 << 6;
        const HUGE_PAGE =       1 << 7;
        const GLOBAL =          1 << 8;
        const NO_EXECUTE =      1 << 63;
    }
}

impl EntryFlags {
    pub fn from_elf_section_flags(section: &ElfSection) -> EntryFlags {
        use multiboot2::{ELF_SECTION_ALLOCATED, ELF_SECTION_WRITABLE, ELF_SECTION_EXECUTABLE};

        let mut flags = EntryFlags::empty();

        if section.flags().contains(ELF_SECTION_ALLOCATED) {
            // section is loaded to memory
            flags = flags | PRESENT;
        }
        if section.flags().contains(ELF_SECTION_WRITABLE) {
            flags = flags | WRITABLE;
        }
        if !section.flags().contains(ELF_SECTION_EXECUTABLE) {
            flags = flags | NO_EXECUTE;
        }

        flags
    }
}
