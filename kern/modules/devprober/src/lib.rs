#![no_std]

extern crate alloc;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use fdt::{node::FdtNode, Fdt};
pub use jrinx_devprober_macro::*;
use jrinx_error::Result;
use log::info;
use spin::Once;

#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DevIdent {
    DeviceType(&'static str),
    Compatible(&'static str),
}

#[repr(C)]
pub struct DevProber {
    ident: DevIdent,
    probe: fn(node: &FdtNode) -> Result<()>,
}

impl DevProber {
    pub const fn new(ident: DevIdent, probe: fn(node: &FdtNode) -> Result<()>) -> Self {
        Self { ident, probe }
    }
}
pub static ROOT_COMPATIBLE: Once<String> = Once::new();

pub fn probe_all_device(fdt: &Fdt) -> Result<()> {
    ROOT_COMPATIBLE
        .try_call_once::<_, ()>(|| Ok(fdt.root().compatible().first().to_string()))
        .unwrap();
    info!("Root compatible: {}", ROOT_COMPATIBLE.get().unwrap());
    let mut devprober_list: Vec<_> = devprober_iter().collect();
    devprober_list.sort_unstable_by(|a, b| b.ident.cmp(&a.ident));
    for devprober in &devprober_list{
        info!("{:?}", devprober.ident);
    }
    for devprober in devprober_list {
        match devprober.ident {
            DevIdent::DeviceType(device_type) => {
                for node in fdt.all_nodes().filter(|node| {
                    node.property("device_type")
                        .is_some_and(|prop| prop.as_str().is_some_and(|ty| ty == device_type))
                }) {
                    (devprober.probe)(&node)?;
                }
            }
            DevIdent::Compatible(compatible) => {
                for node in fdt.all_nodes().filter(|node| {
                    node.compatible()
                        .is_some_and(|cp| cp.all().any(|c| c == compatible))
                }) {
                    (devprober.probe)(&node)?;
                }
            }
        }
    }
    Ok(())
}

fn devprober_iter() -> impl Iterator<Item = &'static DevProber> {
    (jrinx_layout::_sdev()..jrinx_layout::_edev())
        .step_by(core::mem::size_of::<&DevProber>())
        .map(|a| unsafe { *(a as *const &DevProber) })
}
