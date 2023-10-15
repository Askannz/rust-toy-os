use uefi::table::boot::{MemoryMap, MemoryType};
use x86_64::VirtAddr;
use x86_64::structures::paging::{PageTable, OffsetPageTable, Translate, mapper::TranslateResult};
use linked_list_allocator::LockedHeap;

const HEAP_SIZE: usize = 10000 * 4 * 1024;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_allocator(memory_map: &MemoryMap) {

    log::info!("Initializing heap allocator");

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

pub fn get_mapper() -> OffsetPageTable<'static> {

    // UEFI has already set up paging with identity-mapping
    let phys_offset = VirtAddr::new(0x0);

    // Get active L4 table
    let l4_table = unsafe {
        use x86_64::registers::control::Cr3;
        let (l4_frame, _) = Cr3::read();

        let phys = l4_frame.start_address();
        let virt = phys_offset + phys.as_u64();
        let ptr: *mut PageTable = virt.as_mut_ptr();
    
        &mut *ptr
    };

    unsafe { OffsetPageTable::new(l4_table, phys_offset) }
}
