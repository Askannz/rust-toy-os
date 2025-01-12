use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

use x86_64::VirtAddr;

const NB_BLOCK_SIZES: usize = 28;

pub struct SimpleAllocator {
    pub heap: UnsafeCell<Option<SimpleHeap>>,
}

pub struct SimpleHeap {

    start: *mut u8,
    ptr: *mut u8,

    // This represents a matrix
    // <nb of possible block sizes> x <nb of possible block alignments>
    // Alignment cannot be larger than size, so half the matrix is unused.
    // But this keeps the code simple and doesn't waste that much memory.
    trackers: [Option<*mut u8>; NB_BLOCK_SIZES * NB_BLOCK_SIZES],

    stats: AllocStats,
} 

#[derive(Debug, Clone,)]
pub struct AllocStats {
    pub total: usize,
    pub explored: usize,
    pub allocated: usize,
    pub lost: usize,
    pub reclaimable: usize
}

impl SimpleAllocator {

    pub const fn new() -> Self {
        Self { heap: UnsafeCell::new(None) }
    }

    pub fn init(&self, heap_addr: VirtAddr, heap_size: usize) {

        let heap = self.heap.get();
        let start_ptr = heap_addr.as_mut_ptr();

        unsafe {
            *heap = Some(SimpleHeap {

                start: start_ptr,
                ptr: start_ptr,

                trackers: [None; NB_BLOCK_SIZES * NB_BLOCK_SIZES],

                stats: AllocStats {
                    total: heap_size,
                    explored: 0,
                    allocated: 0,
                    lost: 0,
                    reclaimable: 0,
                }
            })
        }
    }

    pub fn size(&self) -> usize {
        self.get_heap().stats.total
    }

    pub fn get_stats(&self) -> AllocStats {
        self.get_heap().stats.clone()
    }

    fn get_heap(&self) -> &SimpleHeap {
        unsafe {
            self.heap.get().as_ref().unwrap().as_ref().expect("Allocator not initialized")
        }
    }

    fn get_heap_mut(&self) -> &mut SimpleHeap {
        unsafe {
            self.heap.get().as_mut().unwrap().as_mut().expect("Allocator not initialized")
        }
    }
}

unsafe impl Sync for SimpleAllocator {}

unsafe impl GlobalAlloc for SimpleAllocator {

    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {

        let size = layout.size();
        let align = layout.align();

        let (tracker_index, block_size) = get_tracker(size, align);

        let heap = self.get_heap_mut();

        heap.stats.allocated += size;

        let alloc_ptr = match heap.trackers[tracker_index] {

            // Reclaiming a previously allocated and unused block
            Some(tracker_ptr) => {

                // Getting the next block in the linked list by reading at this memory location
                let next_addr = {
                    let s = core::slice::from_raw_parts(tracker_ptr, 8);
                    let s: &[u8; 8] = s.try_into().unwrap();
                    u64::from_le_bytes(*s)
                };
                
                heap.trackers[tracker_index] = {
                    if next_addr == 0x0 { None } // We've exhausted the reclaimable blocks for this block size
                    else { Some(VirtAddr::new(next_addr).as_mut_ptr()) }
                };
    
                heap.stats.reclaimable -= block_size;
    
                tracker_ptr
            },

            // Creating a new block
            None => {

                let offset = heap.ptr.align_offset(align);
                heap.ptr = heap.ptr.add(offset);
        
                let alloc_ptr = heap.ptr;
        
                heap.ptr = heap.ptr.add(block_size);
                heap.stats.lost += offset;
                heap.stats.explored = {
                    let p0 = heap.start as usize;
                    let p1 = heap.ptr as usize;
                    p1 - p0
                };
    
                alloc_ptr
            }
        };

        alloc_ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {

        let size = layout.size();
        let align = layout.align();

        let (tracker_index, block_size) = get_tracker(size, align);

        let heap = self.get_heap_mut();

        let prev_addr = match heap.trackers[tracker_index] {

            Some(tracker_ptr) => VirtAddr::from_ptr(tracker_ptr).as_u64(),

            // No other block in the list to link to
            None => 0x0,
        };

        let s = core::slice::from_raw_parts_mut(ptr, 8);
        s.copy_from_slice(&prev_addr.to_le_bytes());

        heap.trackers[tracker_index] = Some(ptr);

        heap.stats.allocated -= size;
        heap.stats.reclaimable += block_size;
    }
}


fn get_tracker(size: usize, align: usize) -> (usize, usize) {

    // Block needs to be big enough to contain a linked list pointer
    let block_size = usize::max(8, size.next_power_of_two());

    let size_index = usize::ilog2(block_size) as usize;
    let align_index = usize::ilog2(align) as usize;

    let tracker_index = size_index * NB_BLOCK_SIZES + align_index;

    (tracker_index, block_size)
}
