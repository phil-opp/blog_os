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

extern "C" fn page_fault_handler() -> ! {
    println!("EXCEPTION: PAGE FAULT");
    loop {}
}
