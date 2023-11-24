use core::cell::OnceCell;
use uefi::table::boot::{MemoryMap, MemoryType};
use x86_64::{VirtAddr, PhysAddr};
use x86_64::structures::paging::{Translate, PageTable, OffsetPageTable, mapper::TranslateResult};
use linked_list_allocator::LockedHeap;

const HEAP_SIZE: usize = 20000 * 4 * 1024;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub static mut MAPPER: OnceCell<MemoryMapper> = OnceCell::new();

pub fn init_allocator(memory_map: &MemoryMap) {

    log::info!("Initializing heap allocator");

    let desc = memory_map
        .entries()
        .filter(|desc| desc.ty == MemoryType::CONVENTIONAL)
        .filter(|desc| (desc.page_count * 4000) as usize >= HEAP_SIZE)
        .max_by_key(|desc| desc.page_count)
        .expect("Cannot find suitable memory region for heap");

    log::debug!(
        "Found suitable memory region for heap at {:#x} ({} pages)",
        desc.phys_start, desc.page_count
    );

    unsafe {
        ALLOCATOR.lock().init(desc.phys_start as usize, HEAP_SIZE);
    }
}

#[derive(Debug)]
pub struct MemoryMapper {
    page_table: OffsetPageTable<'static>,
    phys_offset: VirtAddr,
}

impl MemoryMapper {

    pub fn virt_to_phys(&self, virt: VirtAddr) -> PhysAddr {

        let (frame, offset) = match self.page_table.translate(virt) {
            TranslateResult::Mapped { frame, offset, .. } => (frame, offset),
            v => panic!("Cannot translate page: {:?}", v)
        };

        frame.start_address() + offset
    }

    // Note: technically there can be more than one VirtAddr mapped to
    // a given PhysAddr, but we only care about the one that has been 
    // offset-mapped by UEFI
    pub fn phys_to_virt(&self, phys: PhysAddr) -> VirtAddr {
        self.phys_offset + phys.as_u64()
    }

    pub fn ref_to_phys<T: ?Sized>(&self, p: &T) -> PhysAddr {
        let virt = VirtAddr::new(p as *const T as *const usize as u64);
        self.virt_to_phys(virt)
    }
}

pub fn init_mapper() {

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

    let page_table = unsafe { OffsetPageTable::new(l4_table, phys_offset) };

    let mapper = MemoryMapper {
        page_table,
        phys_offset,
    };

    unsafe {
        MAPPER.set(mapper).expect("Memory mapper already initialized?");
    }

}

pub fn get_mapper() -> &'static MemoryMapper {
    unsafe { MAPPER.get().expect("Memory mapper not initialized?") }
}
