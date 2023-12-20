
use super::riscv_intc::{self, Intc};
use super::irq_manager::IrqManager;
use crate::io::{Io, Mmio};
use crate::{Driver, InterruptController, InterruptHandler};
use alloc::sync::Arc;
use core::ops::Range;
use fdt::node::{self, FdtNode, NodeProperty};
use jrinx_devprober::devprober;
use jrinx_error::{InternalError, Result};
use jrinx_hal::{hal, Cpu, Hal};
use spin::{Mutex, Once};
const IRQ_RANGE: Range<usize> = 1..1024;
const PLIC_PRIORITY_BASE: usize = 0x0;
const PLIC_PENDING_BASE: usize = 0x1000;
const PLIC_ENABLE_BASE: usize = 0x2000;
const PLIC_CONTEXT_BASE: usize = 0x20_0000;

const PLIC_CONTEXT_THRESHOLD: usize = 0x0;
const PLIC_CONTEXT_CLAIM: usize = 0x4 / core::mem::size_of::<u32>();

const PLIC_ENABLE_CONTEXT_OFFSET: usize = 0x80 / core::mem::size_of::<u32>();
const PLIC_CONTEXT_HART_OFFSET: usize = 0x1000 / core::mem::size_of::<u32>();
static GLOBAL_RISC_PLIC: Once<PLIC> = Once::new();
#[devprober(compatible = "sifive,plic-1.0.0", compatible = "riscv,plic0")]
fn probe(node: &FdtNode) -> Result<()> {
    let region = node
        .reg()
        .ok_or(InternalError::DevProbeError)?
        .next()
        .ok_or(InternalError::DevProbeError)?;
    let addr = region.starting_address;
    let size = region.size.ok_or(InternalError::DevProbeError)?;
    GLOBAL_RISC_PLIC.try_call_once::<_, ()>(|| unsafe {
        Ok((PLIC::new(addr as usize)))
    });
    
    Ok(())
}

struct PLICInner {
    priority_base: &'static mut Mmio<u32>,
    enable_base: &'static mut Mmio<u32>,
    context_base: &'static mut Mmio<u32>,
    irq_manager: IrqManager<1024>, //更好的写法？
}
impl PLICInner {
    fn is_valid(&self, irq_num: usize) -> bool {
        IRQ_RANGE.contains(&irq_num)
    }
    fn get_current_cpu_claim(&mut self) -> Option<usize> {
        let hart_id = hal!().cpu().id();
        let irq_num = self
            .context_base
            .add(hart_id * PLIC_CONTEXT_HART_OFFSET)
            .add(PLIC_CONTEXT_CLAIM)
            .read() as usize;
        if irq_num == 0 {
            None
        } else {
            Some(irq_num)
        }
    }
    fn eoi(&mut self, irq_num: usize) {
        debug_assert!(self.is_valid(irq_num));
        let hart_id = hal!().cpu().id();
        self.context_base
            .add(hart_id * PLIC_CONTEXT_HART_OFFSET)
            .add(PLIC_CONTEXT_CLAIM)
            .write(irq_num as _);
    }
    fn enable(&mut self, cpu_id: usize, irq_num: usize) {
        debug_assert!(self.is_valid(irq_num));
        self.enable_base
            .add(cpu_id * PLIC_ENABLE_CONTEXT_OFFSET)
            .add(irq_num / 32)
            .write(1 << irq_num % 32);
    }
    fn disable(&mut self, cpu_id: usize, irq_num: usize) {
        debug_assert!(self.is_valid(irq_num));
        self.enable_base
            .add(cpu_id * PLIC_ENABLE_CONTEXT_OFFSET)
            .add(irq_num / 32)
            .write(0 << irq_num % 32);
    }
    fn set_priority(&mut self, irq_num: usize, priority: u32) {
        debug_assert!(self.is_valid(irq_num));
        self.priority_base.add(irq_num).write(priority);
    }
    fn set_threshold(&mut self, cpu_id: usize, threshold: u32) {
        self.context_base
            .add(cpu_id * PLIC_CONTEXT_HART_OFFSET)
            .add(PLIC_CONTEXT_THRESHOLD)
            .write(threshold);
    }
}

pub struct PLIC {
    inner: Mutex<PLICInner>,
}
impl PLIC {
    pub fn new(base_addr: usize) -> Self {
        let mut inner = PLICInner {
            priority_base: unsafe { Mmio::from_base(base_addr + PLIC_PRIORITY_BASE) },
            enable_base: unsafe { Mmio::from_base(base_addr + PLIC_ENABLE_BASE) },
            context_base: unsafe { Mmio::from_base(base_addr + PLIC_CONTEXT_BASE) },
            irq_manager: IrqManager::new(IRQ_RANGE),
        };
        PLIC {
            inner: Mutex::new(inner),
        }
    }
}
impl Driver for PLIC {
    fn name(&self) -> &str {
        "riscv_plic"
    }

    fn handle_irq(&self, irq_num: usize) {
        let mut inner = self.inner.lock();
        inner.irq_manager.handle_irq(irq_num);
        inner.eoi(irq_num);
    }
}
impl InterruptController for PLIC {
    fn is_valid(&self, irq_num: usize) -> bool {
        self.inner.lock().is_valid(irq_num)
    }

    fn contains(&self, irq_num: usize) -> bool {
        self.inner.lock().irq_manager.contains(irq_num)
    }

    fn enable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()> {
        let mut inner = self.inner.lock();
        inner.enable(cpu_id, irq_num);
        Ok(())
    }

    fn disable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()> {
        let mut inner = self.inner.lock();
        inner.disable(cpu_id, irq_num);
        Ok(())
    }

    fn register_handler(&self, irq_num: usize, handler: InterruptHandler) -> Result<()> {
        // let mut inner = self.inner.lock();
        // inner.irq_manager.register_handler(irq_num, handler);
        // Ok(())
        todo!()
    }
    fn register_device(&self, irq_num: usize, dev: Arc<dyn Driver>) -> Result<()> {
        let mut inner = self.inner.lock();
        inner.irq_manager.register_device(irq_num, dev);
        Ok(())
    }
    fn unregister_handler(&self, irq_num: usize) -> Result<()> {
        let mut inner = self.inner.lock();
        inner.irq_manager.unregister_handler(irq_num);
        Ok(())
    }
}
