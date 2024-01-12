use crate::io::{Io, Mmio, ReadOnly};
use crate::irq::riscv_intc::GLOBAL_INTC;
use crate::{Driver, Uart};
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use bitflags::bitflags;
use core::ops::{BitAnd, BitOr, Not};
use fdt::node::FdtNode;
use jrinx_addr::{PhysAddr, VirtAddr};
use jrinx_config::EXTERNAL_DEVICE_REGION;
use jrinx_devprober::devprober;
use jrinx_error::{InternalError, Result};
use jrinx_hal::{hal, Hal, Vm};
use jrinx_paging::boot::BootPageTable;
use jrinx_paging::{GenericPagePerm, GenericPageTable, PagePerm};
use jrinx_phys_frame::PhysFrame;
use jrinx_vmm::KERN_PAGE_TABLE;
use log::{log, Level};
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
    unsafe {
        BootPageTable.map(
            VirtAddr::new(addr),
            PhysAddr::new(region.starting_address as usize),
        );
    }
    hal!().vm().sync_all();
    GLOBAL_NS16550.try_call_once::<_, ()>(|| Ok(NS16550::new(addr as usize)));

    // info!("123");

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
        self.fifo_control.write(0);
        self.line_control.write(0b11);
        self.modem_control.write(0);
        self.interrupt_enable.write(IntEnFlags::RECEIVED.bits());
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
        // info!("base: {:x}", base);
        let uart: &mut NS16550Inner = unsafe { Mmio::<u8>::from_base_as(base) };
        uart.init();
        Self {
            inner: Mutex::new(uart),
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
