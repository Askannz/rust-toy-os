use alloc::vec;
use alloc::vec::Vec;
use core::{mem::{MaybeUninit, size_of}, sync::atomic::{fence, Ordering}};
use x86_64::{structures::paging::{OffsetPageTable}, VirtAddr};
use crate::virtio::BootInfo;
use crate::{serial_println, pci::PciBar};
use crate::get_phys_addr;

use super::{VirtioDevice, VirtioQueue, QueueMessage, VirtqSerializable, to_bytes, from_bytes};

pub const W: usize = 1920;
pub const H: usize = 1080;

pub struct VirtioGPU {
    pub virtio_dev: VirtioDevice,
    pub framebuffer: Vec<u8>,
}

impl VirtioGPU {
    pub fn new(boot_info: &'static BootInfo, mapper: &OffsetPageTable, mut virtio_dev: VirtioDevice) -> Self {

        let virtio_dev_type = virtio_dev.get_virtio_device_type();
        if virtio_dev_type != 16 {
            panic!("VirtIO device is not a GPU device (device type = {}, expected 16)", virtio_dev_type)
        }

        let max_buf_size = *vec![
            size_of::<VirtioGpuRespDisplayInfo>(),
            size_of::<VirtioGpuResourceCreate2d>(),
            size_of::<VirtioGpuResourceAttachBacking>(),
            size_of::<VirtioGpuSetScanout>(),
            size_of::<VirtioGpuTransferToHost2d>(),
            size_of::<VirtioGpuResourceFlush>()
        ].iter().max().unwrap();

        virtio_dev.initialize_queue(boot_info, &mapper, 0, max_buf_size);  // queue 0 (controlq)
        virtio_dev.write_status(0x04);  // DRIVER_OK

        VirtioGPU {
            virtio_dev,
            framebuffer: vec![0u8; 4*W*H],
        }
    }

    pub fn get_dims(&self) -> (usize, usize) {
        (W, H)
    }

    pub fn get_max_scanouts(&self, boot_info: &'static BootInfo) -> u32 {

        let addr = self.get_device_config_bar_addr(boot_info);
        let ptr: *mut u32 = addr.as_mut_ptr();
        
        unsafe { *ptr.offset(2) }
    }

    pub fn read_events(&self, boot_info: &'static BootInfo) -> u32 {

        let addr = self.get_device_config_bar_addr(boot_info);
        let ptr: *mut u32 = addr.as_mut_ptr();
        
        unsafe { *ptr }
    }

    pub fn clear_events(&self, boot_info: &'static BootInfo) {

        let addr = self.get_device_config_bar_addr(boot_info);
        let ptr: *mut u32 = addr.as_mut_ptr();
        
        unsafe { *ptr.offset(1) = 1 };
        fence(Ordering::SeqCst);
    }

    fn get_device_config_bar_addr(&self, boot_info: &'static BootInfo) -> VirtAddr {

        let phys_offset = VirtAddr::new(boot_info.physical_memory_offset);

        let config_cap = self.virtio_dev.capabilities.iter()
            .find(|cap| cap.cfg_type == 0x04)
            .expect("No VirtIO device config capability?");

        let bar_addr = match self.virtio_dev.pci_device.bars[&config_cap.bar] {
            PciBar::Memory { base_addr, .. } => base_addr,
            PciBar::IO { .. } => unimplemented!(
                "Support for I/O BARs in VirtIO not implemented")
        };

        phys_offset + bar_addr + (config_cap.bar_offset as u64)
    }

    fn send_command<U, V>(&mut self, input: U) -> V 
        where U: VirtqSerializable, V: VirtqSerializable
    {

        let controlq = self.virtio_dev.queues.get_mut(&0).unwrap();

        controlq.try_push(vec![
            QueueMessage::DevReadOnly { buf: to_bytes(&input) },
            QueueMessage::DevWriteOnly { size: size_of::<V>() }
        ]).unwrap();

        loop {
           if let Some(resp_list) = controlq.try_pop() {
                assert_eq!(resp_list.len(), 2);
                let resp_buf = &resp_list[1];
                // TODO: check response status code
                break from_bytes(&resp_buf);
           }
        }
    }

    fn send_command_noreply<U: VirtqSerializable>(&mut self, input: U) -> Option<()> {
        let resp: VirtioGpuCtrlHdr = self.send_command(input);
        if resp._type == VirtioGpuCtrlType::VIRTIO_GPU_RESP_OK_NODATA as u32 {
            Some(())
        } else {
            serial_println!("Resp type: 0x{:x}", resp._type);
            None
        }
    }

    pub fn get_display_info(&mut self) -> VirtioGpuRespDisplayInfo {

        self.send_command(VirtioGpuCtrlHdr {
            _type: VirtioGpuCtrlType::VIRTIO_GPU_CMD_GET_DISPLAY_INFO as u32,
            ..VirtioGpuCtrlHdr::default()
        })
    }

    pub fn init_framebuffer(&mut self, mapper: &OffsetPageTable) {

        let resource_id = 0x1;

        self.send_command_noreply(VirtioGpuResourceCreate2d {
            hdr: VirtioGpuCtrlHdr {
                _type: VirtioGpuCtrlType::VIRTIO_GPU_CMD_RESOURCE_CREATE_2D as u32,
                ..VirtioGpuCtrlHdr::default()
            },
            resource_id,
            format: 67, // RGBA,
            width: W as u32,
            height: H as u32
        }).unwrap();

        let fb_addr = get_phys_addr(mapper, self.framebuffer.as_mut_ptr());

        self.send_command_noreply(VirtioGpuResourceAttachBacking {
            hdr: VirtioGpuCtrlHdr {
                _type: VirtioGpuCtrlType::VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING as u32,
                ..VirtioGpuCtrlHdr::default()
            },
            resource_id,
            nr_entries: 1,
            entries: {
                let mut entries = [VirtioGpuMemEntry::default(); MAX_MEM_PAGES];
                entries[0] = VirtioGpuMemEntry {
                    addr: fb_addr,
                    length: self.framebuffer.len() as u32,
                    padding: 0x0
                };
                entries
            }
        }).unwrap();

        self.send_command_noreply(VirtioGpuSetScanout {
            hdr: VirtioGpuCtrlHdr {
                _type: VirtioGpuCtrlType::VIRTIO_GPU_CMD_SET_SCANOUT as u32,
                ..VirtioGpuCtrlHdr::default()
            },
            r: VirtioGpuRect {
                x: 0,
                y: 0,
                width: W as u32,
                height: H as u32
            },
            scanout_id: 0,
            resource_id,
        }).unwrap();
    }

    pub fn flush(&mut self) {

        let resource_id = 0x1;

        self.send_command_noreply(VirtioGpuTransferToHost2d {
            hdr: VirtioGpuCtrlHdr {
                _type: VirtioGpuCtrlType::VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D as u32,
                ..VirtioGpuCtrlHdr::default()
            },
            r: VirtioGpuRect {
                x: 0,
                y: 0,
                width: W as u32,
                height: H as u32
            },
            offset: 0x0,
            resource_id,
            padding: 0x0
        }).unwrap();

        self.send_command_noreply(VirtioGpuResourceFlush {
            hdr: VirtioGpuCtrlHdr {
                _type: VirtioGpuCtrlType::VIRTIO_GPU_CMD_RESOURCE_FLUSH as u32,
                ..VirtioGpuCtrlHdr::default()
            },
            r: VirtioGpuRect {
                x: 0,
                y: 0,
                width: W as u32,
                height: H as u32
            },
            resource_id,
            padding: 0x0
        }).unwrap();
    }
}

impl VirtqSerializable for VirtioGpuCtrlHdr {}

impl Default for VirtioGpuCtrlHdr {
    fn default() -> Self {
        let x = MaybeUninit::<Self>::zeroed();
        let mut x = unsafe { x.assume_init() };
        x.flags = 0x1;
        x
    }
}

const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct VirtioGpuCtrlHdr { 
    _type: u32,
    flags: u32, 
    fence_id: u64,
    ctx_id: u32,
    padding: u32
}

#[repr(C)]
#[allow(non_camel_case_types)]
enum VirtioGpuCtrlType {
    VIRTIO_GPU_CMD_GET_DISPLAY_INFO = 0x0100,
    VIRTIO_GPU_CMD_RESOURCE_CREATE_2D = 0x0101,
    VIRTIO_GPU_CMD_SET_SCANOUT = 0x0103,
    VIRTIO_GPU_CMD_RESOURCE_FLUSH = 0x0104,
    VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D = 0x0105,
    VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING = 0x0106,
    
    VIRTIO_GPU_RESP_OK_NODATA = 0x1100
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioGpuRect { 
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32
}


//
// VIRTIO_GPU_CMD_GET_DISPLAY_INFO

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtioGpuRespDisplayInfo {
    hdr: VirtioGpuCtrlHdr,
    pub pmodes: [VirtioGpuDisplayOne; VIRTIO_GPU_MAX_SCANOUTS]
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioGpuDisplayOne { 
    pub r: VirtioGpuRect, 
    pub enabled: u32,
    pub flags: u32
}

impl VirtqSerializable for VirtioGpuRespDisplayInfo {}

//
// VIRTIO_GPU_CMD_RESOURCE_CREATE_2D

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioGpuResourceCreate2d { 
    hdr: VirtioGpuCtrlHdr,
    resource_id: u32,
    format: u32,
    width: u32,
    height: u32
}

impl VirtqSerializable for VirtioGpuResourceCreate2d {}

//
// VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING

const MAX_MEM_PAGES: usize = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioGpuResourceAttachBacking {
    hdr: VirtioGpuCtrlHdr,
    resource_id: u32,
    nr_entries: u32,
    entries: [VirtioGpuMemEntry; MAX_MEM_PAGES]
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioGpuMemEntry {
    addr: u64,
    length: u32,
    padding: u32
}

impl Default for VirtioGpuMemEntry {
    fn default() -> Self {
        let x = MaybeUninit::<Self>::zeroed();
        unsafe { x.assume_init() }
    }
}

impl VirtqSerializable for VirtioGpuResourceAttachBacking {}

//
// VIRTIO_GPU_CMD_SET_SCANOUT

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioGpuSetScanout { 
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    scanout_id: u32,
    resource_id: u32
}

impl VirtqSerializable for VirtioGpuSetScanout {}

//
// VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioGpuTransferToHost2d {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    offset: u64,
    resource_id: u32,
    padding: u32
}

impl VirtqSerializable for VirtioGpuTransferToHost2d {}

//
// VIRTIO_GPU_CMD_RESOURCE_FLUSH

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtioGpuResourceFlush { 
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    resource_id: u32,
    padding: u32
}

impl VirtqSerializable for VirtioGpuResourceFlush {}
