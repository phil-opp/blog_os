use x86_64::{
    structures::paging::{MappedPageTable, MapperAllSizes, PageTable, PhysFrame},
    VirtAddr,
};

/// Initialize a new MappedPageTable.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: u64) -> impl MapperAllSizes {
    let level_4_table = active_level_4_table(physical_memory_offset);
    let phys_to_virt = move |frame: PhysFrame| -> *mut PageTable {
        let phys = frame.start_address().as_u64();
        let virt = VirtAddr::new(phys + physical_memory_offset);
        virt.as_mut_ptr()
    };
    MappedPageTable::new(level_4_table, phys_to_virt)
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: u64) -> &'static mut PageTable {
    use x86_64::{registers::control::Cr3, VirtAddr};

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = VirtAddr::new(phys.as_u64() + physical_memory_offset);
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}
