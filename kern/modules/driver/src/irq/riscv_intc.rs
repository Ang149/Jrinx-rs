use crate::{Driver, InterruptController, InterruptHandler};
use alloc::string::ToString;
use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use jrinx_devprober::devprober;
use jrinx_error::{InternalError, Result};
//use jrinx_trap::{arch::Context, breakpoint, soft_int, timer_int, GenericContext, TrapReason};
use fdt::node::{self, FdtNode, NodeProperty};
use riscv::register::scause::Interrupt;
use riscv::register::sie;
use spin::{Mutex, Once, RwLock};

use super::riscv_plic::GLOBAL_RISC_PLIC;
pub static GLOBAL_INTC: Once<Arc<dyn InterruptController>> = Once::new();

#[devprober(compatible = "riscv,cpu-intc")]
fn probe(node: &FdtNode) -> Result<()> {
    GLOBAL_INTC.call_once(|| Arc::new(Intc::new()));
    Ok(())
}
pub struct Intc {
    name: String,
    handler_table: RwLock<BTreeMap<usize, InterruptHandler>>,
}
impl Driver for Intc {
    fn name(&self) -> &str {
        &self.name
    }
    fn handle_irq(&self, irq_num: usize) {
        GLOBAL_RISC_PLIC.get().unwrap().handle_irq(irq_num);
    }
}
impl InterruptController for Intc {
    fn register_device(&self, irq_num: usize, dev: Arc<&'static dyn Driver>) -> Result<()> {
        GLOBAL_RISC_PLIC
            .get()
            .unwrap()
            .register_device(irq_num, dev)
    }
    fn register_handler(&self, irq_num: usize, handler: InterruptHandler) -> Result<()> {
        GLOBAL_RISC_PLIC
            .get()
            .unwrap()
            .register_handler(irq_num, handler)
    }
    fn enable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()> {
        unsafe {
            match Interrupt::from(irq_num) {
                Interrupt::SupervisorSoft => sie::clear_ssoft(),
                Interrupt::SupervisorTimer => sie::clear_stimer(),
                Interrupt::SupervisorExternal => sie::clear_sext(),
                _ => return Err(InternalError::DevWriteError),
            }
        }
        Ok(())
    }
    fn disable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()> {
        unsafe {
            match Interrupt::from(irq_num) {
                Interrupt::SupervisorSoft => sie::set_ssoft(),
                Interrupt::SupervisorTimer => sie::set_stimer(),
                Interrupt::SupervisorExternal => sie::set_sext(),
                _ => return Err(InternalError::DevWriteError),
            }
        }
        Ok(())
    }

    fn unregister_handler(&self, irq_num: usize) -> Result<()> {
        GLOBAL_RISC_PLIC.get().unwrap().unregister_handler(irq_num)
    }
}
impl Intc {
    fn new() -> Self {
        Self {
            name: "riscv_intc".to_string(),
            handler_table: RwLock::new(BTreeMap::new()),
        }
    }
}
