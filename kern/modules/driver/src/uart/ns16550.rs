use crate::io::{Io, Mmio, ReadOnly};
use core::ops::{BitAnd, BitOr, Not};
use lock::Mutex;
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
impl<T: Io> Uart for NS16550Inner<T>
where
    T::Value: From<u8> + TryInto<u8>,
{
    fn init(&self) -> Result<()> {
        self.interrupt_enable.write(0x00.into());
        self.fifo_control.write(0xC7.into());
        self.modem_control.write(0x0B.into());
        self.interrupt_enable.write(0x01.into());
    }
    fn read(&self) -> Result<u8> {
        if self.line_status.read().contains(LineStsFlags::INPUT_FULL) {
            let data = self.data.read();
            if self.line_status.read().contains(LineStsFlags::ERRORED) {
                Err(Error::new("NS16550 read error"))
            } else {
                Ok(data.try_into().unwrap())
            }
        }
    }
    fn write(&self, data: u8) -> Result<()> {
        while !self.line_status.read().contains(LineStsFlags::OUTPUT_EMPTY) {}
        self.data.write(data.into());
        if self.line_status.read().contains(LineStsFlags::ERRORED) {
            Err(Error::new("NS16550 write error"))
        } else {
            Ok(())
        }
    }
    fn write_str(&self, data: &str) -> Result<()> {
        for c in data.bytes() {
            match c {
                b'\n' => {
                    self.write(b'\r')?;
                    self.write(b'\n')?;
                }
                _ => self.write(c)?,
            }
        }
        Ok(())
    }
}
