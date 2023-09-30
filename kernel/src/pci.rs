use alloc::borrow::ToOwned;
use alloc::{rc::Rc, vec::Vec, vec, boxed::Box};
use core::mem;
use core::cell::RefCell;
use alloc::collections::BTreeMap;
use x86_64::instructions::port::{PortWriteOnly, Port};
use bitvec::prelude::Lsb0;
use bitvec::view::BitView;
use bitvec::field::BitField;

use crate::serial_println;




#[derive(Debug)]
pub struct PciDevice {

    pub addr: PciAddress,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,

    pub capabilities: Vec<PciCapability>,
    pub bars: BTreeMap<usize, PciBar>,
}

#[derive(Debug, Clone)]
pub struct PciAddress {
    bus: u8,
    device: u8,
    function: u8
}

#[derive(Debug, Clone)]
pub struct PciCapability {
    pub vendor: u8,
    pub offset: u8
}

#[derive(Debug)]
pub struct PciConfigSpace {
    address_port: PortWriteOnly<u32>,
    data_port: Port<u32>
}

#[derive(Debug, Clone, Copy)]
pub enum PciBar { 
    Memory {
        addr_type: BarAddrType,
        prefetchable: bool,
        base_addr: u64,
        size: u32
    },
    IO {
        base_addr: u32,
        size: u32
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BarAddrType { Bar32, Bar64 }


impl PciDevice {
    pub fn set_interrupt_line(&self, line: u8) {

        let mut pci_config_space = PciConfigSpace::new();

        let mut word = unsafe { pci_config_space.read(&self.addr, 0x3c) };

        let bits = word.view_bits_mut::<Lsb0>();

        let line: u32 = line.into();
        let line_bits = line.view_bits();
        bits[..8].copy_from_bitslice(&line_bits[..8]);
        let new_word = bits.load::<u32>();

        unsafe { pci_config_space.write(&self.addr, 0x3c, new_word) };
    }

    pub fn read_interrupt_line(&self) -> u8 {

        let mut pci_config_space = PciConfigSpace::new();

        let mut word = unsafe { pci_config_space.read(&self.addr, 0x3c) };

        let bits = word.view_bits_mut::<Lsb0>();
        bits[..8].load()
    }

    pub fn disable_msix(&self) {

        let mut pci_config_space = PciConfigSpace::new();
        
        let cap = self.capabilities.iter()
            .find(|cap| cap.vendor == 0x11);
        
        let cap = 
            if let Some(cap) = cap { cap }
            else { return };

        let mut word = unsafe { pci_config_space.read(&self.addr, cap.offset) };

        let bits = word.view_bits_mut::<Lsb0>();
        bits.set(31, false);

        unsafe { pci_config_space.write(&self.addr, cap.offset, bits.load()) };
    }
}


pub fn enumerate() -> impl Iterator<Item=PciDevice> {

    let pci_config_space = {
        let pci_config_space = PciConfigSpace::new();
        Rc::new(RefCell::new(pci_config_space))
    };

    // Don't care about multi-function devices for now
    let function = 0;

    let bus_iter = 0..=255u8;
    let device_iter = 0..32u8;

    bus_iter
        .flat_map(move |bus| device_iter.clone().map(move |device| (bus, device)))
        .filter_map(move |(bus, device)| {

            let mut config_ref = pci_config_space.borrow_mut();
            let addr = PciAddress { bus, device, function };

            let word_0 = unsafe { config_ref.read(&addr, 0x0) };

            // No device at this address
            if word_0 == u32::MAX { return None }

            // Header type
            let word_0c = unsafe { config_ref.read(&addr, 0x0c) };
            let bits_0c = word_0c.view_bits::<Lsb0>();
            let mut header_bits = bits_0c[16..24].to_owned();
            header_bits.set(7, false);
            let header_type = header_bits.load::<u8>();
            assert_eq!(header_type, 0x00); // We don't support PCI bridges for now

            // Device/Vendor IDs
            let bits_0 = word_0.view_bits::<Lsb0>();
            let device_id = bits_0[16..32].load();
            let vendor_id = bits_0[0..16].load();

            // Device class
            let word_8 = unsafe { config_ref.read(&addr, 0x8) };
            let bits_8 = word_8.view_bits::<Lsb0>();
            let class = bits_8[24..32].load();

            // PCI capabilities
            let capabilities = {

                let mut cap_ptr = {
                    let mut word_34 = unsafe { config_ref.read(&addr, 0x34) };
                    let bits_34 = word_34.view_bits_mut::<Lsb0>();
                    bits_34[..2].fill(false);
                    bits_34[..8].load::<u8>()
                };


                let mut capabilities = Vec::new();
                while cap_ptr != 0x00 {

                    let mut word_0 = unsafe { config_ref.read(&addr, cap_ptr) };
                    let bits_0 = word_0.view_bits_mut::<Lsb0>();

                    let vendor = bits_0[..8].load();
    
                    capabilities.push(PciCapability {
                        vendor, offset: cap_ptr
                    });

                    cap_ptr = bits_0[8..16].load();
                }

                capabilities
            };

            // BARS
            let bars = {

                let mut bars = BTreeMap::new();
                let mut it = 0..6;

                while let Some(i) = it.next() {

                    let offset = 0x10 + 0x4 * (i as u8);
                    let word_bars = unsafe { config_ref.read(&addr, offset) };

                    let bits_bar = word_bars.view_bits::<Lsb0>();

                    let io_mapped = bits_bar[0];
                    let n_flags = if io_mapped { 2 } else { 4 };

                    // BAR size
                    let size: u32 = {

                        let mut word_size = unsafe {
                            config_ref.write(&addr, offset, u32::MAX);
                            let word_size = config_ref.read(&addr, offset);
                            config_ref.write(&addr, offset, word_bars);
                            word_size
                        };

                        let bits_size = word_size.view_bits_mut::<Lsb0>();
                        bits_size[..n_flags].fill(false);
                        let val: u32 = bits_size.load();
                        
                        if val == 0 { 0 }
                        else { !val + 1 }
                    };

                    if size == 0 {
                        continue;
                    }
            
                    let bar = match io_mapped {
                
                        // Memory-mapped BAR
                        false => {

                            // 32 or 64-bit BAR?
                            let addr_type = match bits_bar[1..3].load::<u8>() {
                                0x00 => BarAddrType::Bar32,
                                0x02 => BarAddrType::Bar64,
                                val => panic!("Unsupported BAR size type: {}", val)
                            };

                            // Address (lower bits)
                            let addr_low_bits = {
                                let mut val: u64 = bits_bar.load();
                                let addr_bits = val.view_bits_mut();
                                addr_bits[..4].fill(false);
                                addr_bits.to_owned()
                            };
            
                            let base_addr = match addr_type {

                                BarAddrType::Bar32 => addr_low_bits.load::<u64>(),

                                BarAddrType::Bar64 => {

                                    // Grabbing high bits of the address from next BAR
                                    let next_i = it.next().expect("64-bit BAR but already in last BAR");
                                    let next_offset = 0x10 + 0x4 * (next_i as u8);
                                    let next_word_bars = unsafe { config_ref.read(&addr, next_offset) };
                                    let next_word_bars: u64 = next_word_bars.into(); 
                                    let addr_high_bits = next_word_bars.view_bits::<Lsb0>();

                                    let addr_low_bits = addr_low_bits.as_bitslice();

                                    let mut addr = 0u64;
                                    let addr_bits = addr.view_bits_mut::<Lsb0>();
                                    
                                    addr_bits[00..32].copy_from_bitslice(&addr_low_bits[..32]);
                                    addr_bits[32..64].copy_from_bitslice(&addr_high_bits[..32]);

                                    addr_bits.load::<u64>()
                                }
                            };
            
                            PciBar::Memory {
                                addr_type,
                                prefetchable: bits_bar[3],
                                base_addr,
                                size
                            }
                        },
            
                        // I/0-mapped BAR
                        true => {
                            let mut addr_bits = bits_bar.to_owned();
                            addr_bits[..2].fill(false);
                            let base_addr = addr_bits.load::<u32>();
                            PciBar::IO { base_addr, size }
                        }
                    };
            
                    bars.insert(i, bar);
                }

                bars
            };

            Some(PciDevice {
                addr, vendor_id, device_id, class,
                capabilities, bars,
            })
        })
}


impl PciConfigSpace {

    pub fn new() -> Self {
        PciConfigSpace {
            address_port: PortWriteOnly::<u32>::new(0xCF8),
            data_port: Port::<u32>::new(0xCFC)
        }
    }

    pub unsafe fn read_struct<T: Clone>(&mut self, addr: &PciAddress, offset: u8) -> T {

        let n = mem::size_of::<T>();
        assert_eq!(n % 4, 0);
        let num_words = n / 4;

        let buf: Vec<u32> = (0..num_words).map(|i| {
            let i: u8 = i.try_into().unwrap();
            let addr_word = Self::get_addr_word(addr, offset + 4 * i);
            self.address_port.write(addr_word);
            self.data_port.read()
        }).collect();

        let ptr = buf.as_ptr() as *const T;
        ptr.as_ref().unwrap().clone()
    }

    // Unsafe because addr and offset have to point to valid data
    pub unsafe fn read(&mut self, addr: &PciAddress, offset: u8) -> u32 {

        let addr_word = Self::get_addr_word(addr, offset);

        self.address_port.write(addr_word);
        self.data_port.read() 
    }

    // Same
    pub unsafe fn write(&mut self, addr: &PciAddress, offset: u8, val: u32) {

        let addr_word = Self::get_addr_word(addr, offset);

        self.address_port.write(addr_word);
        self.data_port.write(val);
    }

    fn get_addr_word(addr: &PciAddress, offset: u8) -> u32 {

        let mut val = 0u32;
        let bits = val.view_bits_mut::<Lsb0>();

        let offset: u32 = offset.into();
        let function: u32 = addr.function.into();
        let device: u32 = addr.device.into();
        let bus: u32 = addr.bus.into();

        let offset_bits = offset.view_bits::<Lsb0>();
        let function_bits = function.view_bits::<Lsb0>();
        let device_bits = device.view_bits::<Lsb0>();
        let bus_bits = bus.view_bits::<Lsb0>();
        let enable_bits = (1u32 << 7).view_bits::<Lsb0>();

        bits[00..08].copy_from_bitslice(&offset_bits[..8]);
        bits[08..11].copy_from_bitslice(&function_bits[..3]);
        bits[11..16].copy_from_bitslice(&device_bits[..5]);
        bits[16..24].copy_from_bitslice(&bus_bits[..8]);
        bits[24..32].copy_from_bitslice(&enable_bits[..8]);

        bits.load()
    }
}
