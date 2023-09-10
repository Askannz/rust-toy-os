use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use crate::{serial_print, serial_println, VIRTIO_ACK_INPUT, VIRTIO_ACK_GPU};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {

        let mut idt = InterruptDescriptorTable::new();

        idt[InterruptIndex::Pci_Input.as_usize()]
            .set_handler_fn(pci_input_interrupt_handler);

        idt[InterruptIndex::Pci_GPU.as_usize()]
            .set_handler_fn(pci_gpu_interrupt_handler);

        idt
    };
}

extern "x86-interrupt" fn pci_input_interrupt_handler(
    _stack_frame: InterruptStackFrame
) {

    use x86_64::instructions::interrupts::without_interrupts;

    without_interrupts(|| {
        if let Some(ack_obj) = VIRTIO_ACK_INPUT.lock().as_mut() {
            ack_obj.ack_interrupt();
        }
    });

    unsafe {

        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Pci_Input.as_u8());
    }
}

extern "x86-interrupt" fn pci_gpu_interrupt_handler(
    _stack_frame: InterruptStackFrame
) {

    use x86_64::instructions::interrupts::without_interrupts;

    serial_print!("PCI interrupt");

    without_interrupts(|| {
        if let Some(ack_obj) = VIRTIO_ACK_GPU.lock().as_mut() {
            ack_obj.ack_interrupt();
        }
    });

    unsafe {

        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Pci_GPU.as_u8());
    }
}

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> = 
    spin::Mutex::new(unsafe {
        ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)
    });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
    Pci_Input = PIC_2_OFFSET + 3,
    Pci_GPU = PIC_2_OFFSET + 2
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}
