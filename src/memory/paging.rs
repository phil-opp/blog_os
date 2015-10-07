use super::{VirtualAddress, Frame, PAGE_SIZE, FrameStack};
use core::ops::Deref;

//pub type FrameAllocator = super::FrameStack;
pub type Page = super::Page;

bitflags! {
    flags PageTableFieldFlags: u64 {
        const PRESENT =         1 << 0,
        const WRITABLE =        1 << 1,
        const USER_ACCESSIBLE = 1 << 2,
        const WRITE_THROUGH =   1 << 3,
        const NO_CACHE =        1 << 4,
        const ACCESSED =        1 << 5,
        const DIRTY =           1 << 6,
        const OTHER1 =          1 << 9,
        const OTHER2 =          1 << 10,
        const NO_EXECUTE =      1 << 63,
    }
}

pub struct Controller<'a, A> where A: 'a {
    allocator: &'a mut A,
}

impl<'a, A> Controller<'a, A> where A: FrameStack {
    pub unsafe fn new(allocator: &mut A) -> Controller<A> {
        Controller {
            allocator: allocator,
        }
    }

    pub fn map_to(&mut self, page: Page, frame:Frame, writable: bool, executable: bool) {
        let mut flags = PRESENT;
        if writable {
            flags = flags | WRITABLE;
        }
        if !executable {
            flags = flags | NO_EXECUTE;
        }

        page.map_to(frame, flags, || {self.allocate_frame()})
    }

    pub fn identity_map(&mut self, page: Page, writable: bool, executable: bool) {
        self.map_to(page, Frame{number: page.number}, writable, executable)
    }

    pub fn unmap(&mut self, page: Page) -> Frame {
        page.unmap()
    }

    pub fn begin_new_table(&mut self) {
        let new_p4_frame = self.allocate_frame();
        let new_p4_page = &mut PageTablePage(Page{number: new_p4_frame.number});
        unsafe{new_p4_page.zero()};
        new_p4_page.field(511).set(new_p4_frame, PRESENT | WRITABLE);

        let old_p4_page = &mut PageTablePage(Page{number: 0o777_777_777_777});
        old_p4_page.field(511).set(new_p4_frame, PRESENT | WRITABLE);

        Self::flush_tlb();
    }

    pub fn activate_new_table(&mut self) {
        unsafe {
            let p4_address: u64 = {
                let field = *(0xfffffffffffffff8 as *const u64);
                field & !0xfff
            };

            asm!("mov cr3, $0" :: "r"(p4_address) :: "intel")
        }
    }

    fn allocate_frame(&mut self) -> Frame {
        let unmap_page = |page: Page| {
            page.unmap()
        };
        self.allocator.pop(unmap_page).expect("no more frames available")
    }

    fn flush_tlb() {
        unsafe{asm!("mov rax, cr3
            mov cr3, rax" ::: "{rax}" : "intel")}
    }
}

// first allocated address starts on second P4-Page
pub const FIRST_PAGE : Page = Page {
    number: 0o_001_000_000_000,
};

// a page containing a page table
struct PageTablePage(Page);

struct PageIter(Page);

struct PageTableField(*const u64);

impl Page {
    fn from_address(address: &VirtualAddress) -> Page {
        Page {
            number: address.0 as usize >> 12,
        }
    }

    pub fn start_address(&self) -> VirtualAddress {
        if self.number >= 0o400_000_000_000 {
            //sign extension
            VirtualAddress(((self.number << 12) | 0o177777_000_000_000_000_0000) as *const u8)
        } else {
            VirtualAddress((self.number << 12) as *const u8)
        }
    }

    fn p4_index(&self) -> usize {(self.number >> 27) & 0o777}
    fn p3_index(&self) -> usize {(self.number >> 18) & 0o777}
    fn p2_index(&self) -> usize {(self.number >> 9) & 0o777}
    fn p1_index(&self) -> usize {(self.number >> 0) & 0o777}

    fn p4_page(&self) -> PageTablePage {
        PageTablePage(Page {
            number: 0o_777_777_777_777,
        })
    }
    fn p3_page(&self) -> PageTablePage {
        PageTablePage(Page {
            number: 0o_777_777_777_000 | self.p4_index(),
        })
    }
    fn p2_page(&self) -> PageTablePage {
        PageTablePage(Page {
            number: 0o_777_777_000_000 | (self.p4_index() << 9) | self.p3_index(),
        })
    }
    fn p1_page(&self) -> PageTablePage {
        PageTablePage(Page {
            number: 0o_777_000_000_000 | (self.p4_index() << 18) | (self.p3_index() << 9)
                | self.p2_index(),
        })
    }

    fn map_to<F>(&self, frame: Frame, flags: PageTableFieldFlags, mut allocate_frame: F)
        where F: FnMut() -> Frame,
    {
        let p4_field = self.p4_page().field(self.p4_index());
        if p4_field.is_free() {
            p4_field.set(allocate_frame(), PRESENT | WRITABLE);
            unsafe{self.p3_page().zero()};
        }
        let p3_field = self.p3_page().field(self.p3_index());
        if p3_field.is_free() {
            p3_field.set(allocate_frame(), PRESENT | WRITABLE);
            unsafe{self.p2_page().zero()};
        }
        let p2_field = self.p2_page().field(self.p2_index());
        if p2_field.is_free() {
            p2_field.set(allocate_frame(), PRESENT | WRITABLE);
            unsafe{self.p1_page().zero()};
        }
        let p1_field = self.p1_page().field(self.p1_index());
        //TODOassert!(p1_field.is_free());
        p1_field.set(frame, flags);
    }

    fn unmap(self) -> Frame {
        let p1_field = self.p1_page().field(self.p1_index());
        let frame = p1_field.pointed_frame();
        p1_field.set_free();
        // TODO free p(1,2,3) table if empty
        frame
    }

    unsafe fn zero(&self) {
        let page = self.start_address().0 as *mut [u64; (PAGE_SIZE/64) as usize];
        *page = [0; (PAGE_SIZE/64) as usize];
    }

    pub fn next_pages(self) -> PageIter {
        PageIter(self)
    }
}

impl Iterator for PageIter {
    type Item = Page;

    fn next(&mut self) -> Option<Page> {
        self.0.number += 1;
        Some(self.0)
    }
}

impl PageTablePage {
    fn field(&self, index: usize) -> PageTableField {
        //print!("index: {} pointer: {:o}\n", index, self.0.start_address().0 as usize + (index * 8));
        PageTableField((self.0.start_address().0 as usize + (index * 8)) as *const u64)
    }
}

impl Deref for PageTablePage {
    type Target = Page;
    fn deref(&self) -> &Page { &self.0 }
}

impl VirtualAddress {
    pub fn page(&self) -> Page {
        Page::from_address(self)
    }

    fn page_offset(&self) -> u32 {
        self.0 as u32 & 0xfff
    }
}

impl PageTableField {
    fn is(&self, flags: PageTableFieldFlags) -> bool {
        PageTableFieldFlags::from_bits_truncate(unsafe{*(self.0)}).contains(flags)
    }

    fn add_flag(&self, flags: PageTableFieldFlags) {
        unsafe{*(self.0 as *mut u64) |= flags.bits};
    }

    fn remove_flag(&self, flags: PageTableFieldFlags) {
        unsafe{*(self.0 as *mut u64) &= !flags.bits};
    }

    fn is_free(&self) -> bool {
        //TODO
        let f = unsafe{*self.0};
        let free = f == 0;
        free
    }

    fn set_free(&self) {
        //TODO
        unsafe{ *(self.0 as *mut _)= 0 };
    }

    fn pointed_frame(&self) -> Frame {
        Frame {
            number: ((unsafe{(*self.0)} & 0x000fffff_fffff000) >> 12) as usize,
        }
    }

    fn set(&self, frame: Frame, flags: PageTableFieldFlags) {
        let new = (((frame.number as u64) << 12) & 0x000fffff_fffff000) | flags.bits();
        unsafe{*(self.0 as *mut u64) = new};
    }
}
