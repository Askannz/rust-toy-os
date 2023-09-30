use core::convert::TryInto;

use alloc::boxed::Box;
use alloc::vec::Vec;

use core::mem;
use x86_64::VirtAddr;
use x86_64::structures::paging::OffsetPageTable;
use bitvec::prelude::Lsb0;
use bitvec::view::BitView;
use volatile::Volatile;
use crate::serial_println;
use crate::{pci::{PciDevice, PciBar, PciConfigSpace}, get_phys_addr};


const VIRTIO_PCI_VENDOR: u8 = 0x09;

pub mod input;
pub mod gpu;
pub mod network;

#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum FeatureBits {
    VIRTIO_F_VERSION_1 = 0x1
}

pub struct BootInfo {
    pub physical_memory_offset: u64
}

pub struct VirtioInterruptAck {
    isr_ptr: Volatile<&'static mut u8>,
    pub latest_status: Option<IsrStatus>
}

unsafe impl Sync for VirtioInterruptAck {}

pub struct VirtioDevice {
    pci_device: PciDevice,
    common_config_cap: VirtioCapability,
    notification_cap: VirtioCapability,
    device_specific_config_cap: Option<VirtioCapability>,
    pub common_config: Volatile<&'static mut VirtioPciCommonCfg>,

}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(non_camel_case_types)]
pub enum CfgType {
    VIRTIO_PCI_CAP_COMMON_CFG = 0x1,
    VIRTIO_PCI_CAP_NOTIFY_CFG = 0x2,
    VIRTIO_PCI_CAP_ISR_CFG = 0x3,
    VIRTIO_PCI_CAP_DEVICE_CFG = 0x4,
    VIRTIO_PCI_CAP_PCI_CFG = 0x5,
}

pub struct VirtioQueue<const Q_SIZE: usize, T: VirtqSerializable> {
    q_index: u16,
    buffers: Box<[Box<T>]>,
    descriptor_area: Box<VirtqDescTable<Q_SIZE>>,
    driver_area: Box<VirtqAvail<Q_SIZE>>,
    device_area: Box<VirtqUsed<Q_SIZE>>,
    avail_desc: [bool; Q_SIZE],
    pop_index: usize,
    notify_ptr: VirtAddr
}

pub trait VirtqSerializable: Clone + Default {}

#[derive(Clone)]
pub enum QueueMessage<T: VirtqSerializable> {
    DevWriteOnly,
    DevReadOnly { data: T }
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

impl<const Q_SIZE: usize, T: VirtqSerializable> VirtioQueue<Q_SIZE, T> {

    fn get_descriptor(&mut self) -> Option<usize> {
        for (desc_index, available) in self.avail_desc.iter_mut().enumerate() {
            if *available {
                *available = false;
                return Some(desc_index)
            }
        }
        return None
    }

    pub fn try_push(&mut self, messages: Vec<QueueMessage<T>>) -> Option<()> {

        let n = messages.len();

        let desc_indices: Vec<usize> = (0..n)
            .map(|_| self.get_descriptor())
            .collect::<Option<Vec<usize>>>()?;

        for (i, msg) in messages.into_iter().enumerate() {

            let desc_index = desc_indices[i];
            let descriptor = self.descriptor_area.get_mut(desc_index).unwrap();

            match msg {
                QueueMessage::DevReadOnly { data } => {

                    let buffer = self.buffers.get_mut(desc_index).unwrap().as_mut();
                    *buffer = data;
        
                    descriptor.flags = 0x0;
                    descriptor.len = mem::size_of::<T>() as u32;
                },
                QueueMessage::DevWriteOnly => {
                    descriptor.flags = 0x2;
                    descriptor.len = mem::size_of::<T>() as u32;
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

    pub fn try_pop(&mut self) -> Option<Vec<T>> {

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
            
            let out_val = self.buffers[desc_index].as_ref().clone();
            out.push(out_val);

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

#[derive(Debug)]
pub struct VirtioCapability {
    config_space_offset: u8, 
    virtio_cap: VirtioPciCap,
}


#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct VirtioPciCap { 
    cap_vndr: u8,
    cap_next: u8,
    cap_len: u8,
    cfg_type: CfgType,
    bar: u8,
    padding: [u8; 3],
    offset: u32,  // This is the offset into the BAR, and NOT the PCI config space
    length: u32,
}

fn get_addr_in_bar(boot_info: &'static BootInfo, pci_device: &PciDevice, virtio_cap: VirtioPciCap) -> VirtAddr {

    let phys_offset = VirtAddr::new(boot_info.physical_memory_offset);

    let bar_addr = match pci_device.bars[&virtio_cap.bar.into()] {
        PciBar::Memory { base_addr, .. } => base_addr,
        PciBar::IO { .. } => unimplemented!()
    };

    phys_offset + bar_addr + (virtio_cap.offset as u64)
}

impl VirtioDevice {

    pub fn new(
        boot_info: &'static BootInfo,
        pci_device: PciDevice,
        feature_bits: u32,
    ) -> Self {

        let mut pci_config_space = PciConfigSpace::new();

        let mut common_config_cap = None;
        let mut notification_cap = None;
        let mut device_specific_config_cap = None;

        pci_device.capabilities.iter()
            .filter(|pci_cap| pci_cap.vendor == VIRTIO_PCI_VENDOR)
            .for_each(|pci_cap| {

                let virtio_cap = unsafe {
                    pci_config_space.read_struct::<VirtioPciCap>(&pci_device.addr, pci_cap.offset)
                };

                match virtio_cap.cfg_type {

                    CfgType::VIRTIO_PCI_CAP_COMMON_CFG => {
                        assert!(common_config_cap.is_none());
                        common_config_cap.replace(VirtioCapability { 
                            config_space_offset: pci_cap.offset,
                            virtio_cap,
                        });
                    }
                    
                    CfgType::VIRTIO_PCI_CAP_NOTIFY_CFG => {
                        assert!(notification_cap.is_none());
                        notification_cap.replace(VirtioCapability { 
                            config_space_offset: pci_cap.offset,
                            virtio_cap,
                        });
                    },

                    CfgType::VIRTIO_PCI_CAP_DEVICE_CFG => {
                        assert!(device_specific_config_cap.is_none());
                        device_specific_config_cap.replace(VirtioCapability { 
                            config_space_offset: pci_cap.offset,
                            virtio_cap,
                        });
                    }

                    _ => ()
                };
            });

        let common_config_cap = common_config_cap.unwrap();
        let notification_cap = notification_cap.unwrap();

        let common_config = {
            let addr = get_addr_in_bar(boot_info, &pci_device, common_config_cap.virtio_cap);
            let ptr = addr.as_mut_ptr() as *mut VirtioPciCommonCfg;
            Volatile::new(unsafe { ptr.as_mut().unwrap() })
        };

        let mut dev = VirtioDevice {
            pci_device,
            common_config_cap,
            notification_cap,
            device_specific_config_cap,
            common_config,
        };

        dev.initialize(feature_bits);

        dev
    }

    /*
    pub fn get_ack_object(&self, boot_info: &'static BootInfo) -> VirtioInterruptAck {

        let phys_offset = VirtAddr::new(boot_info.physical_memory_offset);

        let isr_status_ptr = {

            let isr_cap = self.capabilities.iter()
                .find(|cap| cap.virtio_cap.cfg_type == CfgType::VIRTIO_PCI_CAP_ISR_CFG)
                .expect("No VirtIO ISR config capability?");

            let bar_addr = match self.pci_device.bars[&isr_cap.virtio_cap.bar.into()] {
                PciBar::Memory { base_addr, .. } => base_addr,
                PciBar::IO { .. } => unimplemented!(
                    "Support for I/O BARs in VirtIO not implemented")
            };

            let addr = phys_offset + bar_addr + (isr_cap.virtio_cap.offset as u64);
            let ptr: *mut u8 = addr.as_mut_ptr();
            Volatile::new(unsafe { ptr.as_mut().unwrap() })
        };

        VirtioInterruptAck {
            isr_ptr: isr_status_ptr,
            latest_status: None
        }
    }
    */

    fn initialize(&mut self, feature_bits: u32) {

        self.write_status(0x0);  // RESET

        self.pci_device.disable_msix();

        self.write_status(0x01);  // ACKNOWLEDGE
        self.write_status(0x02);  // DRIVER

        let bits_0 = feature_bits;
        let bits_1 = FeatureBits::VIRTIO_F_VERSION_1 as u32;

        self.write_feature_bits(0x0, bits_0);
        self.write_feature_bits(0x1, bits_1);

        self.write_status(0x08);  // FEATURES_OK

        // Making sure features have been accepted
        let status = self.read_status();
        assert_eq!(status, 0x08);
    }

    pub fn initialize_queue<const Q_SIZE: usize, T: VirtqSerializable>(
        &mut self,
        boot_info: &'static BootInfo,
        mapper: &OffsetPageTable,
        q_index: u16,
    ) -> VirtioQueue<Q_SIZE, T> {

        // TODO: prevent a queue from being initialized twice

        let mut desc_table = Box::new({

            let zero_desc = VirtqDesc {
                addr: 0x0,
                len: 0,
                flags: 0x0,
                next: 0
            };
    
            [zero_desc; Q_SIZE]
        });
    
        let available_ring = Box::new(VirtqAvail {
            flags: 0x0,
            idx: 0,
            ring: [0u16; Q_SIZE],
            used_event: 0 // Unused
        });
    
        let used_ring = Box::new({
    
            let zero_used_elem = VirtqUsedElem { id: 0, len: 0};
    
            VirtqUsed {
                flags: 0x0,
                idx: 0,
                ring: [zero_used_elem; Q_SIZE],
                avail_event: 0 // Unused
            }
        });


        // Calculating addresses
    
        let descr_area_addr = get_phys_addr(mapper, desc_table.as_ref());
        let driver_area_addr = get_phys_addr(mapper, available_ring.as_ref());
        let dev_area_addr = get_phys_addr(mapper, used_ring.as_ref());

        // serial_println!("descr_area_addr={:x}", descr_area_addr);
        // serial_println!("driver_area_addr={:x}", driver_area_addr);
        // serial_println!("dev_area_addr={:x}", dev_area_addr);

        {

            self.common_config.map_mut(|c| &mut c.queue_select).update(|v| *v = q_index);
            self.common_config.map_mut(|c| &mut c.queue_desc).update(|v| *v = descr_area_addr);
            self.common_config.map_mut(|c| &mut c.queue_driver).update(|v| *v = driver_area_addr);
            self.common_config.map_mut(|c| &mut c.queue_device).update(|v| *v = dev_area_addr);
            self.common_config.map_mut(|c| &mut c.queue_enable).update(|v| *v = 1);

            // Reading back queue size
            let q_size: usize = self.common_config.map(|s| &s.queue_size).read().into();
            assert_eq!(q_size, Q_SIZE);
        }

        // Allocating buffers
        let buffers: Vec<Box<T>> = (0..Q_SIZE).map(|_| Box::new(T::default())).collect();
        let buffers: Box<[Box<T>]> = buffers.into_boxed_slice();

        for (index, buf) in buffers.iter().enumerate() {

            desc_table[index] = VirtqDesc {
                addr: get_phys_addr(mapper, buf.as_ref()),
                len: 0,
                flags: 0x0,
                next: 0
            };
        }

        let avail_desc = [true; Q_SIZE];

        let notify_ptr = self.get_queue_notify_ptr(boot_info, q_index);

        VirtioQueue {
            q_index,
            buffers,
            descriptor_area: desc_table,
            driver_area: available_ring,
            device_area: used_ring,
            avail_desc,
            pop_index: 0,
            notify_ptr
        }
    }

    fn get_queue_notify_ptr(&mut self, boot_info: &'static BootInfo, q_index: u16) -> VirtAddr {

        serial_println!("{:?}", self.notification_cap);

        let mut pci_config_space = PciConfigSpace::new();

        let queue_notify_off: u64 = {

            self.common_config
                .map_mut(|s| &mut s.queue_select)
                .update(|queue_select| *queue_select = q_index);

            self.common_config
                .map(|s| &s.queue_notify_off)
                .read()
                .into()
        };

        let notify_off_multiplier: u64 = unsafe {
            let offset = self.notification_cap.config_space_offset + 4;
            pci_config_space.read(&self.pci_device.addr, offset)
        }.into();

        let base_addr = get_addr_in_bar(boot_info, &self.pci_device, self.notification_cap.virtio_cap);
        let addr = base_addr + queue_notify_off * notify_off_multiplier;
        
        addr
    }

    pub fn write_status(&mut self, val: u8) {
        self.common_config
            .map_mut(|s| &mut s.device_status)
            .write(val);
    }

    pub fn read_status(&self) -> u8 {
        self.common_config
            .map(|s| &s.device_status)
            .read()
    }

    pub fn get_virtio_device_type(&self) -> u16 {
        self.pci_device.device_id - 0x1040
    }

    fn write_feature_bits(&mut self, select: u32, val: u32) {

        self.common_config
            .map_mut(|s| &mut s.driver_feature_select)
            .update(|sel_val| *sel_val = select);

        self.common_config
            .map_mut(|s| &mut s.driver_feature)
            .update(|feat_val| *feat_val = val);
    }

    fn read_feature_bits(&mut self, select: u32) -> u32 {

        self.common_config
            .map_mut(|s| &mut s.device_feature_select)
            .update(|sel_val| *sel_val = select);

        self.common_config
            .map(|s| &s.device_feature)
            .read()
    }
}


type VirtqDescTable<const Q_SIZE: usize> = [VirtqDesc; Q_SIZE];

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16
}

#[repr(C, align(2))]
pub struct VirtqAvail<const Q_SIZE: usize> {
    flags: u16,
    pub idx: u16,
    ring: [u16; Q_SIZE],
    used_event: u16
}

#[repr(C, align(4))]
pub struct VirtqUsed<const Q_SIZE: usize> {
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
