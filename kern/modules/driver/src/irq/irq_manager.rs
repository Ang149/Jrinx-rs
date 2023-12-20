use crate::Driver;
use alloc::sync::Arc;
use core::ops::Range;
pub struct IrqManager<const IRQ_RANGE: usize> {
    irq_range: Range<usize>,
    table: [Option<Arc<dyn Driver>>; IRQ_RANGE], 
}
impl<const IRQ_RANGE: usize> IrqManager<IRQ_RANGE> {
    pub fn new(irq_range: Range<usize>) -> Self {
        const EMPTY_DRIVER: Option<Arc<dyn Driver>> = None;
        Self {
            irq_range,
            table: [EMPTY_DRIVER; IRQ_RANGE], 
        }
    }
    pub fn register_device(&mut self, irq_num: usize, dev: Arc<dyn Driver>) {
        if self.irq_range.contains(&irq_num) && irq_num!= 0 {
            self.table[irq_num] = Some(dev);
        }
    }

    pub fn unregister_handler(&mut self, irq_num: usize) {
        if self.irq_range.contains(&irq_num) && irq_num!= 0 {
            self.table[irq_num] = None;
        }
    }
    pub fn handle_irq(&self, irq_num: usize) {
        if self.irq_range.contains(&irq_num) && irq_num!= 0 {
            if let Some(dev) = &self.table[irq_num] {
                dev.handle_irq(irq_num);
            }
        }
    }
    pub fn contains(&self, irq_num: usize) -> bool {
        self.table[irq_num].is_some()
    }
}
