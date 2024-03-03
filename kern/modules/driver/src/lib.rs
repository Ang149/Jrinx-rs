#![no_std]
#![feature(used_with_arg)]
#![allow(unused)]
#![feature(allocator_api)]
extern crate alloc;

#[macro_use]
extern crate log;

pub mod bus;
pub mod io;
pub mod irq;
mod mem;
pub mod net;
pub mod uart;
use core::cell::RefCell;

use alloc::{boxed::Box, sync::Arc};
use fdt::Fdt;
use jrinx_error::Result;
use jrinx_hal::{hal, Hal};
use net::net_buf::NetBufPtr;

pub struct UPIntrFreeCell<T> {
    /// inner data
    inner: RefCell<T>,
}

pub fn probe_all(fdt: &Fdt<'_>) {
    info!("probing all devices");
    jrinx_devprober::probe_all_device(fdt).unwrap();
}

pub type InterruptHandler = Box<dyn Fn() + Send + Sync>;

pub trait Driver: Send + Sync {
    fn name(&self) -> &str;

    fn handle_irq(&self, irq_num: usize);
}

pub trait InterruptController: Driver {
    fn enable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()>;
    fn disable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()>;
    fn register_device(&self, irq_num: usize, dev: Arc<dyn Driver>) -> Result<()>;
    fn info(&self);
}

pub trait Uart: Driver {
    fn init(&self) -> Result<()>;
    fn read(&self) -> Result<u8>;
    fn write(&self, data: u8) -> Result<()>;
    fn write_str(&self, data: &str) -> Result<()>;
}
pub struct EthernetAddress(pub [u8; 6]);
pub trait VirtioNet: Driver {
    /// The ethernet address of the NIC.
    fn mac_address(&self) -> EthernetAddress;

    /// Whether can transmit packets.
    fn can_transmit(&self) -> bool;

    /// Whether can receive packets.
    fn can_receive(&self) -> bool;

    /// Size of the receive queue.
    fn rx_queue_size(&self) -> usize;

    /// Size of the transmit queue.
    fn tx_queue_size(&self) -> usize;

    /// Gives back the `rx_buf` to the receive queue for later receiving.
    ///
    /// `rx_buf` should be the same as the one returned by
    /// [`NetDriverOps::receive`].
    fn recycle_rx_buffer(&mut self, rx_buf: NetBufPtr) -> Result<()>;

    /// Poll the transmit queue and gives back the buffers for previous transmiting.
    /// returns [`DevResult`].
    fn recycle_tx_buffers(&mut self) -> Result<()>;

    /// Transmits a packet in the buffer to the network, without blocking,
    /// returns [`DevResult`].
    fn transmit(&mut self, tx_buf: NetBufPtr) -> Result<()>;

    /// Receives a packet from the network and store it in the [`NetBuf`],
    /// returns the buffer.
    ///
    /// Before receiving, the driver should have already populated some buffers
    /// in the receive queue by [`NetDriverOps::recycle_rx_buffer`].
    ///
    /// If currently no incomming packets, returns an error with type
    /// [`DevError::Again`].
    fn receive(&mut self) -> Result<NetBufPtr>;

    /// Allocate a memory buffer of a specified size for network transmission,
    /// returns [`DevResult`]
    fn alloc_tx_buffer(&mut self, size: usize) -> Result<NetBufPtr>;
}