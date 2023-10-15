
use core::mem::MaybeUninit;

use alloc::vec;
use alloc::vec::Vec;
use x86_64::structures::paging::OffsetPageTable;
use crate::{virtio::BootInfo};
use super::{VirtioDevice, VirtioQueue, QueueMessage, VirtqSerializable};

const Q_SIZE: usize = 256;
// https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html#x1-2050006
pub const MAX_PACKET_SIZE: usize = 1514;

#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum NetworkFeatureBits {
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
    pub fn new(boot_info: &'static BootInfo, mapper: &'static OffsetPageTable, mut virtio_dev: VirtioDevice) -> Self {

        let mut receiveq1 = virtio_dev.initialize_queue(boot_info, mapper, 0);  // queue 0 (receiveq1)
        let transmitq1 = virtio_dev.initialize_queue(boot_info, mapper, 1);  // queue 1 (transmitq1)
        virtio_dev.write_status(0x04);  // DRIVER_OK

        let device_config = unsafe {
            virtio_dev.read_device_specific_config::<VirtioNetConfig>(boot_info)
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

    pub fn try_send(&mut self, value: Vec<u8>) -> Option<()> {

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
            ])
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
