#![warn(unused_variables)]
use super::{riscv_intc::IRQ_TABLE, riscv_plic::PLIC_PHANDLE};
use alloc::collections::BTreeMap;
use alloc::{vec, vec::Vec};
use core::time::Duration;
use jrinx_error::InternalError;
use jrinx_hal::{hal, Cpu, Hal};
use jrinx_multitask::inspector::InspectorStatus;
use jrinx_multitask::runtime::{Runtime, RuntimeStatus};
use jrinx_timed_event::{TimedEvent, TimedEventHandler};
use spin::{Mutex, Once};
const CPU_COUNT: usize = 5;
const TIME_INTERVAL: u64 = 1;
pub(crate) static INTERRUPT_COUNT: Once<Mutex<Vec<u8>>> = Once::new();
static ROTATE_INDEX: Mutex<usize> = Mutex::new(0);
pub fn init_strategy() {
    let cpu_id = hal!().cpu().id();
    INTERRUPT_COUNT.call_once(|| Mutex::new(vec![0; CPU_COUNT.try_into().unwrap()]));
    *ROTATE_INDEX.lock() = cpu_id;

    IRQ_TABLE
        .write()
        .get(PLIC_PHANDLE.get().unwrap())
        .unwrap()
        .lock()
        .enable(cpu_id, 8)
        .unwrap();
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
    let mut cpu_task_priority: BTreeMap<usize, u8> = BTreeMap::new();
    for cpu_id in 0..CPU_COUNT {
        let _ = Runtime::with_spec_cpu(cpu_id, |rt| {
            Ok({
                let RuntimeStatus::Running(inspector_id) = rt.status() else {
                    return Err(InternalError::InvalidRuntimeStatus);
                };
                rt.with_inspector(inspector_id, |is| {
                    Ok({
                        let InspectorStatus::Running(executor_id) = is.status() else {
                            return Err(InternalError::InvalidRuntimeStatus);
                        };
                        let priority =
                            is.with_executor(executor_id, |ex| ex.current_task_priority())?;
                        cpu_task_priority.insert(cpu_id, priority.into());
                    })
                })
            })
        });
    }
    // info!(
    //     "priority cpu0:{:?},cpu1:{:?},cpu2:{:?},cpu3:{:?},cpu4:{:?}",
    //     cpu_task_priority.get(&0).unwrap(),
    //     cpu_task_priority.get(&1).unwrap(),
    //     cpu_task_priority.get(&2).unwrap(),
    //     cpu_task_priority.get(&3).unwrap(),
    //     cpu_task_priority.get(&4).unwrap(),
    // );
    let mut min_index = 0;
    let interrupt_count = INTERRUPT_COUNT.get().unwrap().lock();
    let mut cpu_load: BTreeMap<usize, u32> = BTreeMap::new();
    for i in 0..CPU_COUNT {
        IRQ_TABLE
            .write()
            .get(PLIC_PHANDLE.get().unwrap())
            .unwrap()
            .lock()
            .disable(i, 10)
            .unwrap();
        let cur_interrupt_count = *interrupt_count.get(i).unwrap();
        let cur_task_pri: u8 = *cpu_task_priority.get(&i).unwrap();
        let cur_load: u32 = u32::from(cur_interrupt_count) | u32::from(cur_task_pri) << 24;
        cpu_load.insert(i, cur_load);
        if cur_load < *cpu_load.get(&min_index).unwrap() {
            min_index = i;
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
