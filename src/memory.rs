use x86_64::structures::paging::PageTable;
use x86_64::PhysAddr;

/// Returns the physical address for the given virtual address, or `None` if the
/// virtual address is not mapped.
pub fn translate_addr(addr: usize, level_4_table_addr: usize) -> Option<PhysAddr> {
    // retrieve the page table indices of the address that we want to translate
    let level_4_index = (addr >> 39) & 0o777;
    let level_3_index = (addr >> 30) & 0o777;
    let level_2_index = (addr >> 21) & 0o777;
    let level_1_index = (addr >> 12) & 0o777;
    let page_offset = addr & 0o7777;

    // check that level 4 entry is mapped
    let level_4_table = unsafe { &*(level_4_table_addr as *const PageTable) };
    if level_4_table[level_4_index].addr().is_null() {
        return None;
    }
    let level_3_table_addr = (level_4_table_addr << 9) | (level_4_index << 12);

    // check that level 3 entry is mapped
    let level_3_table = unsafe { &*(level_3_table_addr as *const PageTable) };
    if level_3_table[level_3_index].addr().is_null() {
        return None;
    }
    let level_2_table_addr = (level_3_table_addr << 9) | (level_3_index << 12);

    // check that level 2 entry is mapped
    let level_2_table = unsafe { &*(level_2_table_addr as *const PageTable) };
    if level_2_table[level_2_index].addr().is_null() {
        return None;
    }
    let level_1_table_addr = (level_2_table_addr << 9) | (level_2_index << 12);

    // check that level 1 entry is mapped and retrieve physical address from it
    let level_1_table = unsafe { &*(level_1_table_addr as *const PageTable) };
    let phys_addr = level_1_table[level_1_index].addr();
    if phys_addr.is_null() {
        return None;
    }

    Some(phys_addr + page_offset)
}
