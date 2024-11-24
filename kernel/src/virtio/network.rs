use core::mem::MaybeUninit;

use super::{QueueMessage, VirtioDevice, VirtioQueue, VirtqSerializable};
use crate::pci::PciDevice;
use alloc::vec::Vec;
use tinyvec::ArrayVec;

const Q_SIZE: usize = 256;
// https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html#x1-2050006
pub const MAX_PACKET_SIZE: usize = 1514;

const BUF_SIZE: usize = core::mem::size_of::<VirtioNetPacket>();

#[repr(u32)]
#[allow(non_camel_case_types)]
enum NetworkFeatureBits {
    VIRTIO_NET_F_MAC = 0x1 << 5,
}

pub struct VirtioNetwork {
    pub virtio_dev: VirtioDevice,
    pub mac_addr: [u8; 6],
    receiveq1: VirtioQueue<Q_SIZE, BUF_SIZE>,
    transmitq1: VirtioQueue<Q_SIZE, BUF_SIZE>,
    recv_counter: usize,
    sent_counter: usize,
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
            .find(|&i| pci_devices[i].vendor_id == 0x1af4 && pci_devices[i].device_id == 0x1000)
            .expect("Cannot find VirtIO network device");

        let pci_dev = pci_devices.swap_remove(i);
        let feature_bits = NetworkFeatureBits::VIRTIO_NET_F_MAC as u32;
        let mut virtio_dev = VirtioDevice::new(pci_dev, feature_bits);

        let mut receiveq1 = virtio_dev.initialize_queue(0); // queue 0 (receiveq1)
        let transmitq1 = virtio_dev.initialize_queue(1); // queue 1 (transmitq1)
        virtio_dev.write_status(0x04); // DRIVER_OK

        let device_config = unsafe { virtio_dev.read_device_specific_config::<VirtioNetConfig>() };

        let msg = [QueueMessage::<VirtioNetPacket>::DevWriteOnly];

        unsafe { while receiveq1.try_push(&msg).is_some() {} }

        VirtioNetwork {
            virtio_dev,
            mac_addr: device_config.mac,
            receiveq1,
            transmitq1,
            recv_counter: 0,
            sent_counter: 0,
        }
    }

    pub fn try_recv(&mut self) -> Option<[u8; MAX_PACKET_SIZE]> {
        let resp_list = unsafe { self.receiveq1.try_pop::<_, 1>()? };

        let virtio_packet: VirtioNetPacket = resp_list[0];

        unsafe {
            self.receiveq1
                .try_push(&[QueueMessage::<VirtioNetPacket>::DevWriteOnly])
                .unwrap();
        }

        self.recv_counter += virtio_packet.data.len();

        Some(virtio_packet.data)
    }

    pub fn send(&mut self, mut data: ArrayVec<[u8; MAX_PACKET_SIZE]>) {
        let len = data.len();
        data.resize(MAX_PACKET_SIZE, 0x00);

        let msg = VirtioNetPacket {
            hdr: VirtioNetHdr {
                flags: 0x0,
                gso_type: 0x0,
                hdr_len: 0x0,
                gso_size: 0x0,
                csum_start: 0x0,
                csum_offset: 0x0,
                num_buffers: 0x0,
            },
            data: data.into_inner(),
        };

        let virtio_buf_len = len + core::mem::size_of::<VirtioNetHdr>();

        unsafe {
            self.transmitq1
                .try_push(&[QueueMessage::DevReadOnly {
                    data: msg,
                    len: Some(virtio_buf_len),
                }])
                .unwrap();
            self.transmitq1.notify_device();
        }

        loop {
            let resp_list_opt = unsafe { self.transmitq1.try_pop::<VirtioNetPacket, 1>() };
            if let Some(_) = resp_list_opt {
                break;
            }
        }

        self.sent_counter += len;
    }

    pub fn get_counters(&mut self) -> (usize, usize) {

        let recv_counter = self.recv_counter;
        let sent_counter = self.sent_counter;
        
        self.recv_counter = 0;
        self.sent_counter = 0;

        (recv_counter, sent_counter)
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
