
use alloc::vec;
use alloc::vec::Vec;
use crate::pci::PciDevice;
use super::{VirtioDevice, QueueMessage, VirtqSerializable, VirtioQueue};

const Q_SIZE: usize = 64;

pub struct VirtioInput {
    pub virtio_dev: VirtioDevice,
    eventq: VirtioQueue<Q_SIZE>
}

impl VirtioInput {
    pub fn new(pci_devices: &mut Vec<PciDevice>) -> Self {

        let i = (0..pci_devices.len())
            .find(|&i| 
                pci_devices[i].vendor_id == 0x1af4 &&
                pci_devices[i].device_id == 0x1040 + 18
            )
            .expect("Cannot find VirtIO input device");

        let pci_dev = pci_devices.swap_remove(i);
        let mut virtio_dev = VirtioDevice::new(pci_dev, 0x0);

        let mut eventq = virtio_dev.initialize_queue(0);  // queue 0 (eventq)
        //log::debug!("out of initialize_queue(): {:?}", eventq.descriptor_area.as_ptr());
        virtio_dev.write_status(0x04);  // DRIVER_OK


        let msg = vec![QueueMessage::<VirtioInputEvent>::DevWriteOnly];
        unsafe { while eventq.try_push(msg.clone()).is_some() {} };

        VirtioInput {
            virtio_dev,
            eventq
        }
    }

    pub fn poll(&mut self) -> Vec<VirtioInputEvent> {

        let mut out = Vec::new();

        while let Some(resp_list) = unsafe { self.eventq.try_pop() } {
            assert_eq!(resp_list.len(), 1);
            // TODO: check response status code
            let event = resp_list.into_iter().next().unwrap();
            out.push(event);

            // TODO: unwrap()
            unsafe{
                self.eventq.try_push(vec![
                    QueueMessage::<VirtioInputEvent>::DevWriteOnly
                ]);
            }
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
