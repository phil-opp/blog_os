use x86::irq::IdtEntry;

pub struct Idt([IdtEntry; 16]);

impl Idt {
    pub fn new() -> Idt {
        Idt([IdtEntry::missing(); 16])
    }

    pub fn set_handler(&mut self, entry: usize, handler: extern fn()->!) {
        let ptr = handler as usize;
        self.0[entry] = IdtEntry::interrupt_gate(0x8, ptr as *const _);
    }

    pub unsafe fn load(&self) {
        use x86::dtables::{DescriptorTablePointer, lidt};
        use core::mem::size_of;

        let ptr = DescriptorTablePointer{
            base: self as *const _ as u64,
            limit: (size_of::<Self>() - 1) as u16,
        };

        lidt(&ptr);
    }
}
