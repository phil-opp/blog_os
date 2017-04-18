use x86_64::structures::idt::{Idt, ExceptionStackFrame};
use memory::MemoryController;

lazy_static! {
    static ref IDT: Idt = {
        let mut idt = Idt::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        idt
    };
}

pub fn init(memory_controller: &mut MemoryController) {
    let double_fault_stack = memory_controller.alloc_stack(1)
        .expect("could not allocate double fault stack");

    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: &mut ExceptionStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut ExceptionStackFrame, _error_code: u64)
{
    println!("\nEXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
    loop {}
}
