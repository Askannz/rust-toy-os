use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

use x86_64::VirtAddr;


const INCREMENT: usize = 0x10000;

pub struct SimpleAllocator {
    pub heap: UnsafeCell<Option<SimpleHeap>>,
}

pub struct SimpleHeap {
    start: *mut u8,
    ptr: *mut u8,
    size: usize,
    next_report: usize,
} 

impl SimpleAllocator {

    pub const fn new() -> Self {
        Self { heap: UnsafeCell::new(None) }
    }

    pub fn init(&self, heap_addr: VirtAddr, heap_size: usize) {

        let heap = self.heap.get();

        unsafe {
            *heap = Some(SimpleHeap {
                start: heap_addr.as_mut_ptr(),
                ptr: heap_addr.as_mut_ptr(),
                size: heap_size,
                next_report: INCREMENT,
            })
        }
    }

    pub fn size(&self) -> usize {
        self.get_heap().size
    }

    pub fn get_usage(&self) -> usize{

        let heap = self.get_heap();

        let p0 = heap.start as usize;
        let p1 = heap.ptr as usize;

        let usage = p1 - p0;

        usage
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

        let heap = self.get_heap_mut();

        let offset = heap.ptr.align_offset(align);
        heap.ptr = heap.ptr.add(offset);

        let alloc_ptr = heap.ptr;

        heap.ptr = heap.ptr.add(size);

        // if heap.ptr as usize > heap.next_report {
        //     let p0 = heap.start as usize;
        //     let p1 = heap.ptr as usize;
        //     let frac = (p1 - p0) as f64 / (heap.size as f64);
        //     log::debug!("Heap: {:p} frac={}", heap.ptr, frac);
        //     heap.next_report += INCREMENT;
        // }

        alloc_ptr
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
