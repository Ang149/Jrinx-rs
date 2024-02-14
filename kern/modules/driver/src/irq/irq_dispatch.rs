use jrinx_hal::{hal, Cpu, Hal};

use super::{riscv_intc::IRQ_TABLE, riscv_plic::PLIC_PHANDLE};
pub struct IrqDispatch{
    strategy_num: usize,
} 
impl IrqDispatch {
    pub fn get_dispatch_num(&self) -> usize {
        self.strategy_num
    }
    
}
pub fn single_cpu_strategy() {
    IRQ_TABLE.write().get(&PLIC_PHANDLE.get().unwrap()).unwrap().lock().enable(hal!().cpu().id(),10);
}