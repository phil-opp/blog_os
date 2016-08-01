mod idt;

macro_rules! handler {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                asm!("mov rdi, rsp
                      sub rsp, 8 // align the stack pointer
                      call $0"
                      :: "i"($name as extern "C" fn(*const ExceptionStackFrame) -> !)
                      : "rdi" : "intel");
                ::core::intrinsics::unreachable();
            }
        }
        wrapper
    }}
}

macro_rules! handler_with_error_code {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                asm!("pop rsi // pop error code into rsi
                      mov rdi, rsp
                      sub rsp, 8 // align the stack pointer
                      call $0"
                      :: "i"($name as extern "C" fn(*const ExceptionStackFrame, u64) -> !)
                      : "rdi" : "intel");
                ::core::intrinsics::unreachable();
            }
        }
        wrapper
    }}
}

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();

        idt.set_handler(0, handler!(divide_by_zero_handler));
        idt.set_handler(6, handler!(invalid_opcode_handler));
        idt.set_handler(8, double_fault_handler);
        idt.set_handler(14, handler_with_error_code!(page_fault_handler));

        idt
    };
}

pub fn init() {
    IDT.load();
}

use vga_buffer::print_error;

extern "C" fn divide_by_zero_handler(stack_frame: *const ExceptionStackFrame) -> ! {
    unsafe {
        print_error(format_args!("EXCEPTION: DIVIDE BY ZERO\n{:#?}", *stack_frame));
    }
    loop {}
}

extern "C" fn invalid_opcode_handler(stack_frame: *const ExceptionStackFrame)
    -> !
{
    unsafe {
        print_error(format_args!("EXCEPTION: INVALID OPCODE at {:#x}\n{:#?}",
            (*stack_frame).instruction_pointer, *stack_frame));
    }
    loop {}
}

extern "C" fn page_fault_handler(stack_frame: *const ExceptionStackFrame,
                                 error_code: u64) -> !
{
    unsafe {
        print_error(format_args!(
            "EXCEPTION: PAGE FAULT with error code {:?}\n{:#?}",
            PageFaultErrorCode::from_bits(error_code), *stack_frame));
    }
    loop {}
}

extern "C" fn double_fault_handler() -> ! {
    unsafe { print_error(format_args!("EXCEPTION: DOUBLE FAULT")) };
    loop {}
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

bitflags! {
    flags PageFaultErrorCode: u64 {
        const PROTECTION_VIOLATION = 1 << 0,
        const CAUSED_BY_WRITE = 1 << 1,
        const USER_MODE = 1 << 2,
        const MALFORMED_TABLE = 1 << 3,
        const INSTRUCTION_FETCH = 1 << 4,
    }
}
