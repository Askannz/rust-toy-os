use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;


const INCREMENT: usize = 0x10000;

pub struct SimpleAllocator {
    pub heap: UnsafeCell<SimpleHeap>,
}

pub struct SimpleHeap {
    start: *mut u8,
    ptr: *mut u8,
    size: usize,
    next_report: usize,
} 

impl SimpleAllocator {
    pub const fn new() -> Self {
        Self { heap:  UnsafeCell::new(SimpleHeap { ptr: (0x1600000) as *mut u8, start: (0x1600000) as *mut u8, next_report: INCREMENT, size: 3095576576 })}
    }
}

unsafe impl Sync for SimpleAllocator {}

unsafe impl GlobalAlloc for SimpleAllocator {

    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let heap = self.heap.get().as_mut().unwrap();

        let offset = heap.ptr.align_offset(align);
        heap.ptr = heap.ptr.add(offset);

        let alloc_ptr = heap.ptr;

        heap.ptr = heap.ptr.add(size);

        if heap.ptr as usize > heap.next_report {
            let p0 = heap.start as usize;
            let p1 = heap.ptr as usize;
            let frac = (p1 - p0) as f64 / (heap.size as f64);
            log::debug!("Heap: {:p} frac={}", heap.ptr, frac);
            heap.next_report += INCREMENT;
        }

        alloc_ptr
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
