
use core::mem::MaybeUninit;

use alloc::vec;
use alloc::vec::Vec;
use crate::pci::PciDevice;
use super::{VirtioDevice, VirtioQueue, QueueMessage, VirtqSerializable};

const Q_SIZE: usize = 256;
// https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html#x1-2050006
pub const MAX_PACKET_SIZE: usize = 1514;

#[repr(u32)]
#[allow(non_camel_case_types)]
enum NetworkFeatureBits {
    VIRTIO_NET_F_MAC = 0x1 << 5
}

pub struct VirtioNetwork {
    pub virtio_dev: VirtioDevice,
    pub mac_addr: [u8; 6],
    receiveq1: VirtioQueue<Q_SIZE>,
    transmitq1: VirtioQueue<Q_SIZE>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct VirtioNetConfig { 
    mac: [u8; 6],
    status: u16, 
    max_virtqueue_pairs: u16, 
    mtu: u16, 
}

impl VirtioNetwork {
    pub fn new(pci_devices: &mut Vec<PciDevice>) -> Self {

        let i = (0..pci_devices.len())
            .find(|&i| 
                pci_devices[i].vendor_id == 0x1af4 &&
                pci_devices[i].device_id == 0x1000
            )
            .expect("Cannot find VirtIO network device");

        let pci_dev = pci_devices.swap_remove(i);
        let feature_bits = NetworkFeatureBits::VIRTIO_NET_F_MAC as u32;
        let mut virtio_dev = VirtioDevice::new(pci_dev, feature_bits);


        let mut receiveq1 = virtio_dev.initialize_queue(0);  // queue 0 (receiveq1)
        let transmitq1 = virtio_dev.initialize_queue(1);  // queue 1 (transmitq1)
        virtio_dev.write_status(0x04);  // DRIVER_OK

        let device_config = unsafe {
            virtio_dev.read_device_specific_config::<VirtioNetConfig>()
        };

        let msg = vec![QueueMessage::<VirtioNetPacket>::DevWriteOnly];

        unsafe { while receiveq1.try_push(msg.clone()).is_some() {} }

        VirtioNetwork {
            virtio_dev,
            mac_addr: device_config.mac,
            receiveq1,
            transmitq1
        }
    }


    pub fn try_recv(&mut self) -> Option<Vec<u8>> {

        let resp_list = unsafe { self.receiveq1.try_pop()? };
        assert_eq!(resp_list.len(), 1);

        let virtio_packet: VirtioNetPacket = resp_list[0];

        unsafe {
            self.receiveq1.try_push(vec![
                QueueMessage::<VirtioNetPacket>::DevWriteOnly
            ]).unwrap();
        }

        Some(virtio_packet.data.to_vec())
    }

    pub fn send(&mut self, value: Vec<u8>) {

        assert!(value.len() <= MAX_PACKET_SIZE);

        let mut data = [0x00; MAX_PACKET_SIZE];

        data[0..value.len()].copy_from_slice(&value[0..value.len()]);

        let msg = VirtioNetPacket {
            hdr: VirtioNetHdr { 
                flags: 0x0,
                gso_type: 0x0,
                hdr_len: 0x0,
                gso_size: 0x0,
                csum_start: 0x0,
                csum_offset: 0x0,
                num_buffers: 0x0
            },
            data
        };

        let virtio_buf_len = value.len() + core::mem::size_of::<VirtioNetHdr>();

        unsafe {
            self.transmitq1.try_push(vec![
                QueueMessage::DevReadOnly { data: msg, len: Some(virtio_buf_len) },
            ]).unwrap()
        }

        loop {
            let resp_list_opt: Option<Vec<VirtioNetPacket>>  = unsafe { self.transmitq1.try_pop() };
            if let Some(resp_list) = resp_list_opt {
                assert_eq!(resp_list.len(), 1);
                break;
            }
         }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioNetPacket {
    pub hdr: VirtioNetHdr,
    pub data: [u8; MAX_PACKET_SIZE],
}

impl Default for VirtioNetPacket {
    fn default() -> Self {
        let x = MaybeUninit::<Self>::zeroed();
        unsafe { x.assume_init() }
    }
}

impl VirtqSerializable for VirtioNetPacket {}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioNetHdr {
    pub flags: u8,
    pub gso_type: u8,

    // TODO: proper endianness
    pub hdr_len: u16,
    pub gso_size: u16,
    pub csum_start: u16,
    pub csum_offset: u16,
    pub num_buffers: u16,
}
