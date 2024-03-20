
use crate::Driver;
use alloc::{collections::BTreeMap, sync::Arc};
use core::{ops::Range, time::Duration};
pub struct IrqManager {
    irq_range: Range<usize>,
    table: BTreeMap<usize, Option<Arc<dyn Driver>>>,
}
impl IrqManager {
    pub fn new(irq_range: Range<usize>) -> Self {
        Self {
            irq_range,
            table: BTreeMap::<usize, Option<Arc<dyn Driver>>>::new(),
        }
    }
    pub fn register_device(&mut self, irq_num: usize, dev: Arc<dyn Driver>) {
        if self.irq_range.contains(&irq_num) && irq_num != 0 {
            self.table.insert(irq_num, Some(dev));
        }
    }

    // pub fn unregister_handler(&mut self, irq_num: usize) {
    //     if self.irq_range.contains(&irq_num) && irq_num != 0 {
    //         self.table.remove(&irq_num);
    //     }
    // }
    pub fn handle_irq(&self, irq_num: usize)->Duration {
        let mut start_time = Duration::new(0, 0);
        if self.irq_range.contains(&irq_num) && irq_num != 0 {
            if let Some(dev) = self.table.get(&irq_num) {
                start_time = dev.as_ref().unwrap().handle_irq(irq_num);
            }
            else
            {
                info!("handle error");
            }
        }
        start_time
    }
    // pub fn contains(&self, irq_num: usize) -> bool {
    //     self.table.get(&irq_num).is_some()
    // }
}
