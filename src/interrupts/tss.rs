use memory::StackPointer;

#[derive(Debug)]
#[repr(C, packed)]
pub struct TaskStateSegment {
    reserved_0: u32,
    pub privilege_stacks: PrivilegeStackTable,
    reserved_1: u64,
    pub interrupt_stacks: InterruptStackTable,
    reserved_2: u64,
    reserved_3: u16,
    iomap_base: u16,
}

impl TaskStateSegment {
    pub fn new() -> TaskStateSegment {
        TaskStateSegment {
            privilege_stacks: PrivilegeStackTable([None, None, None]),
            interrupt_stacks: InterruptStackTable::new(),
            iomap_base: 0,
            reserved_0: 0,
            reserved_1: 0,
            reserved_2: 0,
            reserved_3: 0,
        }
    }
}

#[derive(Debug)]
pub struct PrivilegeStackTable([Option<StackPointer>; 3]);

#[derive(Debug)]
pub struct InterruptStackTable([Option<StackPointer>; 7]);

impl InterruptStackTable {
    pub fn new() -> InterruptStackTable {
        InterruptStackTable([None, None, None, None, None, None, None])
    }

    pub fn insert_stack(&mut self, stack_pointer: StackPointer) -> Result<u8, StackPointer> {
        // TSS index starts at 1
        for (entry, i) in self.0.iter_mut().zip(1..) {
            if entry.is_none() {
                *entry = Some(stack_pointer);
                return Ok(i);
            }
        }
        Err(stack_pointer)
    }
}
