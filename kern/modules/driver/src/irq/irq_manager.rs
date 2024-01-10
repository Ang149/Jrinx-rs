use crate::Driver;
use alloc::{collections::BTreeMap, sync::Arc};
use core::ops::Range;
use spin::Once;
pub struct IrqManager<const IRQ_RANGE: usize> {
    irq_range: Range<usize>,
    table: BTreeMap<usize, Option<Arc<&'static dyn Driver>>>,
}
impl<const IRQ_RANGE: usize> IrqManager<IRQ_RANGE> {
    pub fn new(irq_range: Range<usize>) -> Self {
        const EMPTY_DRIVER: Option<Arc<&dyn Driver>> = None;
        Self {
            irq_range,
            table: BTreeMap::<usize, Option<Arc<&'static dyn Driver>>>::new(),
        }
    }
    pub fn register_device(&mut self, irq_num: usize, dev: Arc<&'static dyn Driver>) {
        if self.irq_range.contains(&irq_num) && irq_num != 0 {
            self.table.insert(irq_num, Some(dev));
        }
    }

    pub fn unregister_handler(&mut self, irq_num: usize) {
        if self.irq_range.contains(&irq_num) && irq_num != 0 {
            self.table.remove(&irq_num);
        }
    }
    pub fn handle_irq(&self, irq_num: usize) {
        if self.irq_range.contains(&irq_num) && irq_num != 0 {
            if let Some(dev) = self.table.get(&irq_num) {
                dev.as_ref().unwrap().handle_irq(irq_num);
            }
        }
    }
    pub fn contains(&self, irq_num: usize) -> bool {
        self.table.get(&irq_num).is_some()
    }
}
