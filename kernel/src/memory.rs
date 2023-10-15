use uefi::table::boot::{MemoryMap, MemoryType};
use linked_list_allocator::LockedHeap;

const HEAP_SIZE: usize = 10000 * 4 * 1024;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_allocator(memory_map: &MemoryMap) {

    let desc = memory_map
        .entries()
        .filter(|desc| desc.ty == MemoryType::CONVENTIONAL)
        .max_by_key(|desc| desc.page_count)
        .expect("Cannot find suitable memory region for heap");

    log::debug!(
        "Found suitable memory region for heap at {:#x} ({} pages)",
        desc.phys_start, desc.page_count
    );

    assert!(HEAP_SIZE < (desc.page_count * 4000) as usize);
    unsafe {
        ALLOCATOR.lock().init(desc.phys_start as usize, HEAP_SIZE);
    }
}
