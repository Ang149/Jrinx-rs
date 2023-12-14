#![allow(unused)]
use crate::io::{Io, Mmio, ReadOnly};
use crate::Uart;
use bitflags::bitflags;
use core::ops::{BitAnd, BitOr, Not};
use jrinx_error::{InternalError, Result};
bitflags! {
    /// Interrupt enable flags
    struct IntEnFlags: u8
    {
        const RECEIVED = 1;
        const SENT = 1 << 1;
        const ERRORED = 1 << 2;
        const STATUS_CHANGE = 1 << 3;
    }
}

bitflags! {
    /// Line status flags
    struct LineStsFlags: u8
    {
        const INPUT_FULL = 1;
        const OUTPUT_EMPTY = 1 << 5;
    }
}
struct NS16550Inner<T: Io> {
    data: T,
    interrupt_enable: T,
    line_control: T,
    fifo_control: T,
    modem_control: T,
    line_status: ReadOnly<T>,
    modem_status: ReadOnly<T>,
}
impl<T: Io> NS16550Inner<T>
where
    T::Value: From<u8> + TryInto<u8>,
{
    fn line_status(&self) -> LineStsFlags {
        LineStsFlags::from_bits_truncate(
            (self.line_status.read() & 0xFF.into())
                .try_into()
                .unwrap_or(0),
        )
    }
    fn init(&mut self) -> Result<()> {
        self.interrupt_enable.write(0x00.into());
        self.fifo_control.write(0xC7.into());
        self.modem_control.write(0x0B.into());
        self.interrupt_enable.write(0x01.into());
        Ok(())
    }
    fn read(&mut self) -> Result<u8> {
        if self.line_status().contains(LineStsFlags::INPUT_FULL) {
            let data = self.data.read();
            Ok(data.try_into().unwrap_or(0))
        } else {
            Result::Err(InternalError::DevReadError)
        }
    }
    fn write(&mut self, data: u8) -> Result<()> {
        while !self.line_status().contains(LineStsFlags::OUTPUT_EMPTY) {}
        self.data.write(data.into());
        Ok(())
    }
}
