use x86_64::structures::tss::TaskStateSegment;

static DOUBLE_FAULT_STACK: [u8; 4096] = [0; 4096];
const DOUBLE_FAULT_IST_INDEX: usize = 0;

pub fn init() {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] = {
        let stack_start = &DOUBLE_FAULT_STACK as *const [u8; _] as usize;
        let stack_size = DOUBLE_FAULT_STACK.len();
        let stack_end = stack_start + stack_size;
        stack_end
    };
}
