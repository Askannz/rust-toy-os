use alloc::collections::VecDeque;
use alloc::vec::Vec;

use smoltcp::phy::{self, Device, DeviceCapabilities, Medium};
use smoltcp::time::Instant;

use crate::serial_println;

/// A loopback device.
#[derive(Debug)]
pub struct Loopback {
    pub(crate) queue: DebugQueue<Vec<u8>>,
    medium: Medium,
}

impl Loopback {
    pub fn new(medium: Medium) -> Loopback {
        Loopback {
            queue: DebugQueue::new(),
            medium,
        }
    }
}

impl Device for Loopback {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken<'a>;

    fn capabilities(&self) -> DeviceCapabilities {

        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 65535;
        caps.medium = self.medium;

        caps
    }

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.queue.pop_front().map(move |buffer| {
            let rx = RxToken { buffer };
            let tx = TxToken {
                queue: &mut self.queue,
            };
            (rx, tx)
        })
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(TxToken {
            queue: &mut self.queue,
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
#[derive(Debug)]
pub struct TxToken<'a> {
    queue: &'a mut DebugQueue<Vec<u8>>,
}

impl<'a> phy::TxToken for TxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = Vec::new();
        buffer.resize(len, 0);
        let result = f(&mut buffer);
        self.queue.push_back(buffer);
        result
    }
}

#[derive(Debug)]
pub struct DebugQueue<T> {
    queue: VecDeque<T>
}

impl<T: core::fmt::Debug> DebugQueue<T> {
    pub fn new() -> Self {
        DebugQueue { queue: VecDeque::new() }
    }
    pub fn pop_front(&mut self) -> Option<T> {
        self.queue.pop_front()
    }
    pub fn push_back(&mut self, value: T) {
        serial_println!("Pushing {:x?}", value);
        self.queue.push_back(value);
    }
}


