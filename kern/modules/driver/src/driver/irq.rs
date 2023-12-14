pub trait InterruptController: Driver {
    fn is_valid(&self,irq_num:usize)->bool;
    fn enable(&mut self,cpu_id:usize,irq_num:usize)->Result<()> ;
    fn disable(&mut self,cpu_id:usize,irq_num:usize)->Result<()> ;
    fn register_handler(&self,irq_num:usize,handler:InterruptHandler)->Result<()> ;
    fn unregister_handler(&self,irq_num:usize)->Result<()> ;
}