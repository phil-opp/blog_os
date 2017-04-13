use memory::PAGE_SIZE;

const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;

pub struct Page {
   number: usize,
}
