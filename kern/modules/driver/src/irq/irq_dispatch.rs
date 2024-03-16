#![warn(unused_variables)]
use core::time::Duration;

use jrinx_hal::{hal, Cpu, Hal};
use jrinx_timed_event::{TimedEvent, TimedEventHandler};

use super::{riscv_intc::IRQ_TABLE, riscv_plic::PLIC_PHANDLE};
pub static mut INTERRUPT_COUNT: [usize; 5] = [0, 0, 0, 0, 0];
static mut ROTATE_INDEX: usize = 0;
pub fn init_strategy() {
    let cpu_id = hal!().cpu().id();
    unsafe {
        INTERRUPT_COUNT[cpu_id] = 1;
        ROTATE_INDEX = cpu_id;
    }
    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(cpu_id, 10)
        .unwrap();
}
pub fn min_count_strategy() {
    TimedEvent::create(
        hal!().cpu().get_time() + Duration::from_secs(1),
        TimedEventHandler::new(|| min_count_cpu_strategy_event(), || {}),
    );
}
pub fn min_count_cpu_strategy_event() {
    let mut min_index = 0;
    for i in 0..5 {
        IRQ_TABLE
            .write()
            .get(PLIC_PHANDLE.get().unwrap())
            .unwrap()
            .lock()
            .disable(i, 10)
            .unwrap();
        unsafe {
            if INTERRUPT_COUNT[min_index] > INTERRUPT_COUNT[i] {
                min_index = i;
            }
        }
    }

    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(min_index, 10)
        .unwrap();

    min_count_strategy();
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
        .disable((value + tmp - 1) % tmp, 10)
        .unwrap();
    binding
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(value + 0, 10)
        .unwrap();
}
