use core::convert::TryInto;
use core::hash::Hasher;
use alloc::borrow::ToOwned;
use alloc::boxed::Box;

use core::{mem, usize};
use x86_64::{VirtAddr, PhysAddr};
use volatile::Volatile;
use tinyvec::ArrayVec;

use crate::pci::{PciDevice, PciBar, PciConfigSpace};
use crate::memory;


const VIRTIO_PCI_VENDOR: u8 = 0x09;

pub mod input;
pub mod gpu;
pub mod network;

#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum FeatureBits {
    VIRTIO_F_VERSION_1 = 0x1
}

#[allow(dead_code)]
pub struct VirtioInterruptAck {
    isr_ptr: Volatile<&'static mut u8>,
    pub latest_status: Option<IsrStatus>
}

unsafe impl Sync for VirtioInterruptAck {}

#[allow(dead_code)]
pub struct VirtioDevice {
    pci_device: PciDevice,
    common_config_cap: VirtioCapability,
    notification_cap: VirtioCapability,
    device_specific_config_cap: Option<VirtioCapability>,
    pub common_config: Volatile<&'static mut VirtioPciCommonCfg>,

}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(non_camel_case_types, dead_code)]
pub enum CfgType {
    VIRTIO_PCI_CAP_COMMON_CFG = 0x1,
    VIRTIO_PCI_CAP_NOTIFY_CFG = 0x2,
    VIRTIO_PCI_CAP_ISR_CFG = 0x3,
    VIRTIO_PCI_CAP_DEVICE_CFG = 0x4,
    VIRTIO_PCI_CAP_PCI_CFG = 0x5,
}

struct VirtQStorage<const Q_SIZE: usize> {
    descriptor_area: VirtqDescTable<Q_SIZE>,
    driver_area: VirtqAvail<Q_SIZE>,
    device_area: VirtqUsed<Q_SIZE>,
    avail_desc: [bool; Q_SIZE],
}

impl<const Q_SIZE: usize> VirtQStorage<Q_SIZE> {
    const fn new() -> Self {

        let desc_table = {

            let zero_desc = VirtqDesc {
                addr: 0x0,
                len: 0,
                flags: 0x0,
                next: 0
            };
    
            [zero_desc; Q_SIZE]
        };
    
        let available_ring = VirtqAvail {
            flags: 0x0,
            idx: 0,
            ring: [0u16; Q_SIZE],
            used_event: 0 // Unused
        };
    
        let used_ring = {
    
            let zero_used_elem = VirtqUsedElem { id: 0, len: 0};
    
            VirtqUsed {
                flags: 0x0,
                idx: 0,
                ring: [zero_used_elem; Q_SIZE],
                avail_event: 0 // Unused
            }
        };

        let avail_desc = [true; Q_SIZE];

        VirtQStorage {
            descriptor_area: desc_table,
            driver_area: available_ring,
            device_area: used_ring,
            avail_desc
        }
    }
}

pub struct VirtioQueue<const Q_SIZE: usize, const BUF_SIZE: usize> {
    q_index: u16,
    storage: Box<VirtQStorage<Q_SIZE>>,
    pop_index: usize,
    notify_ptr: VirtAddr,
}

pub trait VirtqSerializable: Clone + Default {}

#[derive(Clone)]
pub enum QueueMessage<T: VirtqSerializable> {
    DevWriteOnly,
    DevReadOnly { data: T, len: Option<usize> }
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

impl<const Q_SIZE: usize, const BUF_SIZE: usize> VirtioQueue<Q_SIZE, BUF_SIZE> {

    fn take_descriptor(&mut self) -> Option<usize> {
        for (desc_index, available) in self.storage.avail_desc.iter_mut().enumerate() {
            if *available {
                *available = false;
                return Some(desc_index)
            }
        }
        return None
    }

    fn return_descriptor(&mut self, desc_index: usize){
        self.storage.avail_desc[desc_index] = true;
    }

    pub unsafe fn try_push<T: VirtqSerializable, const N: usize>(&mut self, messages: &[QueueMessage<T>; N]) -> Option<()> {

        let n = messages.len();
        assert!(n > 0);

        let desc_indices = {
            let mut desc_indices = [0usize; N];
            for i in 0..N {
                match self.take_descriptor() {
                    Some(desc_index) => desc_indices[i] = desc_index,
                    None => {  // We couldn't reserve the required number of descriptors
    
                        // Returning already reserved descriptors and bailing out
                        for desc_index in &desc_indices[..i] {
                            self.return_descriptor(*desc_index)
                        }
                        return None
                    }
                }
            }
            desc_indices
        };

        for (i, msg) in messages.into_iter().enumerate() {

            let desc_index = desc_indices[i];

            let descriptor = self.storage.descriptor_area.get_mut(desc_index).unwrap();

            let buffer = match msg {
                QueueMessage::DevReadOnly { data, len } => {
                    descriptor.flags = 0x0;
                    descriptor.len = match *len {
                        Some(len) => len,
                        None => mem::size_of::<T>()
                    } as u32;
                    data.clone()
                },
                QueueMessage::DevWriteOnly => {
                    descriptor.flags = 0x2;
                    descriptor.len = mem::size_of::<T>() as u32;
                    T::default()
                }
            };

            let mapper = memory::get_mapper();

            unsafe { 
                let virt_addr = mapper.phys_to_virt(PhysAddr::new(descriptor.addr));
                let mut desc_buffer: Box<T> = Box::from_raw(virt_addr.as_mut_ptr());
                *desc_buffer = buffer;
                Box::leak(desc_buffer);
            };

            if i < n - 1 {
                descriptor.next = desc_indices[i + 1] as u16;
                descriptor.flags |= 0x1;
            }
        }

        let ring_index: usize = self.storage.driver_area.idx.into();
        self.storage.driver_area.ring[ring_index % Q_SIZE] = desc_indices[0] as u16;

        self.storage.driver_area.idx += 1;

        Some(())
    }

    pub unsafe fn notify_device(&self) {

        let q_index: u8 = self.q_index.try_into().unwrap();

        let mut ptr = {
            let ptr: *mut u16 = self.notify_ptr.as_mut_ptr();
            Volatile::new(unsafe {ptr.as_mut().unwrap()})
        };

        ptr.write(q_index as u16);

    }

    pub unsafe fn try_pop<T: VirtqSerializable, const N: usize>(&mut self) -> Option<[T; N]> {

        let mapper = memory::get_mapper();

        let new_index: usize = self.storage.device_area.idx.into();
        if new_index == self.pop_index {
            return None;
        }

        let idx: usize = self.pop_index.try_into().unwrap();
        let it: VirtqUsedElem = self.storage.device_area.ring[idx % Q_SIZE];
        //log::debug!("Received element: {:?}", it);

        let mut out = ArrayVec::<[T; N]>::new();
        let mut desc_index: usize = it.id.try_into().unwrap();

        loop {

            let descriptor = self.storage.descriptor_area.get(desc_index).unwrap();
            //log::debug!("Received descriptor: {:?}", descriptor);
            unsafe { 
                let virt_addr = mapper.phys_to_virt(PhysAddr::new(descriptor.addr));
                let desc_buffer: Box<T> = Box::from_raw(virt_addr.as_mut_ptr());
                out.push(*desc_buffer.to_owned());
                Box::leak(desc_buffer);
            };

            let next_desc = descriptor.next.into();

            self.return_descriptor(desc_index);

            if next_desc != 0 {
                desc_index = next_desc
            } else {
                break;
            }
        }

        self.pop_index += 1;

        Some(out.into_inner())
    }
}

#[derive(Debug)]
pub struct VirtioCapability {
    config_space_offset: u8, 
    virtio_cap: VirtioPciCap,
}


#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioPciCap { 
    cap_vndr: u8,
    cap_next: u8,
    cap_len: u8,
    cfg_type: CfgType,
    bar: u8,
    padding: [u8; 3],
    offset: u32,  // This is the offset into the BAR, and NOT the PCI config space
    length: u32,
}

pub fn get_addr_in_bar(pci_device: &PciDevice, virtio_cap: &VirtioPciCap) -> VirtAddr {

    let bar_addr = match pci_device.bars[&virtio_cap.bar.into()] {
        PciBar::Memory { base_addr, .. } => base_addr,
        PciBar::IO { .. } => unimplemented!()
    };

    let phys_addr = PhysAddr::new(bar_addr + (virtio_cap.offset as u64));

    memory::get_mapper().phys_to_virt(phys_addr)
}

impl VirtioDevice {

    pub fn new(
        pci_device: PciDevice,
        feature_bits: u32,
    ) -> Self {

        let mut pci_config_space = PciConfigSpace::new();

        let mut find_cap = |cfg_type: CfgType| -> Option<VirtioCapability> {
            pci_device.capabilities.iter()
                .filter(|pci_cap| pci_cap.vendor == VIRTIO_PCI_VENDOR)
                .filter_map(|pci_cap| {

                    let virtio_cap = unsafe {
                        pci_config_space.read_struct::<VirtioPciCap>(&pci_device.addr, pci_cap.offset)
                    };

                    if virtio_cap.cfg_type != cfg_type {
                        return None
                    }

                    Some(VirtioCapability { 
                        config_space_offset: pci_cap.offset,
                        virtio_cap,
                    })
                })
                .next()
        };

        let common_config_cap = find_cap(CfgType::VIRTIO_PCI_CAP_COMMON_CFG);
        let notification_cap = find_cap(CfgType::VIRTIO_PCI_CAP_NOTIFY_CFG);
        let device_specific_config_cap = find_cap(CfgType::VIRTIO_PCI_CAP_DEVICE_CFG);

        let common_config_cap = common_config_cap.unwrap();
        let notification_cap = notification_cap.unwrap();

        let common_config = {
            let addr = get_addr_in_bar(&pci_device, &common_config_cap.virtio_cap);
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

    pub fn initialize_queue<const Q_SIZE: usize, const BUF_SIZE: usize>(
        &mut self,
        q_index: u16,
    ) -> VirtioQueue<Q_SIZE, BUF_SIZE> {

        let mapper = memory::get_mapper();

        // TODO: prevent a queue from being initialized twice

        let mut storage = Box::new(VirtQStorage::new());

        for descriptor in storage.descriptor_area.iter_mut(){
            let buffer = Box::new([0u8; BUF_SIZE]);
            let buf_ref = Box::leak(buffer);
            let pys_addr =  memory::get_mapper().ref_to_phys(buf_ref);
            descriptor.addr = pys_addr.as_u64();
        }

        // Calculating addresses
    
        let descr_area_addr = mapper.ref_to_phys(storage.descriptor_area.as_ref()).as_u64();
        let driver_area_addr = mapper.ref_to_phys(&storage.driver_area).as_u64();
        let dev_area_addr = mapper.ref_to_phys(&storage.device_area).as_u64();

        // log::debug!("descr_area_addr={:x}", descr_area_addr);
        // log::debug!("driver_area_addr={:x}", driver_area_addr);
        // log::debug!("dev_area_addr={:x}", dev_area_addr);

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
        

        let notify_ptr = self.get_queue_notify_ptr(q_index);

        VirtioQueue {
            q_index,
            storage,
            pop_index: 0,
            notify_ptr,
        }
    }

    unsafe fn read_device_specific_config<T>(&self) -> &'static T {

        let cap = self.device_specific_config_cap.as_ref().unwrap();

        let addr = get_addr_in_bar(&self.pci_device, &cap.virtio_cap);
        let ptr = addr.as_ptr() as *const T;

        ptr.as_ref().unwrap()
    }

    fn get_queue_notify_ptr(&mut self, q_index: u16) -> VirtAddr {

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

        let base_addr = get_addr_in_bar(&self.pci_device, &self.notification_cap.virtio_cap);
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

    fn write_feature_bits(&mut self, select: u32, val: u32) {

        self.common_config
            .map_mut(|s| &mut s.driver_feature_select)
            .update(|sel_val| *sel_val = select);

        self.common_config
            .map_mut(|s| &mut s.driver_feature)
            .update(|feat_val| *feat_val = val);
    }

    #[allow(dead_code)]
    fn read_feature_bits(&mut self, select: u32) -> u32 {

        self.common_config
            .map_mut(|s| &mut s.device_feature_select)
            .update(|sel_val| *sel_val = select);

        self.common_config
            .map(|s| &s.device_feature)
            .read()
    }
}


pub type VirtqDescTable<const Q_SIZE: usize> = [VirtqDesc; Q_SIZE];

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
