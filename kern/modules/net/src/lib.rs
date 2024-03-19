#![no_std]
#![feature(new_uninit)]
extern crate alloc;
mod smoltcp_impl;

use core::net::{Ipv4Addr, SocketAddr};

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
const LOCAL_PORT: u16 = 5555;
const CONTENT: &str = "hello jrinx";

pub fn net_test() {
    // let tcp_socket = TcpSocket::new();
    // tcp_socket
    //     .bind(SocketAddr::new(
    //         core::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
    //         LOCAL_PORT,
    //     ))
    //     .unwrap();
    // tcp_socket.listen().unwrap();
    // info!("listen on:http://{}/", tcp_socket.local_addr().unwrap());
    // let new_socket = tcp_socket.accept().unwrap();
    // let addr = new_socket.peer_addr().unwrap();
    // info!("addr is {}", addr);
    // new_socket.send(CONTENT.as_bytes()).unwrap();
}
