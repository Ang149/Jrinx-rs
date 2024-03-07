#![no_std]
#![feature(new_uninit)]
extern crate alloc;
mod smoltcp_impl;

pub use self::smoltcp_impl::TcpSocket;
use alloc::sync::Arc;
use jrinx_driver::net::virtio_net::VirtIoNetMutex;
use jrinx_driver::Driver;
use log::info;

pub fn init_network(net_dev: Arc<VirtIoNetMutex>) {
    info!("Initialize network subsystem...");
    info!("  use NIC 0: {:?}", net_dev.name());
    smoltcp_impl::init(net_dev);
}
