use alloc::vec::Vec;

use smoltcp::phy::{self, Device, DeviceCapabilities, Medium};
use smoltcp::time::Instant;

use crate::virtio::network::{VirtioNetwork, MAX_PACKET_SIZE};

pub struct SmolTcpVirtio {
    pub virtio_dev: VirtioNetwork,
}

impl SmolTcpVirtio {
    pub fn new(virtio_dev: VirtioNetwork) -> SmolTcpVirtio {
        SmolTcpVirtio {
            virtio_dev
        }
    }
}

impl Device for SmolTcpVirtio {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken<'a>;

    fn capabilities(&self) -> DeviceCapabilities {

        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = MAX_PACKET_SIZE;
        caps.medium = Medium::Ethernet;

        caps
    }

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.virtio_dev.try_recv().map(move |buffer| {
            let rx = RxToken { buffer };
            let tx = TxToken {
                virtio_dev: &mut self.virtio_dev,
            };
            (rx, tx)
        })
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(TxToken {
            virtio_dev: &mut self.virtio_dev,
        })
    }
}

#[doc(hidden)]
pub struct RxToken {
    buffer: Vec<u8>,
}

impl phy::RxToken for RxToken {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        f(&mut self.buffer)
    }
}

#[doc(hidden)]
pub struct TxToken<'a> {
    virtio_dev: &'a mut VirtioNetwork,
}

impl<'a> phy::TxToken for TxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = Vec::new();
        buffer.resize(len, 0);
        let result = f(&mut buffer);
        self.virtio_dev.try_send(buffer);
        result
    }
}
