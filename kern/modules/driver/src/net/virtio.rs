use core::{
    net::{Ipv4Addr, SocketAddr},
    ptr::NonNull,
};

use alloc::sync::Arc;
use fdt::node::FdtNode;
use jrinx_addr::{PhysAddr, VirtAddr};
use jrinx_config::{EXTERNAL_DEVICE_REGION, PAGE_SIZE};
use jrinx_devprober::devprober;
use jrinx_error::{InternalError, Result};
use jrinx_hal::{hal, Hal as _, Vm};
use jrinx_paging::boot::BootPageTable;
use spin::{Mutex, Once};
use virtio_drivers::transport::{
    mmio::{MmioTransport, VirtIOHeader},
    DeviceType, Transport,
};

use crate::{
    irq::riscv_intc::IRQ_TABLE,
    net::virtio_net::{VirtIoNetInner, VirtIoNetMutex},
    smoltcp_impl::{init, tcp::TcpSocket},
};

//#[devprober(compatible = "virtio,mmio")]
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
    info!("vaddr is {:x?}, count is {:x?}", vaddr, count);
    unsafe {
        BootPageTable.map(VirtAddr::new(vaddr), PhysAddr::new(paddr));
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
            virtio_device(transport, interrupt_parent, irq_num);
        }
    }

    Ok(())
}
pub(crate) static tcp_once: Once<(TcpSocket, Mutex<bool>)> = Once::new();
const LOCAL_PORT: u16 = 5555;
fn virtio_device(transport: MmioTransport, interrupt_parent: usize, irq_num: usize) {
    //info!("virtio type is {:?}", transport.device_type());
    match transport.device_type() {
        DeviceType::Block => {}
        DeviceType::GPU => {}
        DeviceType::Input => {}
        DeviceType::Network => {
            let dev = Arc::new(VirtIoNetMutex::new(VirtIoNetInner::new(transport).unwrap()));
            IRQ_TABLE
                .write()
                .get(&interrupt_parent)
                .unwrap()
                .lock()
                .register_device(irq_num, dev.clone())
                .unwrap();
            init(dev.clone());
            // let tcp_socket = TcpSocket::new();
            // tcp_socket
            //     .bind(SocketAddr::new(
            //         core::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            //         LOCAL_PORT,
            //     ))
            //     .unwrap();
            // tcp_socket.listen().unwrap();
            // info!("listen on:http://{}/", tcp_socket.local_addr().unwrap());
            // info!("create {:?}",tcp_socket.local_addr());
            //tcp_once.call_once(|| (tcp_socket, Mutex::new(false)));
            VIRTIO_DEVICE.call_once(|| dev.clone());
        }
        t => warn!("Unrecognized virtio device: {:?}", t),
    }
}
pub static VIRTIO_DEVICE: Once<Arc<VirtIoNetMutex>> = Once::new();
