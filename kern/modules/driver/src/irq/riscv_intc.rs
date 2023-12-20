use crate::Driver;
use crate::{InterruptController, InterruptHandler};
use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use jrinx_error::{InternalError, Result};
use jrinx_trap::{arch::Context, breakpoint, soft_int, timer_int, GenericContext, TrapReason};
use riscv::register::scause::Interrupt;
use riscv::register::sie;
use spin::{Mutex, RwLock};
pub(crate) static mut DEVICE_TABLE: Mutex<Vec<Arc<dyn Driver>>> = Mutex::new(Vec::new());
pub struct Intc {
    name: String,
    handler_table: RwLock<BTreeMap<usize, InterruptHandler>>,
}
impl Driver for Intc {
    fn name(&self) -> &str {
        &self.name
    }
    fn handle_irq(&self, irq_num: usize) {
        if self.handler_table.read().contains_key(&irq_num) {
            self.handler_table.read()[&irq_num]();
        }
    }
}
impl Intc {
    fn enable(&mut self, irq_num: usize) -> Result<()> {
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
    fn disable(&mut self, irq_num: usize) -> Result<()> {
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
    pub fn register_handler(&mut self, irq_num: usize, handler: InterruptHandler) -> Result<()> {
        self.handler_table.write().insert(irq_num, handler);
        Ok(())
    }
    pub fn register_device(&mut self, dev: Arc<dyn InterruptController>) -> Result<()> {
        todo!()
    }
    pub fn unregister_handler(&self, irq_num: usize) -> Result<()> {
        todo!();
    }
}
