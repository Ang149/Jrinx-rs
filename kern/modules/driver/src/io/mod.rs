use core::ops::{BitAnd, BitOr, Not};
mod mmio;

pub trait InterruptController {
    type Value: Copy
        + BitAnd<Output = Self::Value>
        + BitOr<Output = Self::Value>
        + Not<Output = Self::Value>;

    fn read(&self) -> Self::Value;

    fn write(&mut self, value: Self::Value);
}
#[repr(transparent)]
pub struct ReadOnly<I>(I);

impl<I> ReadOnly<I> {
    pub const fn new(inner: I) -> Self {
        Self(inner)
    }
}

impl<I: Io> ReadOnly<I> {
    #[inline(always)]
    pub fn read(&self) -> I::Value {
        self.0.read()
    }
}

#[repr(transparent)]
pub struct WriteOnly<I>(I);

impl<I> WriteOnly<I> {
    pub const fn new(inner: I) -> Self {
        Self(inner)
    }
}

impl<I: Io> WriteOnly<I> {
    #[inline(always)]
    pub fn write(&mut self, value: I::Value) {
        self.0.write(value);
    }
}
