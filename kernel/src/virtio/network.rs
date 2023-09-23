use core::mem::size_of;
use alloc::vec;
use alloc::vec::Vec;
use x86_64::structures::paging::{OffsetPageTable};
use crate::{virtio::BootInfo, serial_println};
use super::{VirtioDevice, VirtioQueue, QueueMessage, VirtqSerializable, from_bytes, to_bytes};

pub struct VirtioNetwork {
    pub virtio_dev: VirtioDevice,
}

impl VirtioNetwork {
    pub fn new(boot_info: &'static BootInfo, mapper: &OffsetPageTable, mut virtio_dev: VirtioDevice) -> Self {

        let virtio_dev_type = virtio_dev.get_virtio_device_type();
        serial_println!("virtio_dev_type={}", virtio_dev_type);

        // https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html#x1-2050006
        let max_buf_size = 1526;

        virtio_dev.initialize_queue(boot_info, &mapper, 0, max_buf_size);  // queue 0 (receiveq1)
        virtio_dev.initialize_queue(boot_info, &mapper, 1, max_buf_size);  // queue 1 (transmitq1)
        virtio_dev.write_status(0x04);  // DRIVER_OK
    
        let receiveq = virtio_dev.queues.get_mut(&0).unwrap();
        while let Some(_) = receiveq.try_push(vec![
            QueueMessage::DevWriteOnly { size: max_buf_size }
        ]) {}

        VirtioNetwork {
            virtio_dev
        }
    }
}
