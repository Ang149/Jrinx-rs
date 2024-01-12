use crate::io::{Io, Mmio, ReadOnly};
use crate::irq::riscv_intc::GLOBAL_INTC;
use crate::{Driver, Uart};
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use bitflags::bitflags;
use core::ops::{BitAnd, BitOr, Not};
use fdt::node::FdtNode;
use jrinx_devprober::devprober;
use jrinx_error::{InternalError, Result};
use log::{log, Level};
extern crate jrinx_config;
use jrinx_addr::VirtAddr;
use jrinx_config::EXTERNAL_DEVICE_REGION;
use jrinx_paging::{GenericPagePerm, GenericPageTable, PagePerm};
use jrinx_phys_frame::PhysFrame;
use jrinx_vmm::KERN_PAGE_TABLE;
use spin::{Mutex, Once};

pub static GLOBAL_NS16550: Once<NS16550> = Once::new();

#[devprober(compatible = "ns16550a")]
fn probe(node: &FdtNode) -> Result<()> {
    let region = node
        .reg()
        .ok_or(InternalError::DevProbeError)?
        .next()
        .ok_or(InternalError::DevProbeError)?;
    let addr = region.starting_address as usize + EXTERNAL_DEVICE_REGION.addr;
    let size = region.size.ok_or(InternalError::DevProbeError)?;
    let mut page_table = KERN_PAGE_TABLE.write();
    let phys_frame = PhysFrame::alloc()?;
    page_table.map(
        VirtAddr::new(addr as usize),
        phys_frame,
        PagePerm::G | PagePerm::R | PagePerm::W,
    )?;
    log!(Level::Info, "a log event");
    //GLOBAL_NS16550.try_call_once::<_, ()>(|| unsafe { Ok(NS16550::new(addr as usize)) });
    // GLOBAL_INTC
    //     .get()
    //     .unwrap()
    //     .register_device(6, Arc::new(GLOBAL_NS16550.get().unwrap()));
    //GLOBAL_NS16550.get().unwrap().write_str("test uart write");
    Ok(())
}
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
pub struct NS16550 {
    inner: Mutex<&'static mut NS16550Inner>,
}
#[repr(C)]
struct NS16550Inner {
    data: Mmio<u8>,
    interrupt_enable: Mmio<u8>,
    line_control: Mmio<u8>,
    fifo_control: Mmio<u8>,
    modem_control: Mmio<u8>,
    line_status: ReadOnly<Mmio<u8>>,
    modem_status: ReadOnly<Mmio<u8>>,
}
impl NS16550Inner {
    fn line_status(&self) -> LineStsFlags {
        LineStsFlags::from_bits_truncate(
            (self.line_status.read() & 0xFF as u8)
                .try_into()
                .unwrap_or(0),
        )
    }
    fn init(&mut self) -> Result<()> {
        self.interrupt_enable.write(0x00 as u8);
        self.fifo_control.write(0xC7 as u8);
        self.modem_control.write(0x0B as u8);
        self.interrupt_enable.write(0x01 as u8);
        Ok(())
    }
    fn read(&mut self) -> Option<u8> {
        if self.line_status().contains(LineStsFlags::INPUT_FULL) {
            let data = self.data.read();
            Some(data.try_into().unwrap_or(0))
        } else {
            None
        }
    }
    fn write(&mut self, data: u8) -> Result<()> {
        while !self.line_status().contains(LineStsFlags::OUTPUT_EMPTY) {}

        self.data.write(data);
        Ok(())
    }
}
impl NS16550 {
    fn new(base: usize) -> Self {
        unsafe {
            let uart: &mut NS16550Inner = Mmio::<u8>::from_base_as(base);
            uart.init();
            Self {
                inner: Mutex::new(uart),
            }
        }
    }
    pub fn write(&self, data: u8) -> Result<()> {
        self.inner.lock().write(data)
    }
    pub fn write_str(&self, data: &str) -> Result<()> {
        for b in data.bytes() {
            match b {
                b'\n' => {
                    self.write(b'\r')?;
                    self.write(b'\n')?;
                }
                _ => {
                    self.write(b)?;
                }
            }
        }
        Ok(())
    }
    pub fn read(&self) -> Option<u8> {
        self.inner.lock().read()
    }
}
impl Driver for NS16550 {
    fn name(&self) -> &str {
        "ns16550"
    }
    fn handle_irq(&self, irq_num: usize) {
        while let Some(ch) = self.inner.lock().read() {
            //info!("ns16550 handle irq and read {}", ch);
        }
    }
}
