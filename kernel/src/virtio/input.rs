
use alloc::vec;
use alloc::vec::Vec;
use x86_64::structures::paging::OffsetPageTable;
use crate::virtio::BootInfo;
use super::{VirtioDevice, QueueMessage, VirtqSerializable, VirtioQueue};

const Q_SIZE: usize = 64;

pub struct VirtioInput {
    pub virtio_dev: VirtioDevice,
    eventq: VirtioQueue<Q_SIZE, VirtioInputEvent>
}

impl VirtioInput {
    pub fn new(boot_info: &'static BootInfo, mapper: &OffsetPageTable, mut virtio_dev: VirtioDevice) -> Self {

        let virtio_dev_type = virtio_dev.get_virtio_device_type();
        if virtio_dev_type != 18 {
            panic!("VirtIO device is not an input device (device type = {}, expected 18)", virtio_dev_type)
        }

        let mut eventq = virtio_dev.initialize_queue(boot_info, &mapper, 0);  // queue 0 (eventq)
        virtio_dev.write_status(0x04);  // DRIVER_OK


        let msg = vec![QueueMessage::DevWriteOnly];
        while eventq.try_push(msg.clone()).is_some() {}

        VirtioInput {
            virtio_dev,
            eventq
        }
    }

    pub fn poll(&mut self) -> Vec<VirtioInputEvent> {

        let mut out = Vec::new();

        while let Some(resp_list) = self.eventq.try_pop() {
            assert_eq!(resp_list.len(), 1);
            // TODO: check response status code
            let event = resp_list.into_iter().next().unwrap();
            out.push(event);

            // TODO: unwrap()
            self.eventq.try_push(vec![
                QueueMessage::DevWriteOnly
            ]);
        }

        out
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct VirtioInputEvent {
    pub _type: u16,
    pub code: u16,
    pub value: u32
}

impl VirtqSerializable for VirtioInputEvent {}

impl Default for VirtioInputEvent {
    fn default() -> Self {
        Self {
            _type: 0,
            code: 0,
            value: 0
        }
    }
}
