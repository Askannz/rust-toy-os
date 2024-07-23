/*
    Loosely adapted from
    https://github.com/smoltcp-rs/smoltcp/blob/533f103a9544fa0de7d75383b13fc021f7b0642b/src/phy/loopback.rs
*/

use tinyvec::ArrayVec;

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
    buffer: [u8; MAX_PACKET_SIZE],
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
        let mut buffer = ArrayVec::<[u8; MAX_PACKET_SIZE]>::new();
        for _ in 0..len { buffer.push(0x00); }
        let result = f(&mut buffer);
        self.virtio_dev.send(buffer);
        result
    }
}
