use alloc::{boxed::Box, collections::VecDeque};
use core::matches;
use core::ops::{Deref, DerefMut};
use jrinx_error::{InternalError, Result};
use log::{info, warn};
use smoltcp::iface::{SocketHandle, SocketSet};
use smoltcp::socket::tcp::{self, State};
use smoltcp::wire::{IpAddress, IpEndpoint, IpListenEndpoint};
use spin::Mutex;

use crate::smoltcp_impl::{SocketSetWrapper, LISTEN_QUEUE_SIZE, SOCKET_SET};

const PORT_NUM: usize = 65536;

struct ListenTableEntry {
    listen_endpoint: IpListenEndpoint,
    syn_queue: VecDeque<SocketHandle>,
}

impl ListenTableEntry {
    pub fn new(listen_endpoint: IpListenEndpoint) -> Self {
        Self {
            listen_endpoint,
            syn_queue: VecDeque::with_capacity(LISTEN_QUEUE_SIZE),
        }
    }

    #[inline]
    fn can_accept(&self, dst: IpAddress) -> bool {
        match self.listen_endpoint.addr {
            Some(addr) => addr == dst,
            None => true,
        }
    }
}

impl Drop for ListenTableEntry {
    fn drop(&mut self) {
        for &handle in &self.syn_queue {
            SOCKET_SET.get().unwrap().remove(handle);
        }
    }
}

pub struct ListenTable {
    tcp: Box<[Mutex<Option<Box<ListenTableEntry>>>]>,
}

impl ListenTable {
    pub fn new() -> Self {
        let tcp = unsafe {
            let mut buf = Box::new_uninit_slice(PORT_NUM);
            for i in 0..PORT_NUM {
                buf[i].write(Mutex::new(None));
            }
            buf.assume_init()
        };
        Self { tcp }
    }

    pub fn can_listen(&self, port: u16) -> bool {
        self.tcp[port as usize].lock().is_none()
    }

    pub fn listen(&self, listen_endpoint: IpListenEndpoint) -> Result<()> {
        let port = listen_endpoint.port;
        assert_ne!(port, 0);
        let mut entry = self.tcp[port as usize].lock();
        if entry.is_none() {
            *entry = Some(Box::new(ListenTableEntry::new(listen_endpoint)));
            Ok(())
        } else {
            info!("net listen fail");
            Err(InternalError::NetFail)
        }
    }

    pub fn unlisten(&self, port: u16) {
        info!("TCP socket unlisten on {}", port);
        *self.tcp[port as usize].lock() = None;
    }

    pub fn can_accept(&self, port: u16) -> Result<bool> {
        if let Some(entry) = self.tcp[port as usize].lock().deref() {
            Ok(entry.syn_queue.iter().any(|&handle| is_connected(handle)))
        } else {
            info!("socket accept() failed: not listen");
            Err(InternalError::NetFail)
        }
    }

    pub fn accept(&self, port: u16) -> Result<(SocketHandle, (IpEndpoint, IpEndpoint))> {
        if let Some(entry) = self.tcp[port as usize].lock().deref_mut() {
            let syn_queue = &mut entry.syn_queue;
            let (idx, addr_tuple) = syn_queue
                .iter()
                .enumerate()
                .find_map(|(idx, &handle)| {
                    is_connected(handle).then(|| (idx, get_addr_tuple(handle)))
                })
                .ok_or(InternalError::WouldBlock)?; // wait for connection
            if idx > 0 {
                warn!(
                    "slow SYN queue enumeration: index = {}, len = {}!",
                    idx,
                    syn_queue.len()
                );
            }
            let handle = syn_queue.swap_remove_front(idx).unwrap();
            Ok((handle, addr_tuple))
        } else {
            info!("socket accept() failed: not listen");
            Err(InternalError::NetFail)
        }
    }

    pub fn incoming_tcp_packet(
        &self,
        src: IpEndpoint,
        dst: IpEndpoint,
        sockets: &mut SocketSet<'_>,
    ) {
        if let Some(entry) = self.tcp[dst.port as usize].lock().deref_mut() {
            if !entry.can_accept(dst.addr) {
                // not listening on this address
                warn!("not listening on this address");
                return;
            }
            if entry.syn_queue.len() >= LISTEN_QUEUE_SIZE {
                // SYN queue is full, drop the packet
                warn!("SYN queue overflow!");
                return;
            }
            let mut socket = SocketSetWrapper::new_tcp_socket();
            if socket.listen(entry.listen_endpoint).is_ok() {
                let handle = sockets.add(socket);
                info!(
                    "TCP socket {}: prepare for connection {} -> {}",
                    handle, src, entry.listen_endpoint
                );
                entry.syn_queue.push_back(handle);
            }
        }
    }
}

fn is_connected(handle: SocketHandle) -> bool {
    SOCKET_SET
        .get()
        .unwrap()
        .with_socket::<tcp::Socket, _, _>(handle, |socket| {
            !matches!(socket.state(), State::Listen | State::SynReceived)
        })
}

fn get_addr_tuple(handle: SocketHandle) -> (IpEndpoint, IpEndpoint) {
    SOCKET_SET
        .get()
        .unwrap()
        .with_socket::<tcp::Socket, _, _>(handle, |socket| {
            (
                socket.local_endpoint().unwrap(),
                socket.remote_endpoint().unwrap(),
            )
        })
}
