use memory::frame_allocator::FrameAllocator;
use memory::tlb;
use super::{PAGE_SIZE, Lock};
use memory::frame_allocator::Frame;
use core::intrinsics::offset;
use core::mem::size_of;

const P4: Table = Table( Page{ number: 0o_777_777_777_777} );

pub unsafe fn begin_new_on_identity_mapped_frame(_lock: &mut Lock, new_p4_frame: Frame) {
    let new_p4 = &mut Table(Page{ number: new_p4_frame.number });
    new_p4.zero();
    new_p4.field(511).set(new_p4_frame, PRESENT | WRITABLE);

    P4.field(511).set(new_p4_frame, PRESENT | WRITABLE);

    tlb::flush();
}

pub fn activate_current() {
    unsafe {
        let p4_address: u64 = {
            let field = *(0xfffffffffffffff8 as *const u64);
            field & !0xfff
        };

        asm!("mov cr3, $0" :: "r"(p4_address) :: "intel")
    }
}

pub fn map_to<A>(lock: &mut Lock, page: Page, frame: Frame, writable: bool,
    executable: bool, allocator: &mut A) where A: FrameAllocator
{
    let mut flags = PRESENT;
    if writable {
        flags = flags | WRITABLE;
    }
    if !executable {
        flags = flags | NO_EXECUTE;
    }

    let p4_field = page.p4_page().field(page.p4_index());
    if p4_field.is_unused() {
        p4_field.set(allocator.allocate_frame(lock).expect("no more frames"), PRESENT | WRITABLE);
        unsafe{page.p3_page().zero()};
    }
    let p3_field = page.p3_page().field(page.p3_index());
    if p3_field.is_unused() {
        p3_field.set(allocator.allocate_frame(lock).expect("no more frames"), PRESENT | WRITABLE);
        unsafe{page.p2_page().zero()};
    }
    let p2_field = page.p2_page().field(page.p2_index());
    if p2_field.is_unused() {
        p2_field.set(allocator.allocate_frame(lock).expect("no more frames"), PRESENT | WRITABLE);
        unsafe{page.p1_page().zero()};
    }
    let p1_field = page.p1_page().field(page.p1_index());
    assert!(p1_field.is_unused());
    p1_field.set(frame, flags);
}

pub fn unmap<A>(lock: &mut Lock, page: Page, allocator: &mut A) where A: FrameAllocator {
    assert!(!page.is_unused());
    let p1_field = page.p1_page().field(page.p1_index());
    let frame = p1_field.pointed_frame();
    p1_field.set_unused();
    // TODO free p(1,2,3) table if empty
    allocator.deallocate_frame(lock, frame);
}


/// A mapped or unmapped page
pub struct Page {
    pub number: usize, // TOOD make private
}

impl Page {
    pub fn containing_address(address: usize) -> Page {
        Page {
            number: (address >> 12) & 0o_777_777_777_777,
        }
    }

    pub fn pointer(&self) -> *const () {
        if self.number >= 0o400_000_000_000 {
            //sign extension
            ((self.number << 12) | 0o177777_000_000_000_000_0000) as *const ()
        } else {
            (self.number << 12) as *const ()
        }
    }

    pub fn is_unused(&self) -> bool {
        self.p4_page().field(self.p4_index()).is_unused() ||
        self.p3_page().field(self.p3_index()).is_unused() ||
        self.p2_page().field(self.p2_index()).is_unused() ||
        self.p1_page().field(self.p1_index()).is_unused()
    }

    fn p4_index(&self) -> usize {(self.number >> 27) & 0o777}
    fn p3_index(&self) -> usize {(self.number >> 18) & 0o777}
    fn p2_index(&self) -> usize {(self.number >> 9) & 0o777}
    fn p1_index(&self) -> usize {(self.number >> 0) & 0o777}

    fn p4_page(&self) -> Table {
        P4
    }
    fn p3_page(&self) -> Table {
        Table(Page {
            number: 0o_777_777_777_000 | self.p4_index(),
        })
    }
    fn p2_page(&self) -> Table {
        Table(Page {
            number: 0o_777_777_000_000 | (self.p4_index() << 9) | self.p3_index(),
        })
    }
    fn p1_page(&self) -> Table {
        Table(Page {
            number: 0o_777_000_000_000 | (self.p4_index() << 18) | (self.p3_index() << 9)
                | self.p2_index(),
        })
    }
}

/// A page table on a _mapped_ page.
struct Table(Page);

impl Table {
    unsafe fn zero(&mut self) {
        const ENTRIES: usize = PAGE_SIZE / 8;
        let page = self.0.pointer() as *mut () as *mut [u64; ENTRIES];
        *page = [0; ENTRIES];
    }

    fn field(&self, index: usize) -> &'static mut TableField {
        assert!(index < PAGE_SIZE / size_of::<u64>());
        unsafe {
            let field = offset(self.0.pointer() as *const u64, index as isize);
            &mut *(field as *const _ as *mut _)
        }
    }
}

struct TableField(u64);

impl TableField {
    fn is_unused(&self) -> bool {
        self.0 == 0
    }

    fn set_unused(&mut self) {
        self.0 = 0
    }

    fn set(&mut self, frame: Frame, flags: TableFieldFlags) {
        self.0 = (((frame.number as u64) << 12) & 0x000fffff_fffff000) | flags.bits();
    }

    fn pointed_frame(&self) -> Frame {
        Frame {
            number: ((self.0 & 0x000fffff_fffff000) >> 12) as usize,
        }
    }

}

bitflags! {
    flags TableFieldFlags: u64 {
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
