use super::irq_dispatch::IRQ_COUNT;
use super::irq_manager::IrqManager;
use crate::io::{Io, Mmio};
use crate::irq::riscv_intc::IRQ_TABLE;
use crate::{Driver, InterruptController};
use alloc::sync::Arc;
use core::ops::Range;
use core::time::Duration;
use fdt::node::FdtNode;
use jrinx_addr::{PhysAddr, VirtAddr};
use jrinx_devprober::{devprober, ROOT_COMPATIBLE};
use jrinx_error::{InternalError, Result};
use jrinx_hal::{hal, Cpu, Hal, Vm};
use jrinx_paging::boot::BootPageTable;
use log::info;
use spin::{Mutex, Once};
extern crate jrinx_config;
use jrinx_config::{EXTERNAL_DEVICE_REGION, PAGE_SIZE};

pub static PLIC_PHANDLE: Once<usize> = Once::new();

const IRQ_RANGE: Range<usize> = 1..1024;
const PLIC_PRIORITY_BASE: usize = 0x0;
// const PLIC_PENDING_BASE: usize = 0x1000;
const PLIC_ENABLE_BASE: usize = 0x2000;
const PLIC_CONTEXT_BASE: usize = 0x20_0000;

const PLIC_CONTEXT_THRESHOLD: usize = 0x0;
const PLIC_CONTEXT_CLAIM: usize = 0x4 / core::mem::size_of::<u32>();

const PLIC_ENABLE_CONTEXT_OFFSET: usize = 0x80 / core::mem::size_of::<u32>();
const PLIC_CONTEXT_HART_OFFSET: usize = 0x1000 / core::mem::size_of::<u32>();
#[devprober(compatible = "sifive,plic-1.0.0")]
fn probe(node: &FdtNode) -> Result<()> {
    let region = node
        .reg()
        .ok_or(InternalError::DevProbeError)?
        .next()
        .ok_or(InternalError::DevProbeError)?;
    let addr = region.starting_address as usize + EXTERNAL_DEVICE_REGION.addr;
    let size = region.size.ok_or(InternalError::DevProbeError)?;
    let phandle = node
        .property("phandle")
        .ok_or(InternalError::DevProbeError)?
        .as_usize()
        .unwrap();
    let context_max_id = (size - 0x200000_usize) / 0x1000_usize - 1_usize;
    let count = size / PAGE_SIZE;
    unsafe {
        for i in 0..count {
            BootPageTable.map(
                VirtAddr::new(addr + i * PAGE_SIZE),
                PhysAddr::new(region.starting_address as usize + i * PAGE_SIZE),
            );
        }
    }
    info!("addr is {:x} ,size is {:x}", addr, count);
    IRQ_TABLE.write().insert(
        phandle,
        Arc::new(Mutex::new(Plic::new(addr, context_max_id))) as _,
    );
    PLIC_PHANDLE.call_once(|| phandle);
    hal!().vm().sync_all();
    Ok(())
}

struct PLICInner {
    priority_base: &'static mut Mmio<u32>,
    enable_base: &'static mut Mmio<u32>,
    context_base: &'static mut Mmio<u32>,
    irq_manager: IrqManager,
}
//单核测试用
fn get_current_context_id() -> usize {
    let hart_id = hal!().cpu().id();
    match ROOT_COMPATIBLE.get().unwrap().as_str() {
        "riscv-virtio" => hart_id * 2 + 1,
        "sifive" => {
            if hart_id == 0 {
                panic!("hart id 0 is invalid");
            }
            hart_id * 2
        }
        _ => panic!("unknown root compatible"),
    }
}
fn get_context_id(cpu_id: usize) -> usize {
    match ROOT_COMPATIBLE.get().unwrap().as_str() {
        "riscv-virtio" => cpu_id * 2 + 1,
        "sifive" => {
            if cpu_id == 0 {
                panic!("cpu id 0 is invalid");
            }
            cpu_id * 2
        }
        _ => panic!("unknown root compatible"),
    }
}
impl PLICInner {
    fn info(&mut self) {
        info!(
            "cpu enable bit is {:b} {:b} {:b} {:b} {:b}",
            self.enable_base
                .add(get_context_id(0) * PLIC_ENABLE_CONTEXT_OFFSET)
                .read(),
            self.enable_base
                .add(get_context_id(1) * PLIC_ENABLE_CONTEXT_OFFSET)
                .read(),
            self.enable_base
                .add(get_context_id(2) * PLIC_ENABLE_CONTEXT_OFFSET)
                .read(),
            self.enable_base
                .add(get_context_id(3) * PLIC_ENABLE_CONTEXT_OFFSET)
                .read(),
            self.enable_base
                .add(get_context_id(4) * PLIC_ENABLE_CONTEXT_OFFSET)
                .read()
        );
        info!(
            "cpu threshold is {} {} {} {} {}",
            self.context_base
                .add(get_context_id(0) * PLIC_CONTEXT_HART_OFFSET)
                .add(PLIC_CONTEXT_THRESHOLD)
                .read(),
            self.context_base
                .add(get_context_id(1) * PLIC_CONTEXT_HART_OFFSET)
                .add(PLIC_CONTEXT_THRESHOLD)
                .read(),
            self.context_base
                .add(get_context_id(2) * PLIC_CONTEXT_HART_OFFSET)
                .add(PLIC_CONTEXT_THRESHOLD)
                .read(),
            self.context_base
                .add(get_context_id(3) * PLIC_CONTEXT_HART_OFFSET)
                .add(PLIC_CONTEXT_THRESHOLD)
                .read(),
            self.context_base
                .add(get_context_id(4) * PLIC_CONTEXT_HART_OFFSET)
                .add(PLIC_CONTEXT_THRESHOLD)
                .read()
        );
        // info!("current cpu is {}",hal!().cpu().id());
    }
    fn init(&mut self, context_max_id: usize) {
        for i in 0..=context_max_id {
            self.disable_all(i);
            self.set_threshold(i, 0);
        }
    }
    fn is_valid(&self, irq_num: usize) -> bool {
        IRQ_RANGE.contains(&irq_num)
    }
    fn get_current_cpu_claim(&mut self) -> Option<usize> {
        let context_id = get_current_context_id();
        let irq_num = self
            .context_base
            .add(context_id * PLIC_CONTEXT_HART_OFFSET)
            .add(PLIC_CONTEXT_CLAIM)
            .read() as usize;
        //info!("claim is {}", irq_num);
        if irq_num == 0 {
            None
        } else {
            Some(irq_num)
        }
    }
    fn end_of_interrupt(&mut self, irq_num: usize) {
        debug_assert!(self.is_valid(irq_num));
        let context_id = get_current_context_id();
        self.context_base
            .add(context_id * PLIC_CONTEXT_HART_OFFSET)
            .add(PLIC_CONTEXT_CLAIM)
            .write(irq_num as _);
    }
    fn enable(&mut self, context_id: usize, irq_num: usize) {
        debug_assert!(self.is_valid(irq_num));
        let content = self
            .enable_base
            .add(context_id * PLIC_ENABLE_CONTEXT_OFFSET)
            .add(irq_num / 32)
            .read();
        self.enable_base
            .add(context_id * PLIC_ENABLE_CONTEXT_OFFSET)
            .add(irq_num / 32)
            .write(1 << (irq_num % 32) | content);
    }
    fn disable(&mut self, context_id: usize, irq_num: usize) {
        debug_assert!(self.is_valid(irq_num));
        let content = self
            .enable_base
            .add(context_id * PLIC_ENABLE_CONTEXT_OFFSET)
            .add(irq_num / 32)
            .read();
        self.enable_base
            .add(context_id * PLIC_ENABLE_CONTEXT_OFFSET)
            .add(irq_num / 32)
            .write(!(1 << (irq_num % 32)) & content);
    }
    fn disable_all(&mut self, context_id: usize) {
        for i in 0..128 {
            self.enable_base
                .add(context_id * PLIC_ENABLE_CONTEXT_OFFSET + i * 4)
                .write(0);
        }
    }
    fn set_priority(&mut self, irq_num: usize, priority: u8) {
        debug_assert!(self.is_valid(irq_num));
        self.priority_base.add(irq_num).write(priority as _);
    }
    fn set_threshold(&mut self, context_id: usize, threshold: u8) {
        self.context_base
            .add(context_id * PLIC_CONTEXT_HART_OFFSET)
            .add(PLIC_CONTEXT_THRESHOLD)
            .write(threshold as _);
    }
}
pub struct Plic {
    inner: Mutex<PLICInner>,
}
impl Plic {
    pub fn new(base_addr: usize, context_max_id: usize) -> Self {
        let mut inner = PLICInner {
            priority_base: unsafe { Mmio::from_base(base_addr + PLIC_PRIORITY_BASE) },
            enable_base: unsafe { Mmio::from_base(base_addr + PLIC_ENABLE_BASE) },
            context_base: unsafe { Mmio::from_base(base_addr + PLIC_CONTEXT_BASE) },
            irq_manager: IrqManager::new(IRQ_RANGE),
        };
        inner.init(context_max_id);
        Plic {
            inner: Mutex::new(inner),
        }
    }
}
impl Driver for Plic {
    fn name(&self) -> &str {
        "riscv_plic"
    }

    fn handle_irq(&self, _: usize) -> Duration {
        let mut inner = self.inner.lock();
        match inner.get_current_cpu_claim() {
            Some(irq_num) => {
                debug!("cpu {} claim irq {}", hal!().cpu().id(), irq_num);
                *IRQ_COUNT.lock().get_mut(&irq_num).unwrap() += 1;
                let start_time = inner.irq_manager.handle_irq(irq_num);
                inner.end_of_interrupt(irq_num);
                start_time
            }
            _ => {
                warn!("plic claim zero");
                return Duration::new(0, 0);
            }
        }
        //info!("current cpu claim is {}",irq_num);
    }
}
impl InterruptController for Plic {
    fn info(&self) {
        self.inner.lock().info();
    }
    fn enable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()> {
        let mut inner = self.inner.lock();
        let context_id = get_context_id(cpu_id);
        inner.enable(context_id, irq_num);
        inner.set_threshold(context_id, 0);
        Ok(())
    }

    fn disable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()> {
        let mut inner = self.inner.lock();
        let context_id = get_context_id(cpu_id);
        inner.disable(context_id, irq_num);
        Ok(())
    }

    fn register_device(&self, irq_num: usize, dev: Arc<dyn Driver>) -> Result<()> {
        let mut inner = self.inner.lock();
        inner.irq_manager.register_device(irq_num, dev);
        inner.set_priority(irq_num, 7);
        Ok(())
    }
}
