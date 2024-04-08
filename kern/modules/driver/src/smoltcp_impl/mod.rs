mod addr;
mod listen_table;
pub mod tcp;
use self::listen_table::ListenTable;
use crate::net::net_buf::NetBufPtr;
use crate::net::virtio_net::VirtIoNetMutex;
use crate::smoltcp_impl::tcp::TcpSocket;
use crate::VirtioNet;
use alloc::sync::Arc;
use alloc::vec;
use core::cell::RefCell;
use core::net::{Ipv4Addr, SocketAddr};
use core::ops::DerefMut;
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::socket::{self, AnySocket};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr};
use spin::{Mutex, Once};
const IP: &str = "10.0.2.15";
const GATEWAY: &str = "10.0.2.2";

const IP_PREFIX: u8 = 24;

const RANDOM_SEED: u64 = 0xA2CE_05A2_CE05_A2CE;

const TCP_RX_BUF_LEN: usize = 64 * 1024;
const TCP_TX_BUF_LEN: usize = 64 * 1024;
const LISTEN_QUEUE_SIZE: usize = 512;
/// Number of nanoseconds in a microsecond.
pub const NANOS_PER_MICROS: u64 = 1_000;
pub(crate) static LISTEN_TABLE: Once<ListenTable> = Once::new();
pub(crate) static SOCKET_SET: Once<SocketSetWrapper> = Once::new();
pub(crate) static ETH0: Once<InterfaceWrapper> = Once::new();

pub(crate) struct SocketSetWrapper<'a>(Mutex<SocketSet<'a>>);
pub struct InterfaceWrapper {
    name: &'static str,
    ether_addr: EthernetAddress,
    dev: Mutex<DeviceWrapper>,
    pub(crate) iface: Mutex<Interface>,
}
struct DeviceWrapper {
    inner: RefCell<Arc<VirtIoNetMutex>>, // use `RefCell` is enough since it's wrapped in `Mutex` in `InterfaceWrapper`.
}

impl InterfaceWrapper {
    fn new(name: &'static str, dev: Arc<VirtIoNetMutex>, ether_addr: EthernetAddress) -> Self {
        let mut config = Config::new(HardwareAddress::Ethernet(ether_addr));
        config.random_seed = RANDOM_SEED;

        let mut dev = DeviceWrapper::new(dev);
        let iface = Mutex::new(Interface::new(config, &mut dev, Self::current_time()));
        Self {
            name,
            ether_addr,
            dev: Mutex::new(dev),
            iface,
        }
    }

    fn current_time() -> Instant {
        Instant::from_micros_const((0u64 / NANOS_PER_MICROS) as i64)
    }

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn ethernet_address(&self) -> EthernetAddress {
        self.ether_addr
    }

    pub fn setup_ip_addr(&self, ip: IpAddress, prefix_len: u8) {
        let mut iface = self.iface.lock();
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs.push(IpCidr::new(ip, prefix_len)).unwrap();
        });
    }

    pub fn setup_gateway(&self, gateway: IpAddress) {
        let mut iface = self.iface.lock();
        match gateway {
            IpAddress::Ipv4(v4) => iface.routes_mut().add_default_ipv4_route(v4).unwrap(),
        };
    }

    pub fn poll(&self, sockets: &Mutex<SocketSet>) {
        let mut dev = self.dev.lock();
        let mut iface = self.iface.lock();
        let mut sockets = sockets.lock();
        let timestamp = Self::current_time();
        iface.poll(timestamp, dev.deref_mut(), &mut sockets);
    }
}
impl DeviceWrapper {
    fn new(inner: Arc<VirtIoNetMutex>) -> Self {
        Self {
            inner: RefCell::new(inner),
        }
    }
}
impl Device for DeviceWrapper {
    type RxToken<'a> = NetRxToken<'a> where Self: 'a;
    type TxToken<'a> = NetTxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let dev = self.inner.borrow_mut();
        if let Err(e) = dev.inner.lock().recycle_tx_buffers() {
            warn!("recycle_tx_buffers failed: {:?}", e);
            return None;
        }

        if !dev.inner.lock().can_transmit() {
            return None;
        }
        let rx_buf = match dev.inner.lock().receive() {
            Ok(buf) => buf,
            Err(_err) => {
                //warn!("receive failed: {:?}", err);
                return None;
            }
        };
        Some((NetRxToken(&self.inner, rx_buf), NetTxToken(&self.inner)))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        let dev = self.inner.borrow_mut();
        if let Err(e) = dev.inner.lock().recycle_tx_buffers() {
            warn!("recycle_tx_buffers failed: {:?}", e);
            return None;
        }
        if dev.inner.lock().can_transmit() {
            Some(NetTxToken(&self.inner))
        } else {
            None
        }
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1514;
        caps.max_burst_size = None;
        caps.medium = Medium::Ethernet;
        caps
    }
}
struct NetRxToken<'a>(&'a RefCell<Arc<VirtIoNetMutex>>, NetBufPtr);
struct NetTxToken<'a>(&'a RefCell<Arc<VirtIoNetMutex>>);

impl<'a> RxToken for NetRxToken<'a> {
    fn preprocess(&self, sockets: &mut SocketSet<'_>) {
        snoop_tcp_packet(self.1.packet(), sockets).ok();
    }

    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut rx_buf = self.1;
        info!(
            "RECV {} bytes: {:02X?}",
            rx_buf.packet_len(),
            rx_buf.packet()
        );
        let result = f(rx_buf.packet_mut());
        self.0
            .borrow_mut()
            .inner
            .lock()
            .recycle_rx_buffer(rx_buf)
            .unwrap();
        result
    }
}

impl<'a> TxToken for NetTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let dev = self.0.borrow_mut();
        let mut tx_buf = dev.inner.lock().alloc_tx_buffer(len).unwrap();
        let ret = f(tx_buf.packet_mut());
        debug!("SEND {} bytes: {:02X?}", len, tx_buf.packet());
        dev.inner.lock().transmit(tx_buf).unwrap();
        ret
    }
}

fn snoop_tcp_packet(buf: &[u8], sockets: &mut SocketSet<'_>) -> Result<(), smoltcp::wire::Error> {
    use smoltcp::wire::{EthernetFrame, IpProtocol, Ipv4Packet, TcpPacket};

    let ether_frame = EthernetFrame::new_checked(buf)?;
    let ipv4_packet = Ipv4Packet::new_checked(ether_frame.payload())?;

    if ipv4_packet.next_header() == IpProtocol::Tcp {
        let tcp_packet = TcpPacket::new_checked(ipv4_packet.payload())?;
        let src_addr = (ipv4_packet.src_addr(), tcp_packet.src_port()).into();
        let dst_addr = (ipv4_packet.dst_addr(), tcp_packet.dst_port()).into();
        let is_first = tcp_packet.syn() && !tcp_packet.ack();
        if is_first {
            // create a socket for the first incoming TCP packet, as the later accept() returns.
            LISTEN_TABLE
                .get()
                .unwrap()
                .incoming_tcp_packet(src_addr, dst_addr, sockets);
        }
    }
    Ok(())
}
impl<'a> SocketSetWrapper<'a> {
    fn new() -> Self {
        Self(Mutex::new(SocketSet::new(vec![])))
    }

    pub fn new_tcp_socket() -> socket::tcp::Socket<'a> {
        let tcp_rx_buffer = socket::tcp::SocketBuffer::new(vec![0; TCP_RX_BUF_LEN]);
        let tcp_tx_buffer = socket::tcp::SocketBuffer::new(vec![0; TCP_TX_BUF_LEN]);
        socket::tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
    }

    pub fn add<T: AnySocket<'a>>(&self, socket: T) -> SocketHandle {
        let handle = self.0.lock().add(socket);
        info!("socket {}: created", handle);
        handle
    }

    pub fn with_socket<T: AnySocket<'a>, R, F>(&self, handle: SocketHandle, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let set = self.0.lock();
        let socket = set.get(handle);
        f(socket)
    }

    pub fn with_socket_mut<T: AnySocket<'a>, R, F>(&self, handle: SocketHandle, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut set = self.0.lock();
        let socket = set.get_mut(handle);
        f(socket)
    }

    pub fn poll_interfaces(&self) {
        ETH0.get().unwrap().poll(&self.0);
    }

    pub fn remove(&self, handle: SocketHandle) {
        self.0.lock().remove(handle);
        info!("socket {}: destroyed", handle);
    }
}
pub(crate) fn init(net_dev: Arc<VirtIoNetMutex>) {
    let ether_addr = EthernetAddress(net_dev.inner.lock().mac_address().0);
    let eth0 = InterfaceWrapper::new("eth0", net_dev, ether_addr);

    let ip = IP.parse().expect("invalid IP address");
    let gateway = GATEWAY.parse().expect("invalid gateway IP address");
    eth0.setup_ip_addr(ip, IP_PREFIX);
    eth0.setup_gateway(gateway);

    ETH0.call_once(|| eth0);
    SOCKET_SET.call_once(SocketSetWrapper::new);
    LISTEN_TABLE.call_once(ListenTable::new);

    // info!("created net interface {:?}:", ETH0.get().unwrap().name());
    // info!("  ether:    {}", ETH0.get().unwrap().ethernet_address());
    // info!("  ip:       {}/{}", ip, IP_PREFIX);
    // info!("  gateway:  {}", gateway);
    // loop {
    //     net_test()
    // }
}
const LOCAL_PORT: u16 = 5555;
const CONTENT: &str = "hello jrinx";

pub fn net_test() {
    let tcp_socket = TcpSocket::new();
    tcp_socket
        .bind(SocketAddr::new(
            core::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            LOCAL_PORT,
        ))
        .unwrap();
    tcp_socket.listen().unwrap();
    info!("listen on:http://{}/", tcp_socket.local_addr().unwrap());
    let new_socket = tcp_socket.accept().unwrap();
    let addr = new_socket.peer_addr().unwrap();
    info!("addr is {}", addr);
    new_socket.send(CONTENT.as_bytes()).unwrap();
}
