use x86_64::structures::paging::mapper::MapToError;
use x86_64::{
    structures::paging::{PageTable, OffsetPageTable, PageTableFlags},
    structures::paging::{Page, PhysFrame, Mapper, Size4KiB, FrameAllocator},
    VirtAddr, PhysAddr
};
use bootloader::bootinfo::{BootInfo, MemoryMap, MemoryRegionType};
use linked_list_allocator::LockedHeap;


#[global_allocator]
pub static ALLOCATOR: LockedHeap = LockedHeap::empty();


const HEAP_START: usize = 0x_4444_4444_0000;
const HEAP_SIZE: usize = 40 * 100 * 1024;


pub fn init_mapper(boot_info: &'static BootInfo) -> OffsetPageTable {

    let phys_offset = VirtAddr::new(boot_info.physical_memory_offset);

    unsafe { 

        // Get active L4 table
        let l4_table = {
            use x86_64::registers::control::Cr3;
            let (l4_frame, _) = Cr3::read();

            let phys = l4_frame.start_address();
            let virt = phys_offset + phys.as_u64();
            let ptr: *mut PageTable = virt.as_mut_ptr();
        
            &mut *ptr
        };

        OffsetPageTable::new(l4_table, phys_offset)
    }
}

pub fn init_allocator(
    boot_info: &'static BootInfo,
    mapper: &mut impl Mapper<Size4KiB>
) {

    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    init_heap(mapper, &mut frame_allocator).unwrap();
}


pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>
) -> Result<(), MapToError<Size4KiB>> {

    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        };
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}


pub struct  BootInfoFrameAllocator {
    mem_map: &'static MemoryMap,
    next: usize
}

impl BootInfoFrameAllocator {

    pub unsafe fn init(mem_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            mem_map,
            next: 0
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
    
        let regions = self.mem_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);

        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());

        let frame_addresses = addr_ranges
            .flat_map(|r| r.step_by(4096))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)));

        frame_addresses
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}


#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("Allocation error: {:?}", layout)
}
