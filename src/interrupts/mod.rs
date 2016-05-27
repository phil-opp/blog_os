use x86::task::{load_ltr, TaskStateSegment};
use vga_buffer::print_error;

mod idt;
mod gdt;

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();

        idt.set_handler(0, divide_by_zero_handler);
        idt.set_handler(8, double_fault_handler).set_stack_index(1);
        idt.set_handler(13, general_protection_fault_handler);
        idt.set_handler(14, page_fault_handler);

        idt
    };

    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();

        let stack_size = 1024 * 3; // 3KiB
        let stack_bottom = unsafe {
            ::alloc::heap::allocate(stack_size, 16) as usize // TODO
        };
        let stack_top = stack_bottom + stack_size;

        tss.ist[0] = stack_top as u64;

        tss
    };

    static ref GDT: Gdt = {
        let mut table = gdt::Gdt::new();

        let selectors = GdtSelectors {
            code: table.add_entry(gdt::Entry::code_segment()),
            data: table.add_entry(gdt::Entry::data_segment()),
            tss: table.add_entry(gdt::Entry::tss_segment(&TSS)),
        };

        Gdt {
            table: table,
            selectors: selectors,
        }
    };
}

struct Gdt {
    table: gdt::Gdt,
    selectors: GdtSelectors,
}

struct GdtSelectors {
    code: gdt::Selector,
    data: gdt::Selector,
    tss: gdt::Selector,
}

pub fn init() {
    assert_has_not_been_called!();

    unsafe {
        GDT.table.load();
        gdt::reload_segment_registers(GDT.selectors.code, GDT.selectors.data);
        gdt::load_ltr(GDT.selectors.tss);
        IDT.load();
    }
}

pub extern "C" fn divide_by_zero_handler() -> ! {
    unsafe { print_error(format_args!("EXCEPTION: DIVIDE BY ZERO")) };
    loop {}
}

pub extern "C" fn double_fault_handler() -> ! {
    unsafe { print_error(format_args!("EXCEPTION: DOUBLE FAULT")) };
    loop {}
}

pub extern "C" fn general_protection_fault_handler() -> ! {
    unsafe { print_error(format_args!("EXCEPTION: GENERAL PROTECTION FAULT")) };
    loop {}
}

pub extern "C" fn page_fault_handler() -> ! {
    unsafe { print_error(format_args!("EXCEPTION: PAGE FAULT")) };
    loop {}
}
