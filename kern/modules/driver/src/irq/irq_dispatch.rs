#![warn(unused_variables)]
use alloc::{vec, vec::Vec};
use core::time::Duration;
use jrinx_hal::{hal, Cpu, Hal};
use jrinx_timed_event::{TimedEvent, TimedEventHandler};
use spin::{Mutex, Once};

use super::{riscv_intc::IRQ_TABLE, riscv_plic::PLIC_PHANDLE};
const CPU_COUNT: usize = 5;
const TIME_INTERVAL: u64 = 1;
pub(crate) static INTERRUPT_COUNT: Once<Mutex<Vec<i64>>> = Once::new();
static ROTATE_INDEX: Mutex<usize> = Mutex::new(0);
static PRE: Mutex<usize> = Mutex::new(0);
pub fn init_strategy() {
    let cpu_id = hal!().cpu().id();
    INTERRUPT_COUNT.call_once(|| Mutex::new(vec![0; CPU_COUNT.try_into().unwrap()]));
    *ROTATE_INDEX.lock() = cpu_id;
    *PRE.lock() = cpu_id;
    // IRQ_TABLE
    //     .write()
    //     .get(PLIC_PHANDLE.get().unwrap())
    //     .unwrap()
    //     .lock()
    //     .enable(0, 8)
    //     .unwrap();
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
        hal!().cpu().get_time() + Duration::from_secs(TIME_INTERVAL),
        TimedEventHandler::new(|| min_count_cpu_strategy_event(), || {}),
    );
}
pub fn min_count_cpu_strategy_event() {
    let mut min_index = 0;
    let interrupt_count = INTERRUPT_COUNT.get().unwrap().lock();
    let mut pre_lock = PRE.lock();
    for i in 0..5 {
        if interrupt_count.get(min_index).unwrap() > interrupt_count.get(i).unwrap() {
            min_index = i;
        }
    }
    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .disable(*pre_lock, 10)
        .unwrap();
    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(min_index, 10)
        .unwrap();
    *pre_lock = min_index;
    min_count_strategy();
}
pub fn rotate_strategy() {
    let mut value = ROTATE_INDEX.lock();
    let binding = IRQ_TABLE.write();
    binding
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .disable((*value + CPU_COUNT - 1) % CPU_COUNT, 10)
        .unwrap();
    binding
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(*value + 0, 10)
        .unwrap();
    *value = (*value + 1) % CPU_COUNT;
}
