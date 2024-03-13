use crate::io::{Io, Mmio, ReadOnly};
use crate::irq::riscv_intc::IRQ_TABLE;
use crate::Driver;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use bitflags::bitflags;
use fdt::node::FdtNode;
use jrinx_addr::{PhysAddr, VirtAddr};
use jrinx_config::EXTERNAL_DEVICE_REGION;
use jrinx_devprober::devprober;
use jrinx_error::{InternalError, Result};
use jrinx_hal::{hal, Cpu, Hal, Vm};
use jrinx_paging::boot::BootPageTable;
use spin::Mutex;

#[devprober(compatible = "ns16550a")]
fn probe(node: &FdtNode) -> Result<()> {
    let region = node
        .reg()
        .ok_or(InternalError::DevProbeError)?
        .next()
        .ok_or(InternalError::DevProbeError)?;
    let vaddr = region.starting_address as usize + EXTERNAL_DEVICE_REGION.addr;
    let size = region.size.ok_or(InternalError::DevProbeError)?;
    let irq_num = node
        .interrupts()
        .ok_or(InternalError::DevProbeError)?
        .next()
        .ok_or(InternalError::DevProbeError)?;
    let interrupt_parent = node
        .interrupt_parent()
        .ok_or(InternalError::DevProbeError)?
        .property("phandle")
        .ok_or(InternalError::DevProbeError)?
        .as_usize()
        .unwrap();

    unsafe {
        BootPageTable.map(
            VirtAddr::new(vaddr),
            PhysAddr::new(region.starting_address as usize),
        );
    }
    hal!().vm().sync_all();
    IRQ_TABLE
        .write()
        .get(&interrupt_parent)
        .unwrap()
        .lock()
        .register_device(irq_num, Arc::new(NS16550a::new(vaddr)))
        .unwrap();
    info!("ns16550a vaddr {:x}, size {:x}",vaddr,size);
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
pub struct NS16550a {
    inner: Mutex<&'static mut NS16550Inner>,
    buffer: Mutex<VecDeque<u8>>,
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
        LineStsFlags::from_bits_truncate(self.line_status.read())
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
            Some(data)
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
impl NS16550a {
    fn new(base: usize) -> Self {
        let uart: &mut NS16550Inner = unsafe { Mmio::<u8>::from_base_as(base) };
        uart.init().unwrap();
        Self {
            inner: Mutex::new(uart),
            buffer: Mutex::new(VecDeque::new()),
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
impl Driver for NS16550a {
    fn name(&self) -> &str {
        "ns16550a"
    }
    fn handle_irq(&self, _irq_num: usize) {
        while let Some(ch) = self.inner.lock().read() {
            self.buffer.lock().push_back(ch);
            info!("ns16550a handle irq and read {}", ch as char);
        }
    }
}
