#![no_std]
#![feature(used_with_arg)]
#![allow(unused)]
extern crate alloc;

pub mod io;
pub mod irq;
mod mem;
mod serial;
pub mod uart;

use alloc::{boxed::Box, sync::Arc};
use fdt::Fdt;
use jrinx_error::Result;

pub fn probe_all(fdt: &Fdt<'_>) {
    jrinx_devprober::probe_all_device(fdt).unwrap();
}

pub type InterruptHandler = Box<dyn Fn() + Send + Sync>;

pub trait Driver: Send + Sync {
    fn name(&self) -> &str;

    fn handle_irq(&self, irq_num: usize) {}
}

pub trait InterruptController: Driver {
    fn is_valid(&self, irq_num: usize) -> bool;
    fn enable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()>;
    fn disable(&mut self, cpu_id: usize, irq_num: usize) -> Result<()>;
    fn register_handler(&self, irq_num: usize, handler: InterruptHandler) -> Result<()>;
    fn register_device(&self, irq_num: usize, dev: Arc<dyn Driver>) -> Result<()>;
    fn unregister_handler(&self, irq_num: usize) -> Result<()>;
    fn contains(&self, irq_num: usize) -> bool;
}

pub trait Uart: Driver {
    fn init(&self) -> Result<()>;
    fn read(&self) -> Result<u8>;
    fn write(&self, data: u8) -> Result<()>;
    fn write_str(&self, data: &str) -> Result<()>;
}