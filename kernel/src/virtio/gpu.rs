use alloc::{vec, boxed::Box};

use core::mem::MaybeUninit;
use crate::serial_println;
use crate::memory;

use super::{VirtioDevice, QueueMessage, VirtqSerializable, VirtioQueue};

pub const W: usize = 1366;
pub const H: usize = 768;

const Q_SIZE: usize = 64;

pub struct VirtioGPU {
    pub virtio_dev: VirtioDevice,
    pub framebuffer: Box<[u8]>,
    controlq: VirtioQueue<Q_SIZE>
}

#[repr(C)]
#[derive(Clone, Copy)]
union GpuVirtioMsg {
    resp_display_info: VirtioGpuRespDisplayInfo,
    resource_create_2d: VirtioGpuResourceCreate2d,
    resource_attach_backing: VirtioGpuResourceAttachBacking,
    set_scanout: VirtioGpuSetScanout,
    transfer_to_host_2d: VirtioGpuTransferToHost2d,
    resource_flush: VirtioGpuResourceFlush,
    ctrl_hdr: VirtioGpuCtrlHdr,
}

// TODO: is there a cleaner way?
impl Default for GpuVirtioMsg {
    fn default() -> Self {
        let x = MaybeUninit::<Self>::zeroed();
        unsafe { x.assume_init() }
    }
}

impl VirtqSerializable for GpuVirtioMsg {}

impl VirtioGPU {
    pub fn new(mut virtio_dev: VirtioDevice) -> Self {

        let virtio_dev_type = virtio_dev.get_virtio_device_type();
        if virtio_dev_type != 16 {
            panic!("VirtIO device is not a GPU device (device type = {}, expected 16)", virtio_dev_type)
        }

        let controlq = virtio_dev.initialize_queue(0);  // queue 0 (controlq)
        virtio_dev.write_status(0x04);  // DRIVER_OK

        VirtioGPU {
            virtio_dev,
            framebuffer: vec![0u8; W*H*4].into_boxed_slice(),
            controlq
        }
    }

    pub fn get_dims(&self) -> (usize, usize) {
        (W, H)
    }

    /*
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
            .find(|cap| cap.virtio_cap.cfg_type == CfgType::VIRTIO_PCI_CAP_DEVICE_CFG)
            .expect("No VirtIO device config capability?");

        let bar_addr = match self.virtio_dev.pci_device.bars[&config_cap.virtio_cap.bar.into()] {
            PciBar::Memory { base_addr, .. } => base_addr,
            PciBar::IO { .. } => unimplemented!(
                "Support for I/O BARs in VirtIO not implemented")
        };

        phys_offset + bar_addr + (config_cap.virtio_cap.offset as u64)
    }
    */

    fn send_command(&mut self, input: GpuVirtioMsg) -> GpuVirtioMsg {

        unsafe {
            self.controlq.try_push(vec![
                QueueMessage::DevReadOnly { data: input, len: None },
                QueueMessage::DevWriteOnly
            ]).unwrap();
        }


        loop {
           if let Some(resp_list) = unsafe { self.controlq.try_pop() } {
                assert_eq!(resp_list.len(), 2);
                // TODO: check response status code
                break resp_list[1];
           }
        }
    }

    fn send_command_noreply(&mut self, input: GpuVirtioMsg) -> Option<()> {
        let resp = self.send_command(input);
        let resp: VirtioGpuCtrlHdr = unsafe { resp.ctrl_hdr };
        if resp._type == VirtioGpuCtrlType::VIRTIO_GPU_RESP_OK_NODATA as u32 {
            Some(())
        } else {
            serial_println!("Resp type: 0x{:x}", resp._type);
            None
        }
    }

    #[allow(dead_code)]
    pub fn get_display_info(&mut self) -> VirtioGpuRespDisplayInfo {

        let res = self.send_command(GpuVirtioMsg { ctrl_hdr: VirtioGpuCtrlHdr {
            _type: VirtioGpuCtrlType::VIRTIO_GPU_CMD_GET_DISPLAY_INFO as u32,
            ..VirtioGpuCtrlHdr::default()
        }});

        unsafe { res.resp_display_info }
    }

    pub fn init_framebuffer(&mut self) {

        let resource_id = 0x1;

        self.send_command_noreply(GpuVirtioMsg { resource_create_2d: VirtioGpuResourceCreate2d {
            hdr: VirtioGpuCtrlHdr {
                _type: VirtioGpuCtrlType::VIRTIO_GPU_CMD_RESOURCE_CREATE_2D as u32,
                ..VirtioGpuCtrlHdr::default()
            },
            resource_id,
            format: 67, // RGBA,
            width: W as u32,
            height: H as u32
        }}).unwrap();

        let fb_addr = memory::get_mapper().ref_to_phys(self.framebuffer.as_ref()).as_u64();

        self.send_command_noreply(GpuVirtioMsg { resource_attach_backing: VirtioGpuResourceAttachBacking {
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
        }}).unwrap();

        self.send_command_noreply(GpuVirtioMsg { set_scanout: VirtioGpuSetScanout {
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
        }}).unwrap();
    }

    pub fn flush(&mut self) {

        let resource_id = 0x1;

        self.send_command_noreply(GpuVirtioMsg { transfer_to_host_2d: VirtioGpuTransferToHost2d {
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
        }}).unwrap();

        self.send_command_noreply(GpuVirtioMsg { resource_flush: VirtioGpuResourceFlush {
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
        }}).unwrap();
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
#[allow(non_camel_case_types, dead_code)]
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

