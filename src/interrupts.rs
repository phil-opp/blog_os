use x86_64::structures::idt::Idt;

pub fn init() {
    let mut idt = Idt::new();
}
