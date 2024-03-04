use core::sync::atomic::AtomicUsize;

use jrinx_hal::{cpu, hal, Cpu, Hal};

use super::{riscv_intc::IRQ_TABLE, riscv_plic::PLIC_PHANDLE};
static mut INTERRUPT_COUNT: [usize; 5] = [0, 0, 0, 0, 0];
static mut ROTATE_INDEX: usize = 0;
pub struct IrqDispatch {
    strategy_num: usize,
}
impl IrqDispatch {
    pub fn get_dispatch_num(&self) -> usize {
        self.strategy_num
    }
}
pub fn init_strategy(){
    let cpu_id = hal!().cpu().id();
    unsafe { INTERRUPT_COUNT [cpu_id] = 1;}
    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(cpu_id, 10);
}
pub fn single_cpu_strategy() {
    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(hal!().cpu().id(), 8);
    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(hal!().cpu().id(), 10);
    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .info();
}
pub fn min_count_cpu_strategy() {
    let mut min_index = 0;
    for i in 0..5 {
        IRQ_TABLE
            .write()
            .get(PLIC_PHANDLE.get().unwrap())
            .unwrap()
            .lock()
            .disable(i, 10);
        unsafe {
            if INTERRUPT_COUNT[min_index] > INTERRUPT_COUNT[i] {
                min_index = i;
            }
        }
    }
    unsafe {
        INTERRUPT_COUNT[min_index] = INTERRUPT_COUNT[min_index] + 1;
    }
    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(min_index, 10);
    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .info();
}
pub fn rotate_strategy() {
    let mut value = 0;
    let tmp = 5;
    unsafe {
        value = ROTATE_INDEX;
        ROTATE_INDEX = (ROTATE_INDEX + 1) % tmp;
    }
    let binding = IRQ_TABLE.write();
    binding
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .disable((value + tmp - 1) % tmp, 10);
    binding
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(value + 0, 10);
    binding
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .info();
}
