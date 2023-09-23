use core::mem::size_of;
use alloc::vec;
use alloc::vec::Vec;
use x86_64::structures::paging::{OffsetPageTable};
use crate::{virtio::BootInfo, serial_println};
use super::{VirtioDevice, VirtioQueue, QueueMessage, VirtqSerializable, from_bytes, to_bytes};

const Q_SIZE: usize = 256;

pub struct VirtioNetwork {
    pub virtio_dev: VirtioDevice<Q_SIZE>,
}

impl VirtioNetwork {
    pub fn new(boot_info: &'static BootInfo, mapper: &OffsetPageTable, mut virtio_dev: VirtioDevice<Q_SIZE>) -> Self {

        // https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html#x1-2050006
        let max_buf_size = 1526;

        virtio_dev.initialize_queue(boot_info, &mapper, 0, max_buf_size);  // queue 0 (receiveq1)
        virtio_dev.initialize_queue(boot_info, &mapper, 1, max_buf_size);  // queue 1 (transmitq1)
        virtio_dev.write_status(0x04);  // DRIVER_OK
    
        let receiveq = virtio_dev.queues.get_mut(&0).unwrap();

        let msg = vec![QueueMessage::DevWriteOnly { size: max_buf_size }];
        while receiveq.try_push(msg.clone()).is_some() {}

        VirtioNetwork {
            virtio_dev
        }
    }
}
