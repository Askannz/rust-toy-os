use core::convert::TryInto;
use alloc::collections::BTreeMap;
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::slice;
use core::mem::size_of;
use core::cell::RefCell;
use x86_64::VirtAddr;
use x86_64::structures::paging::OffsetPageTable;
use bitvec::prelude::{Lsb0, BitVec};
use bitvec::view::BitView;
use bitvec::field::BitField;
use volatile::Volatile;
use crate::serial_println;
use crate::{pci::{PciDevice, PciBar}, get_phys_addr};

pub mod input;
pub mod gpu;
pub mod network;

#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum FeatureBits {
    VIRTIO_F_VERSION_1 = 0x1
}

pub const Q_SIZE: usize = 64;

pub struct BootInfo {
    pub physical_memory_offset: u64
}

pub struct VirtioInterruptAck {
    isr_ptr: Volatile<&'static mut u8>,
    pub latest_status: Option<IsrStatus>
}

unsafe impl Sync for VirtioInterruptAck {}

pub struct VirtioDevice {
    pub pci_device: PciDevice,
    capabilities: Vec<VirtioCapability>,
    pub common_config: RefCell<Volatile<&'static mut VirtioPciCommonCfg>>,
    pub queues: BTreeMap<u16, VirtioQueue>
}

struct VirtioCapability {
    cfg_type: u8,
    pci_config_space_offset: u8,
    bar: usize,
    bar_offset: u32,
    length: u32  // Length of the structure pointed to
}

pub struct VirtioQueue {
    q_index: u16,
    buffers: Vec<Vec<u8>>,
    descriptor_area: Box<VirtqDescTable>,
    driver_area: Box<VirtqAvail>,
    device_area: Box<VirtqUsed>,
    avail_desc: [bool; Q_SIZE],
    pop_index: usize,
    notify_ptr: VirtAddr
}

trait VirtqSerializable: Copy {}

unsafe fn to_bytes<T: VirtqSerializable>(obj: T) -> Vec<u8> {
    let ptr = (&obj as *const T) as *const u8;
    slice::from_raw_parts(ptr, size_of::<T>()).to_vec()
}

unsafe fn from_bytes<T: VirtqSerializable>(bytes: Vec<u8>) -> T {

    let t_size = size_of::<T>();
    assert!(t_size <= bytes.len());
    let s = &bytes[0..t_size];

    let ptr = s.as_ptr() as *const T;
    *ptr
}

pub enum QueueMessage {
    DevWriteOnly { size: usize },
    DevReadOnly { buf: Vec<u8> }
}

#[derive(Debug, Clone, Copy)]
pub struct IsrStatus { 
    pub queue: bool,
    pub config: bool
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtioPciCommonCfg { 

    pub device_feature_select: u32,
    pub device_feature: u32,
    pub driver_feature_select: u32,
    pub driver_feature: u32, 

    pub msix_config: u16,
    pub num_queues: u16, 

    pub device_status: u8,
    pub config_generation: u8,

    pub queue_select: u16,
    pub queue_size: u16,
    pub queue_msix_vector: u16,
    pub queue_enable: u16,
    pub queue_notify_off: u16,

    pub queue_desc: u64,
    pub queue_driver: u64,
    pub queue_device: u64
}

impl VirtioInterruptAck {
    pub fn ack_interrupt(&mut self) {
        let isr = self.isr_ptr.read();
        let isr_bits = isr.view_bits::<Lsb0>();
        let isr_status = IsrStatus {
            queue: isr_bits[0],
            config: isr_bits[1]
        };
        self.latest_status = Some(isr_status);
    }
}

impl VirtioQueue {

    fn get_descriptor(&mut self) -> Option<usize> {
        for (desc_index, available) in self.avail_desc.iter_mut().enumerate() {
            if *available {
                *available = false;
                return Some(desc_index)
            }
        }
        return None
    }

    pub fn try_push(&mut self, messages: Vec<QueueMessage>) -> Option<()> {

        let n = messages.len();

        let desc_indices: Vec<usize> = (0..n)
            .map(|_| self.get_descriptor())
            .collect::<Option<Vec<usize>>>()?;

        for (i, msg) in messages.into_iter().enumerate() {

            let desc_index = desc_indices[i];
            let descriptor = self.descriptor_area.get_mut(desc_index).unwrap();

            match msg {
                QueueMessage::DevReadOnly { buf } => {

                    let buffer = self.buffers.get_mut(desc_index).unwrap();

                    assert!(buf.len() <= buffer.len());
                    buffer[0..buf.len()].copy_from_slice(&buf);
        
                    descriptor.flags = 0x0;
                    descriptor.len = buf.len() as u32;
                },
                QueueMessage::DevWriteOnly { size } => {
                    descriptor.flags = 0x2;
                    descriptor.len = size as u32;
                }
            }

            if i < n - 1 {
                descriptor.next = desc_indices[i + 1] as u16;
                descriptor.flags |= 0x1;
            }
        }

        let ring_index: usize = self.driver_area.idx.into();
        self.driver_area.ring[ring_index % Q_SIZE] = desc_indices[0] as u16;

        self.driver_area.idx += 1;

        let q_index: u8 = self.q_index.try_into().unwrap();

        let mut ptr = {
            let ptr: *mut u16 = self.notify_ptr.as_mut_ptr();
            Volatile::new(unsafe {ptr.as_mut().unwrap()})
        };

        ptr.write(q_index as u16);

        Some(())
    }

    pub fn try_pop(&mut self) -> Option<Vec<Vec<u8>>> {

        let new_index: usize = self.device_area.idx.into();
        if new_index == self.pop_index {
            return None;
        }

        let idx: usize = self.pop_index.try_into().unwrap();
        let it: VirtqUsedElem = self.device_area.ring[idx % Q_SIZE];
        //serial_println!("Received element: {:?}", it);

        let mut out = Vec::new();
        let mut desc_index: usize = it.id.try_into().unwrap();

        loop {
            
            let out_vec = self.buffers[desc_index].clone();
            out.push(out_vec);

            let descriptor = self.descriptor_area.get(desc_index).unwrap();
            //serial_println!("Received descriptor: {:?}", descriptor);

            let next_desc = descriptor.next.into();

            self.avail_desc[desc_index] = true;

            if next_desc != 0 {
                desc_index = next_desc
            } else {
                break;
            }
        }

        self.pop_index += 1;

        Some(out)
    }
}

impl VirtioDevice {

   pub fn new(
        boot_info: &'static BootInfo,
        pci_device: PciDevice,
        feature_bits: u32,
    ) -> Self {

        let virtio_capabilities = pci_device.capabilities.iter()
            .filter(|cap| cap.vendor == 0x09)  // VirtIO vendor
            .map(|cap| {

                let word_0 = pci_device.read_config_space(cap.offset);
                let word_1 = pci_device.read_config_space(cap.offset + 0x4);
                let word_2 = pci_device.read_config_space(cap.offset + 0x8);
                let word_3 = pci_device.read_config_space(cap.offset + 0x12);

                let bits_0 = word_0.view_bits::<Lsb0>();
                let bits_1 = word_1.view_bits::<Lsb0>();

                let cfg_type = bits_0[24..32].load();
                let bar = bits_1[..8].load();
                let bar_offset = word_2;
                let length = word_3;

                VirtioCapability {
                    cfg_type, bar, bar_offset, length,
                    pci_config_space_offset: cap.offset
                }
            })
            .collect::<Vec<VirtioCapability>>();

        let phys_offset = VirtAddr::new(boot_info.physical_memory_offset);

        let common_config_ptr = {

            let common_config_cap = virtio_capabilities.iter()
                .find(|cap| cap.cfg_type == 0x01)
                .expect("No VirtIO common config capability?");

            let bar_addr = match pci_device.bars[&common_config_cap.bar] {
                PciBar::Memory { base_addr, .. } => base_addr,
                PciBar::IO { .. } => unimplemented!(
                    "Support for I/O BARs in VirtIO not implemented")
            };

            let addr = phys_offset + bar_addr + (common_config_cap.bar_offset as u64);
            let ptr: *mut VirtioPciCommonCfg = addr.as_mut_ptr();
            Volatile::new(unsafe { ptr.as_mut().unwrap() })
        };

        let mut dev = VirtioDevice {
            pci_device,
            capabilities: virtio_capabilities,
            common_config: RefCell::new(common_config_ptr),
            queues: BTreeMap::new()
        };

        dev.initialize(feature_bits);

        dev
    }

    pub fn get_ack_object(&self, boot_info: &'static BootInfo) -> VirtioInterruptAck {

        let phys_offset = VirtAddr::new(boot_info.physical_memory_offset);

        let isr_status_ptr = {

            let isr_cap = self.capabilities.iter()
                .find(|cap| cap.cfg_type == 0x03)
                .expect("No VirtIO ISR config capability?");

            let bar_addr = match self.pci_device.bars[&isr_cap.bar] {
                PciBar::Memory { base_addr, .. } => base_addr,
                PciBar::IO { .. } => unimplemented!(
                    "Support for I/O BARs in VirtIO not implemented")
            };

            let addr = phys_offset + bar_addr + (isr_cap.bar_offset as u64);
            let ptr: *mut u8 = addr.as_mut_ptr();
            Volatile::new(unsafe { ptr.as_mut().unwrap() })
        };

        VirtioInterruptAck {
            isr_ptr: isr_status_ptr,
            latest_status: None
        }
    }

    fn initialize(&mut self, feature_bits: u32) {

        self.write_status(0x0);  // RESET

        self.pci_device.disable_msix();

        self.write_status(0x01);  // ACKNOWLEDGE
        self.write_status(0x02);  // DRIVER

        /*
            Very very basic feature bits negociation:
            we only accept the VIRTIO_F_VERSION_1 bit
            (minimum required). Should work for the Input
            Device type, unsure about the others.
        */

        let bits_0 = 0u32;
        let bits_1 = feature_bits;

        self.write_feature_bits(0x0, bits_0);
        self.write_feature_bits(0x1, bits_1);

        self.write_status(0x08);  // FEATURES_OK

        // Making sure features have been accepted
        let status = self.read_status();
        assert_eq!(status, 0x08);
    }

    pub fn initialize_queue(
        &mut self,
        boot_info: &'static BootInfo,
        mapper: &OffsetPageTable,
        q_index: u16,
        max_buf_size: usize
    ) {

        let mut desc_table = Box::new({

            let zero_desc = VirtqDesc {
                addr: 0x0,
                len: 0,
                flags: 0x0,
                next: 0
            };
    
            [zero_desc; Q_SIZE]
        });
    
        let mut available_ring = Box::new(VirtqAvail {
            flags: 0x0,
            idx: 0,
            ring: [0u16; Q_SIZE],
            used_event: 0 // Unused
        });
    
        let mut used_ring = Box::new({
    
            let zero_used_elem = VirtqUsedElem { id: 0, len: 0};
    
            VirtqUsed {
                flags: 0x0,
                idx: 0,
                ring: [zero_used_elem; Q_SIZE],
                avail_event: 0 // Unused
            }
        });


        // Calculating addresses
    
        let descr_area_addr = get_phys_addr(
            mapper,
            desc_table.as_mut() as *mut VirtqDescTable);
        let driver_area_addr = get_phys_addr(
            mapper,
            available_ring.as_mut() as *mut VirtqAvail);
        let dev_area_addr = get_phys_addr(
            mapper,
            used_ring.as_mut() as *mut VirtqUsed);

        serial_println!("descr_area_addr={:x}", descr_area_addr);
        serial_println!("driver_area_addr={:x}", driver_area_addr);
        serial_println!("dev_area_addr={:x}", dev_area_addr);

        {
            let mut common_config = self.common_config.borrow_mut();

            common_config.map_mut(|c| &mut c.queue_select).update(|v| *v = q_index);
            common_config.map_mut(|c| &mut c.queue_desc).update(|v| *v = descr_area_addr);
            common_config.map_mut(|c| &mut c.queue_driver).update(|v| *v = driver_area_addr);
            common_config.map_mut(|c| &mut c.queue_device).update(|v| *v = dev_area_addr);
            common_config.map_mut(|c| &mut c.queue_enable).update(|v| *v = 1);

            // Reading back queue size
            let q_size: usize = common_config.map(|s| &s.queue_size).read().into();
            assert_eq!(q_size, Q_SIZE);
        }

        // Allocating buffers
        let mut buffers = Vec::new();

        for index in 0..Q_SIZE {

            let mut buf = Vec::new();
            buf.resize(max_buf_size, 0);

            let buf_ptr = buf.as_mut_ptr();

            desc_table[index] = VirtqDesc {
                addr: get_phys_addr(mapper, buf_ptr),
                len: 0,
                flags: 0x0,
                next: 0
            };

            buffers.push(buf);
        }

        let avail_desc = [true; Q_SIZE];

        let notify_ptr = self.get_queue_notify_ptr(boot_info, q_index);

        let queue = VirtioQueue {
            q_index,
            buffers,
            descriptor_area: desc_table,
            driver_area: available_ring,
            device_area: used_ring,
            avail_desc,
            pop_index: 0,
            notify_ptr
        };

        let o = self.queues.insert(q_index, queue);
        assert!(o.is_none());
    }

    fn get_queue_notify_ptr(&self, boot_info: &'static BootInfo, q_index: u16) -> VirtAddr {

        let phys_offset = VirtAddr::new(boot_info.physical_memory_offset);

        let notify_config_cap = self.capabilities.iter()
            .find(|cap| cap.cfg_type == 0x02)
            .expect("No VirtIO notify config capability?");

        let bar_addr = match self.pci_device.bars[&notify_config_cap.bar] {
            PciBar::Memory { base_addr, .. } => base_addr,
            PciBar::IO { .. } => unimplemented!(
                "Support for I/O BARs in VirtIO not implemented")
        };

        let queue_notify_off: u64 = {

            let mut common_config = self.common_config.borrow_mut();

            common_config
                .map_mut(|s| &mut s.queue_select)
                .update(|queue_select| *queue_select = q_index);

            common_config
                .map(|s| &s.queue_notify_off)
                .read()
                .into()
        };

        let notify_off_multiplier: u64 = self.pci_device.read_config_space(
            notify_config_cap.pci_config_space_offset + 4
        ).into();

        let addr = 
            phys_offset + 
            bar_addr + 
            (notify_config_cap.bar_offset as u64) +
            queue_notify_off * notify_off_multiplier;

        addr
    }

    pub fn write_status(&self, val: u8) {

        self.common_config.borrow_mut()
            .map_mut(|s| &mut s.device_status)
            .write(val);
    }

    pub fn read_status(&self) -> u8 {

        self.common_config.borrow_mut()
            .map(|s| &s.device_status)
            .read()
    }

    pub fn get_virtio_device_type(&self) -> u16 {
        self.pci_device.device_id - 0x1040
    }

    fn write_feature_bits(&self, select: u32, val: u32) {

        let mut common_config = self.common_config.borrow_mut();

        common_config
            .map_mut(|s| &mut s.driver_feature_select)
            .update(|sel_val| *sel_val = select);

        common_config
            .map_mut(|s| &mut s.driver_feature)
            .update(|feat_val| *feat_val = val);
    }

    fn read_feature_bits(&self, select: u32) -> u32 {

        let mut common_config = self.common_config.borrow_mut();

        common_config
            .map_mut(|s| &mut s.device_feature_select)
            .update(|sel_val| *sel_val = select);

        common_config
            .map(|s| &s.device_feature)
            .read()
    }
}


type VirtqDescTable = [VirtqDesc; Q_SIZE];

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16
}

#[repr(C, align(2))]
pub struct VirtqAvail {
    flags: u16,
    pub idx: u16,
    ring: [u16; Q_SIZE],
    used_event: u16
}

#[repr(C, align(4))]
pub struct VirtqUsed {
    flags: u16,
    pub idx: u16,
    ring: [VirtqUsedElem; Q_SIZE],
    avail_event: u16
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct VirtqUsedElem {
    id: u32,
    len: u32
}
