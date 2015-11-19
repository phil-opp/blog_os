/// The paging lock must be unique. It is required for all page table operations and thus
/// guarantees exclusive page table access.
pub struct Lock {
    _private: (),
}

impl !Send for Lock {}
impl !Sync for Lock {}
