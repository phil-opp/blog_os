use x86_64::VirtualAddress;
use x86_64::structures::idt::{Idt, ExceptionStackFrame};
use x86_64::structures::tss::TaskStateSegment;
use memory::MemoryController;
use spin::Once;

mod gdt;

lazy_static! {
    static ref IDT: Idt = {
        let mut idt = Idt::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        idt
    };
}

static TSS: Once<TaskStateSegment> = Once::new();
static GDT: Once<gdt::Gdt> = Once::new();

const DOUBLE_FAULT_IST_INDEX: usize = 0;

pub fn init(memory_controller: &mut MemoryController) {
    let double_fault_stack = memory_controller.alloc_stack(1)
        .expect("could not allocate double fault stack");

    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] = VirtualAddress(
            double_fault_stack.top());
        tss
    });

    let gdt = GDT.call_once(|| {
        let mut gdt = gdt::Gdt::new();
        let code_selector = gdt.add_entry(gdt::Descriptor::
                            kernel_code_segment());
        let tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
        gdt
    });
    gdt.load();

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
