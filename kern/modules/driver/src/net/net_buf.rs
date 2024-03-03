use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};
use core::ptr::NonNull;
use jrinx_error::{InternalError, Result};
use spin::Mutex;

const MIN_BUFFER_LEN: usize = 1526;
const MAX_BUFFER_LEN: usize = 65535;
pub struct NetBuf {
    header_len: usize,
    packet_len: usize,
    capacity: usize,
    buf_ptr: NonNull<u8>,
    pool_offset: usize,
    pool: Arc<NetBufPool>,
}
impl NetBuf {
    unsafe fn get_slice(&self, start: usize, len: usize) -> &[u8] {
        core::slice::from_raw_parts(self.buf_ptr.as_ptr().add(start), len)
    }

    unsafe fn get_slice_mut(&mut self, start: usize, len: usize) -> &mut [u8] {
        core::slice::from_raw_parts_mut(self.buf_ptr.as_ptr().add(start), len)
    }

    /// Returns the capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the length of the header part.
    pub fn header_len(&self) -> usize {
        self.header_len
    }

    /// Returns the header part of the buffer.
    pub fn header(&self) -> &[u8] {
        unsafe { self.get_slice(0, self.header_len) }
    }

    /// Returns the packet part of the buffer.
    pub fn packet(&self) -> &[u8] {
        unsafe { self.get_slice(self.header_len, self.packet_len) }
    }

    /// Returns the mutable reference to the packet part.
    pub fn packet_mut(&mut self) -> &mut [u8] {
        unsafe { self.get_slice_mut(self.header_len, self.packet_len) }
    }

    /// Returns both the header and the packet parts, as a contiguous slice.
    pub fn packet_with_header(&self) -> &[u8] {
        unsafe { self.get_slice(0, self.header_len + self.packet_len) }
    }

    /// Returns the entire buffer.
    pub fn raw_buf(&self) -> &[u8] {
        unsafe { self.get_slice(0, self.capacity) }
    }

    /// Returns the mutable reference to the entire buffer.
    pub fn raw_buf_mut(&mut self) -> &mut [u8] {
        unsafe { self.get_slice_mut(0, self.capacity) }
    }

    /// Set the length of the header part.
    pub fn set_header_len(&mut self, header_len: usize) {
        debug_assert!(header_len + self.packet_len <= self.capacity);
        self.header_len = header_len;
    }

    /// Set the length of the packet part.
    pub fn set_packet_len(&mut self, packet_len: usize) {
        debug_assert!(self.header_len + packet_len <= self.capacity);
        self.packet_len = packet_len;
    }

    /// Converts the buffer into a [`NetBufPtr`].
    pub fn into_buf_ptr(mut self: Box<Self>) -> NetBufPtr {
        let buf_ptr = self.packet_mut().as_mut_ptr();
        let len = self.packet_len;
        NetBufPtr::new(
            NonNull::new(Box::into_raw(self) as *mut u8).unwrap(),
            NonNull::new(buf_ptr).unwrap(),
            len,
        )
    }

    /// Restore [`NetBuf`] struct from a raw pointer.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it may cause some memory issues,
    /// so we must ensure that it is called after calling `into_buf_ptr`.
    pub unsafe fn from_buf_ptr(ptr: NetBufPtr) -> Box<Self> {
        Box::from_raw(ptr.raw_ptr::<Self>())
    }
}

impl Drop for NetBuf {
    /// Deallocates the buffer into the [`NetBufPool`].
    fn drop(&mut self) {
        self.pool.dealloc(self.pool_offset);
    }
}
pub struct NetBufPool {
    capacity: usize,
    buf_len: usize,
    pool: Vec<u8>,
    free_list: Mutex<Vec<usize>>,
}

impl NetBufPool {
    /// Creates a new pool with the given `capacity`, and all buffer lengths are
    /// set to `buf_len`.
    pub fn new(capacity: usize, buf_len: usize) -> Result<Arc<Self>> {
        if capacity == 0 {
            return Err(InternalError::InvalidParam);
        }
        if !(MIN_BUFFER_LEN..=MAX_BUFFER_LEN).contains(&buf_len) {
            return Err(InternalError::InvalidParam);
        }

        let pool = vec![0; capacity * buf_len];
        let mut free_list = Vec::with_capacity(capacity);
        for i in 0..capacity {
            free_list.push(i * buf_len);
        }
        Ok(Arc::new(Self {
            capacity,
            buf_len,
            pool,
            free_list: Mutex::new(free_list),
        }))
    }

    /// Returns the capacity of the pool.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the length of each buffer.
    pub const fn buffer_len(&self) -> usize {
        self.buf_len
    }

    /// Allocates a buffer from the pool.
    ///
    /// Returns `None` if no buffer is available.
    pub fn alloc(self: &Arc<Self>) -> Option<NetBuf> {
        let pool_offset = self.free_list.lock().pop()?;
        let buf_ptr =
            unsafe { NonNull::new(self.pool.as_ptr().add(pool_offset) as *mut u8).unwrap() };
        Some(NetBuf {
            header_len: 0,
            packet_len: 0,
            capacity: self.buf_len,
            buf_ptr,
            pool_offset,
            pool: Arc::clone(self),
        })
    }

    /// Allocates a buffer wrapped in a [`Box`] from the pool.
    ///
    /// Returns `None` if no buffer is available.
    pub fn alloc_boxed(self: &Arc<Self>) -> Option<Box<NetBuf>> {
        Some(Box::new(self.alloc()?))
    }

    /// Deallocates a buffer at the given offset.
    ///
    /// `pool_offset` must be a multiple of `buf_len`.
    fn dealloc(&self, pool_offset: usize) {
        debug_assert_eq!(pool_offset % self.buf_len, 0);
        self.free_list.lock().push(pool_offset);
    }
}

pub struct NetBufPtr {
    // The raw pointer of the original object.
    raw_ptr: NonNull<u8>,
    // The pointer to the net buffer.
    buf_ptr: NonNull<u8>,
    len: usize,
}

impl NetBufPtr {
    /// Create a new [`NetBufPtr`].
    pub fn new(raw_ptr: NonNull<u8>, buf_ptr: NonNull<u8>, len: usize) -> Self {
        Self {
            raw_ptr,
            buf_ptr,
            len,
        }
    }

    /// Return raw pointer of the original object.
    pub fn raw_ptr<T>(&self) -> *mut T {
        self.raw_ptr.as_ptr() as *mut T
    }

    /// Return [`NetBufPtr`] buffer len.
    pub fn packet_len(&self) -> usize {
        self.len
    }

    /// Return [`NetBufPtr`] buffer as &[u8].
    pub fn packet(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.buf_ptr.as_ptr() as *const u8, self.len) }
    }

    /// Return [`NetBufPtr`] buffer as &mut [u8].
    pub fn packet_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.buf_ptr.as_ptr(), self.len) }
    }
}
