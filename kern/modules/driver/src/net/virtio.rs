use core::ptr::NonNull;

use fdt::node::FdtNode;
use jrinx_addr::{PhysAddr, VirtAddr};
use jrinx_config::{EXTERNAL_DEVICE_REGION, PAGE_SIZE};
use jrinx_devprober::devprober;
use jrinx_error::{InternalError,Result};
use jrinx_hal::{hal, Hal as _, Vm};
use jrinx_paging::boot::BootPageTable;
use virtio_drivers::{
    device::{blk::VirtIOBlk, gpu::VirtIOGpu, input::VirtIOInput, net::VirtIONetRaw},
    transport::{
        mmio::{MmioTransport, VirtIOHeader},
        DeviceType, Transport,
    },
    Hal,
};

use crate::{bus::virtio::VirtioHal, irq, net::net::VirtIoNetDev};

#[devprober(compatible = "virtio,mmio")]
fn probe(node: &FdtNode) -> Result<()> {
    let region = node
        .reg()
        .ok_or(InternalError::DevProbeError)?
        .next()
        .ok_or(InternalError::DevProbeError)?;
    let paddr = region.starting_address as usize;
    let vaddr = paddr + EXTERNAL_DEVICE_REGION.addr;
    let size = region.size.ok_or(InternalError::DevProbeError)?;
    let count = size / PAGE_SIZE;
    let interrupt_parent = node
    .interrupt_parent()
    .ok_or(InternalError::DevProbeError)?
    .property("phandle")
    .ok_or(InternalError::DevProbeError)?
    .as_usize()
    .unwrap();
    let irq_num = node
    .interrupts()
    .ok_or(InternalError::DevProbeError)?
    .next()
    .ok_or(InternalError::DevProbeError)?;
    info!("vaddr is {:x?}, count is {:x?}",vaddr,count);
    unsafe {
        BootPageTable.map(
            VirtAddr::new(vaddr),
            PhysAddr::new(paddr),
        );
    }
    hal!().vm().sync_all();
    let header = NonNull::new(vaddr as *mut VirtIOHeader).unwrap();
    match unsafe { MmioTransport::new(header) } {
        Err(e) => warn!("Error creating VirtIO MMIO transport: {}", e),
        Ok(transport) => {
            info!(
                "Detected virtio MMIO device with vendor id {:#X}, device type {:?}, version {:?}",
                transport.vendor_id(),
                transport.device_type(),
                transport.version(),
            );
            let dev = virtio_device(transport,interrupt_parent,irq_num);
            
        }
    }

    Ok(())
}
fn virtio_device(transport: MmioTransport,interrupt_parent:usize,irq_num:usize) {
    info!("virtio type is {:?}",transport.device_type());
    match transport.device_type() {
        DeviceType::Block => {},
        DeviceType::GPU => {},
        DeviceType::Input => {},
        DeviceType::Network => VirtIoNetDev::<VirtioHal, MmioTransport, 64>::new(transport,interrupt_parent,irq_num).unwrap(),
        t => warn!("Unrecognized virtio device: {:?}",t),
    }
}