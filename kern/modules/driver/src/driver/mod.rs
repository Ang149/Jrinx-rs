use Send;
use Sync;
pub trait Driver: Send + Sync {
    fn name(&self) -> &str;

    fn handle_irq(&self, irq_num: usize) {}
}
