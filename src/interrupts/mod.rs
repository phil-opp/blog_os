mod idt;

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();

        idt.set_handler(0, divide_by_zero_handler);
        idt.set_handler(8, double_fault_handler);
        idt.set_handler(13, general_protection_fault_handler);
        idt.set_handler(14, page_fault_handler);

        idt
    };
}

pub fn init() {
    assert_has_not_been_called!();

    unsafe { IDT.load() }
}


pub extern fn divide_by_zero_handler() -> ! {
    println!("EXCEPTION: DIVIDE BY ZERO");
    loop {}
}

pub extern fn double_fault_handler() -> ! {
    println!("EXCEPTION: DOUBLE FAULT");
    loop {}
}

pub extern fn general_protection_fault_handler() -> ! {
    println!("EXCEPTION: GENERAL PROTECTION FAULT");
    loop {}
}

pub extern fn page_fault_handler() -> ! {
    println!("EXCEPTION: PAGE FAULT");
    loop {}
}
