use crate::{Driver, InterruptController, InterruptHandler};
use alloc::string::ToString;
use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use fdt::node::{self, FdtNode, NodeProperty};
use jrinx_devprober::devprober;
use jrinx_error::{InternalError, Result};
use riscv::register::scause::Interrupt;
use riscv::register::sie;
use spin::{Mutex, Once, RwLock};

use super::irq_dispatch::{min_count_cpu_strategy, rotate_strategy};
use super::riscv_plic::PLIC_PHANDLE;

pub static GLOBAL_INTC: Once<Arc<dyn InterruptController>> = Once::new();
pub static IRQ_TABLE:RwLock<BTreeMap<usize, Arc<Mutex<dyn InterruptController>>>> = RwLock::new(BTreeMap::new()) ;
#[devprober(compatible = "riscv,cpu-intc")]
fn probe(node: &FdtNode) -> Result<()> {
    GLOBAL_INTC.call_once(|| Arc::new(Intc::new()));
    Ok(())
}
pub struct Intc {
    name: String,
    // handler_table: RwLock<BTreeMap<usize, InterruptHandler>>,
}
impl Driver for Intc {
    fn name(&self) -> &str {
        &self.name
    }
    fn handle_irq(&self, irq_num: usize) {
        IRQ_TABLE.write().get(PLIC_PHANDLE.get().unwrap()).unwrap().lock().handle_irq(irq_num);
    }
}
impl InterruptController for Intc {
    fn info(&self) {
        todo!()
    }
    fn register_device(&self, irq_num: usize, dev: Arc<dyn Driver>) -> Result<()> {
        todo!()
    }

    fn enable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()> {
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
    fn disable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()> {
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

}
impl Intc {
    fn new() -> Self {
        Self {
            name: "riscv_intc".to_string(),
            // handler_table: RwLock::new(BTreeMap::new()),
        }
    }
}
