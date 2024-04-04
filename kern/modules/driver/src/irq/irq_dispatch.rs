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
pub static IRQ_COUNT: Mutex<BTreeMap<usize, i32>> = Mutex::new(BTreeMap::new());
const UART_LOAD: i32 = 1;
const NET_LOAD: i32 = 4;
pub fn init_strategy() {
    let cpu_id = hal!().cpu().id();
    INTERRUPT_COUNT.call_once(|| Mutex::new(vec![0; CPU_COUNT.try_into().unwrap()]));
    *ROTATE_INDEX.lock() = cpu_id;
    IRQ_COUNT.lock().insert(8, 0);
    IRQ_COUNT.lock().insert(10, 0);
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
pub fn min_load_strategy() {
    TimedEvent::create(
        hal!().cpu().get_time() + Duration::from_secs(TIME_INTERVAL),
        TimedEventHandler::new(|| min_load_strategy_event(), || {}),
    );
}
pub fn min_load_strategy_event() {
    let irq_table_lock = IRQ_TABLE.write();
    let mut plic_lock = irq_table_lock
    .get(PLIC_PHANDLE.get().unwrap())
    .unwrap()
    .lock();
    let mut cpu_task_priority: BTreeMap<usize, u8> = BTreeMap::new();
    for cpu_id in 0..CPU_COUNT {
        plic_lock
            .disable(cpu_id, 10)
            .unwrap();
         plic_lock
            .disable(cpu_id, 8)
            .unwrap();
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
    let mut cpu_task_pri_vec = Vec::from_iter(cpu_task_priority);
    cpu_task_pri_vec.sort_by(|&(_, a), &(_, b)| a.cmp(&b));
    let mut irq_count_lock = IRQ_COUNT.lock();
    let mut net_count = *irq_count_lock.get_mut(&8).unwrap();
    let mut uart_count = *irq_count_lock.get_mut(&10).unwrap();
    let net_irq_load = NET_LOAD * net_count;
    let uart_irq_load = UART_LOAD * uart_count;
    if net_irq_load > uart_irq_load {
        plic_lock
            .enable(cpu_task_pri_vec[0].0, 8)
            .unwrap();
        plic_lock
            .enable(cpu_task_pri_vec[1].0, 10)
            .unwrap();
    } else {
        plic_lock
            .enable(cpu_task_pri_vec[0].0, 10)
            .unwrap();
        plic_lock
            .enable(cpu_task_pri_vec[1].0, 8)
            .unwrap();
    }
    //plic_lock.info();
    let mut interrupt_count = INTERRUPT_COUNT.get().unwrap().lock();
    net_count = 0;
    uart_count = 0;
    min_load_strategy();
}
pub fn min_count_strategy() {
    TimedEvent::create(
        hal!().cpu().get_time() + Duration::from_secs(TIME_INTERVAL),
        TimedEventHandler::new(|| min_count_cpu_strategy_event(), || {}),
    );
}

pub fn min_count_cpu_strategy_event() {
    let irq_table_lock = IRQ_TABLE.write();
    let mut plic_lock = irq_table_lock.get(PLIC_PHANDLE.get().unwrap()).unwrap().lock();
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
    let interrupt_count = INTERRUPT_COUNT.get().unwrap().lock();
    let mut cpu_load: BTreeMap<usize, u32> = BTreeMap::new();
    for i in 0..CPU_COUNT {
        plic_lock
        .disable(i, 10)
        .unwrap();
        plic_lock
        .disable(i, 8)
        .unwrap();
        let cur_interrupt_count = *interrupt_count.get(i).unwrap();
        let cur_task_pri: u8 = *cpu_task_priority.get(&i).unwrap();
        let cur_load: u32 = u32::from(cur_interrupt_count) | u32::from(cur_task_pri) << 16;
        cpu_load.insert(i, cur_load);
    }
    let mut cpu_load_vec = Vec::from_iter(cpu_load);
    cpu_load_vec.sort_by(|&(_, a), &(_, b)| a.cmp(&b));
    plic_lock
        .enable(cpu_load_vec[0].0, 8)
        .unwrap();
    plic_lock
        .enable(cpu_load_vec[1].0, 10)
        .unwrap();
    // info!("enable uart cpu{}",cpu_load_vec[1].0);
    // info!("{} {} {} {} {}",*interrupt_count.get(0).unwrap(),
    //                        *interrupt_count.get(1).unwrap(),
    //                        *interrupt_count.get(2).unwrap(),
    //                        *interrupt_count.get(3).unwrap(),
    //                        *interrupt_count.get(4).unwrap());
    min_count_strategy();
}
pub fn rotate_strategy() {
    let irq_table_lock = IRQ_TABLE.write();
    let mut plic_lock = irq_table_lock.get(PLIC_PHANDLE.get().unwrap()).unwrap().lock();
    let mut value = ROTATE_INDEX.lock();
    plic_lock
        .disable((*value + CPU_COUNT - 1) % CPU_COUNT, 10)
        .unwrap();
    plic_lock
        .enable(*value + 0, 10)
        .unwrap();
    *value = (*value + 1) % CPU_COUNT;
}
