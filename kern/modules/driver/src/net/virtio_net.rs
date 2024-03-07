use core::{any::Any, ptr::NonNull};

use super::net_buf::NetBufPtr;
use crate::bus::{self, virtio::VirtioHal};
use crate::irq::riscv_intc::IRQ_TABLE;
use crate::net::net_buf::{NetBuf, NetBufPool};
use crate::{Driver, EthernetAddress, UPIntrFreeCell, VirtioNet};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use jrinx_error::{InternalError, Result};
use spin::mutex::Mutex;
use virtio_drivers::{
    device::{blk::VirtIOBlk, gpu::VirtIOGpu, input::VirtIOInput, net::VirtIONetRaw},
    transport::{
        mmio::{MmioTransport, VirtIOHeader},
        DeviceType, Transport,
    },
    Hal,
};
const NET_BUF_LEN: usize = 1526;
//QS is virtio queue size
const QS: usize = 64;
pub struct VirtIoNetMutex {
    pub inner: Mutex<VirtIoNetInner>,
}
pub struct VirtIoNetInner {
    rx_buffers: [Option<Box<NetBuf>>; QS],
    tx_buffers: [Option<Box<NetBuf>>; QS],
    free_tx_bufs: Vec<Box<NetBuf>>,
    buf_pool: Arc<NetBufPool>,
    raw: VirtIONetRaw<VirtioHal, MmioTransport, QS>,
}
unsafe impl Send for VirtIoNetInner {}
unsafe impl Sync for VirtIoNetInner {}
impl VirtIoNetInner {
    pub fn new(transport: MmioTransport) -> Result<VirtIoNetInner> {
        let mut inner = VirtIONetRaw::new(transport).unwrap();
        const NONE_BUF: Option<Box<NetBuf>> = None;
        let rx_buffers = [NONE_BUF; QS];
        let tx_buffers = [NONE_BUF; QS];
        let buf_pool = NetBufPool::new(2 * QS, NET_BUF_LEN).unwrap();
        let free_tx_bufs = Vec::with_capacity(QS);
        info!("mac is {:?}", inner.mac_address());
        let mut dev = Self {
            rx_buffers,
            raw: inner,
            tx_buffers,
            free_tx_bufs,
            buf_pool,
        };
        // 1. Fill all rx buffers.
        for (i, rx_buf_place) in dev.rx_buffers.iter_mut().enumerate() {
            let mut rx_buf = dev
                .buf_pool
                .alloc_boxed()
                .ok_or(InternalError::NotEnoughMem)?;
            // Safe because the buffer lives as long as the queue.
            let token = unsafe { dev.raw.receive_begin(rx_buf.raw_buf_mut()).unwrap() };
            assert_eq!(token, i as u16);
            *rx_buf_place = Some(rx_buf);
        }

        // 2. Allocate all tx buffers.use fdt::node::FdtNode;
        for _ in 0..QS {
            let mut tx_buf = dev
                .buf_pool
                .alloc_boxed()
                .ok_or(InternalError::NotEnoughMem)?;
            // Fill header
            let hdr_len = dev
                .raw
                .fill_buffer_header(tx_buf.raw_buf_mut())
                .or(Err(InternalError::InvalidParam))?;
            tx_buf.set_header_len(hdr_len);
            dev.free_tx_bufs.push(tx_buf);
        }
        Ok(dev)
    }
}

impl VirtIoNetMutex {
    pub fn new(net_dev: VirtIoNetInner) -> Self{
        // IRQ_TABLE
        //     .write()
        //     .get(&interrupt_parent)
        //     .unwrap()
        //     .lock()
        //     .register_device(irq_num, Arc::new(dev))
        //     .unwrap();
        Self {
            inner: Mutex::new(net_dev),
        }
    }
}
impl Driver for VirtIoNetMutex {
    fn name(&self) -> &str {
        "virtio-net"
    }

    fn handle_irq(&self, irq_num: usize) {
        let rx_buf = match self.inner.lock().receive() {
            Ok(buf) => {
                info!("received packet len is {}", buf.packet_len());
            }
            Err(err) => {
                if !matches!(err, InternalError::NetAgain) {
                    warn!("receive failed: {:?}", err);
                }
            }
        };
        self.inner.lock().raw.ack_interrupt();
    }
}
impl VirtioNet for VirtIoNetInner {

    fn mac_address(&self) -> EthernetAddress {
        EthernetAddress(self.raw.mac_address())
    }


    fn can_transmit(&self) -> bool {
        !self.free_tx_bufs.is_empty() && self.raw.can_send()
    }


    fn can_receive(&self) -> bool {
        self.raw.poll_receive().is_some()
    }


    fn rx_queue_size(&self) -> usize {
        QS
    }

    fn tx_queue_size(&self) -> usize {
        QS
    }

    fn recycle_rx_buffer(&mut self, rx_buf: NetBufPtr) -> Result<()> {
        let mut rx_buf = unsafe { NetBuf::from_buf_ptr(rx_buf) };
        // Safe because we take the ownership of `rx_buf` back to `rx_buffers`,
        // it lives as long as the queue.
        let new_token = unsafe { self.raw.receive_begin(rx_buf.raw_buf_mut()).unwrap() };
        // `rx_buffers[new_token]` is expected to be `None` since it was taken
        // away at `Self::receive()` and has not been added back.
        if self.rx_buffers[new_token as usize].is_some() {
            return Err(InternalError::DevBadState);
        }
        self.rx_buffers[new_token as usize] = Some(rx_buf);
        Ok(())
    }

    fn recycle_tx_buffers(&mut self) -> Result<()> {
        while let Some(token) = self.raw.poll_transmit() {
            let tx_buf = self.tx_buffers[token as usize]
                .take()
                .ok_or(InternalError::DevBadState)?;
            unsafe {
                self.raw
                    .transmit_complete(token, tx_buf.packet_with_header())
                    .unwrap();
            }
            // Recycle the buffer.
            self.free_tx_bufs.push(tx_buf);
        }
        Ok(())
    }

    fn transmit(&mut self, tx_buf: NetBufPtr) -> Result<()> {
        // 0. prepare tx buffer.
        let tx_buf = unsafe { NetBuf::from_buf_ptr(tx_buf) };
        // 1. transmit packet.
        let token = unsafe {
            self.raw
                .transmit_begin(tx_buf.packet_with_header())
                .unwrap()
        };
        self.tx_buffers[token as usize] = Some(tx_buf);
        Ok(())
    }

    fn receive(&mut self) -> Result<NetBufPtr> {
        if let Some(token) = self.raw.poll_receive() {
            let mut rx_buf = self.rx_buffers[token as usize]
                .take()
                .ok_or(InternalError::DevBadState)?;
            // Safe because the buffer lives as long as the queue.
            let (hdr_len, pkt_len) = unsafe {
                self.raw
                    .receive_complete(token, rx_buf.raw_buf_mut())
                    .unwrap()
            };
            rx_buf.set_header_len(hdr_len);
            rx_buf.set_packet_len(pkt_len);

            Ok(rx_buf.into_buf_ptr())
        } else {
            Err(InternalError::NetAgain)
        }
    }

    fn alloc_tx_buffer(&mut self, size: usize) -> Result<NetBufPtr> {
        // 0. Allocate a buffer from the queue.
        let mut net_buf = self.free_tx_bufs.pop().ok_or(InternalError::NotEnoughMem)?;
        let pkt_len = size;

        // 1. Check if the buffer is large enough.
        let hdr_len = net_buf.header_len();
        if hdr_len + pkt_len > net_buf.capacity() {
            return Err(InternalError::InvalidParam);
        }
        net_buf.set_packet_len(pkt_len);

        // 2. Return the buffer.
        Ok(net_buf.into_buf_ptr())
    }
}
