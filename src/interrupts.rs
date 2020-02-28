use crate::{gdt, hlt_loop, println};
use conquer_once::spin::OnceCell;
use core::{
    sync::atomic::{AtomicU64, Ordering},
    task::Waker,
};
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use lazy_static::lazy_static;
use pic8259_simple::ChainedPics;
use spin;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

pub fn init_queues() {
    INTERRUPT_WAKEUPS
        .try_init_once(|| ArrayQueue::new(10))
        .expect("failed to init interrupt wakeup queue");
    SCANCODE_QUEUE
        .try_init_once(|| ArrayQueue::new(10))
        .expect("failed to init scancode queue");
}

static INTERRUPT_WAKEUPS: OnceCell<ArrayQueue<Waker>> = OnceCell::uninit();

pub(crate) fn interrupt_wakeups() -> &'static ArrayQueue<Waker> {
    INTERRUPT_WAKEUPS
        .try_get()
        .expect("interrupt wakeup queue not initialized")
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

pub(crate) static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);
pub(crate) static TIMER_INTERRUPT_WAKER: AtomicWaker = AtomicWaker::new();

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: &mut InterruptStackFrame) {
    TIMER_TICKS.fetch_add(1, Ordering::Release);
    if let Some(waker) = TIMER_INTERRUPT_WAKER.take() {
        if let Err(_) = interrupt_wakeups().push(waker) {
            println!("WARNING: dropping interrupt wakeup");
        }
    }
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

pub(crate) static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
pub(crate) static KEYBOARD_INTERRUPT_WAKER: AtomicWaker = AtomicWaker::new();

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: &mut InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    let scancode_queue = SCANCODE_QUEUE
        .try_get()
        .expect("scancode queue not initialized");
    if let Err(_) = scancode_queue.push(scancode) {
        println!("WARNING: dropping keyboard input");
    }
    if let Some(waker) = KEYBOARD_INTERRUPT_WAKER.take() {
        if let Err(_) = interrupt_wakeups().push(waker) {
            println!("WARNING: dropping interrupt wakeup");
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

#[cfg(test)]
use crate::{serial_print, serial_println};

#[test_case]
fn test_breakpoint_exception() {
    serial_print!("test_breakpoint_exception...");
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
    serial_println!("[ok]");
}
