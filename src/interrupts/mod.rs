// Copyright 2016 Philipp Oppermann. See the README.md
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use spin::Once;
use memory::MemoryController;
use x86::bits64::task::TaskStateSegment;

mod idt;
mod gdt;

macro_rules! save_scratch_registers {
    () => {
        asm!("push rax
              push rcx
              push rdx
              push rsi
              push rdi
              push r8
              push r9
              push r10
              push r11
        " :::: "intel", "volatile");
    }
}

macro_rules! restore_scratch_registers {
    () => {
        asm!("pop r11
              pop r10
              pop r9
              pop r8
              pop rdi
              pop rsi
              pop rdx
              pop rcx
              pop rax
            " :::: "intel", "volatile");
    }
}

macro_rules! handler {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                save_scratch_registers!();
                asm!("mov rdi, rsp
                      add rdi, 9*8 // calculate exception stack frame pointer
                      call $0"
                      :: "i"($name as extern "C" fn(
                          &ExceptionStackFrame))
                      : "rdi" : "intel");

                restore_scratch_registers!();
                asm!("iretq" :::: "intel", "volatile");
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
                save_scratch_registers!();
                asm!("mov rsi, [rsp + 9*8] // load error code into rsi
                      mov rdi, rsp
                      add rdi, 10*8 // calculate exception stack frame pointer
                      sub rsp, 8 // align the stack pointer
                      call $0
                      add rsp, 8 // undo stack pointer alignment
                      " :: "i"($name as extern "C" fn(
                          &ExceptionStackFrame, u64))
                      : "rdi","rsi" : "intel");
                restore_scratch_registers!();
                asm!("add rsp, 8 // pop error code
                      iretq" :::: "intel", "volatile");
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
        idt.set_handler(3, handler!(breakpoint_handler));
        idt.set_handler(6, handler!(invalid_opcode_handler));
        idt.set_handler(8, handler_with_error_code!(double_fault_handler))
            .set_stack_index(DOUBLE_FAULT_IST_INDEX as u8);
        idt.set_handler(14, handler_with_error_code!(page_fault_handler));

        idt
    };
}

static TSS: Once<TaskStateSegment> = Once::new();
static GDT: Once<gdt::Gdt> = Once::new();
const DOUBLE_FAULT_IST_INDEX: usize = 0;

pub fn init(memory_controller: &mut MemoryController) {
    use x86::shared::segmentation::{SegmentSelector, set_cs};
    use x86::shared::task::load_tr;

    let double_fault_stack = memory_controller.alloc_stack(1)
        .expect("could not allocate double fault stack");

    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.ist[DOUBLE_FAULT_IST_INDEX] = double_fault_stack.top() as u64;
        tss
    });

    let mut code_selector = SegmentSelector::empty();
    let mut tss_selector = SegmentSelector::empty();
    let gdt = GDT.call_once(|| {
        let mut gdt = gdt::Gdt::new();
        tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
        code_selector = gdt.add_entry(gdt::Descriptor::kernel_code_segment());
        gdt
    });
    gdt.load();

    unsafe {
        set_cs(code_selector);
        load_tr(tss_selector);
    }

    IDT.load();
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

extern "C" fn divide_by_zero_handler(stack_frame: &ExceptionStackFrame) {
    println!("\nEXCEPTION: DIVIDE BY ZERO\n{:#?}", stack_frame);
    loop {}
}

extern "C" fn breakpoint_handler(stack_frame: &ExceptionStackFrame) {
    println!("\nEXCEPTION: BREAKPOINT at {:#x}\n{:#?}",
             stack_frame.instruction_pointer,
             stack_frame);
}

extern "C" fn invalid_opcode_handler(stack_frame: &ExceptionStackFrame) {
    println!("\nEXCEPTION: INVALID OPCODE at {:#x}\n{:#?}",
             stack_frame.instruction_pointer,
             stack_frame);
    loop {}
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

extern "C" fn page_fault_handler(stack_frame: &ExceptionStackFrame, error_code: u64) {
    use x86::shared::control_regs;
    println!("\nEXCEPTION: PAGE FAULT while accessing {:#x}\nerror code: \
                                  {:?}\n{:#?}",
             unsafe { control_regs::cr2() },
             PageFaultErrorCode::from_bits(error_code).unwrap(),
             stack_frame);
    loop {}
}

extern "C" fn double_fault_handler(stack_frame: &ExceptionStackFrame, _error_code: u64) {
    println!("\nEXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
    loop {}
}
