mod idt;

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();

        idt.set_handler(14, page_fault_handler);

        idt
    };
}

pub fn init() {
    IDT.load();
}

use vga_buffer::print_error;

extern "C" fn page_fault_handler() -> ! {
    unsafe { print_error(format_args!("EXCEPTION: PAGE FAULT")) };
    loop {}
}
