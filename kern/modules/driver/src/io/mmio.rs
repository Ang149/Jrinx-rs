use jrinx_hal::{mmio_read, mmio_write};

use super::Io;
use core::ops::{BitAnd, BitOr, Not};
#[repr(transparent)]
pub struct Mmio<T>(T);
impl<T> Mmio<T> {
    /// # Safety
    ///
    /// This function is unsafe because `base_addr` may be an arbitrary address.
    pub unsafe fn from_base_as<'a, R>(base_addr: usize) -> &'a mut R {
        assert_eq!(base_addr % core::mem::size_of::<T>(), 0);
        &mut *(base_addr as *mut R)
    }

    /// # Safety
    ///
    /// This function is unsafe because `base_addr` may be an arbitrary address.
    pub unsafe fn from_base<'a>(base_addr: usize) -> &'a mut Self {
        Self::from_base_as(base_addr)
    }

    pub fn add<'a>(&self, offset: usize) -> &'a mut Self {
        unsafe { Self::from_base((&self.0 as *const T).add(offset) as _) }
    }
}

impl<T> Io for Mmio<T>
where
    T: Copy + BitAnd<Output = T> + BitOr<Output = T> + Not<Output = T>,
{
    type Value = T;

    fn read(&self) -> T {
        mmio_read(self.0)
    }

    fn write(&mut self, value: T) {
        mmio_write(&mut self.0 as *mut _, value)
    }
}
