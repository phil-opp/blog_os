mod idt;

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();

        idt.set_handler(0, divide_by_zero_handler);
        //idt.set_handler(8, double_fault_handler);
        //idt.set_handler(13, general_protection_fault_handler);
        idt.set_handler(14, page_fault_handler);

        idt
    };
}

pub fn init() {
    IDT.load();
}

use vga_buffer::print_error;

#[naked]
extern "C" fn divide_by_zero_handler() -> ! {
    unsafe {
        asm!(/* load excepiton fram pointer and call main_handler*/);
    }
    ::core::intrinsics::unreachable();

    extern "C" fn main_handler(stack_frame: *const ExceptionStackFrameErrorCode) -> ! {
        unsafe {
            print_error(format_args!("EXCEPTION: DIVIDE BY ZERO\n{:#?}", *stack_frame));
        }
        loop {}
    }
}

extern "C" fn divide_by_zero_handler() -> ! {
    let stack_frame: *const ExceptionStackFrame;
    unsafe {
        asm!("mov $0, rsp" : "=r"(stack_frame) ::: "intel");
        print_error(format_args!("EXCEPTION: DIVIDE BY ZERO\n{:#?}", *stack_frame));
    };
    loop {}
}

extern "C" fn double_fault_handler() -> ! {
    unsafe { print_error(format_args!("EXCEPTION: DOUBLE FAULT")) };
    loop {}
}

extern "C" fn general_protection_fault_handler() -> ! {
    loop {}
    unsafe { print_error(format_args!("EXCEPTION: GENERAL PROTECTION FAULT")) };
}


#[derive(Debug)]
#[repr(C)]
struct ExceptionStackFrame {
    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

#[derive(Debug)]
#[repr(C)]
struct ExceptionStackFrameErrorCode {
    error_code: u64,
    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

#[naked]
extern "C" fn page_fault_handler() -> ! {
    unsafe {
        asm!("
            mov rdi, rsp
            call $0
        " :: "i"(handler as extern "C" fn(*const ExceptionStackFrameErrorCode) -> !) :: "intel");
    }
    loop{}

    extern "C" fn handler(stack_frame: *const ExceptionStackFrameErrorCode) -> ! {
        unsafe {
            print_error(format_args!("EXCEPTION: PAGE FAULT\n  stack frame: {:#?}", *stack_frame));
        }
        loop {}
    }
}
