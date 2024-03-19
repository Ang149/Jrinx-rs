#![feature(allocator_api)]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(panic_info_message)]
#![feature(used_with_arg)]
//#![deny(warnings)]
#![no_std]
#![no_main]

use core::{
    net::{Ipv4Addr, SocketAddr},
    time,
};

use arch::BootInfo;
use jrinx_hal::{cpu, Cpu, Hal};
use jrinx_multitask::{
    runtime::{self, Runtime},
    spawn, yield_now,
};
use spin::Mutex;

extern crate alloc;
#[macro_use]
extern crate log;

extern crate jrinx_driver as _;
#[macro_use]
extern crate jrinx_hal;
mod arch;
mod bootargs;
mod panic;
mod test;

enum BootState {
    Bootstrap,
    Ready(usize),
}

static BOOT_STATE: Mutex<BootState> = Mutex::new(BootState::Bootstrap);

fn boot_set_ready() {
    let mut boot_state = BOOT_STATE.lock();
    if let BootState::Ready(ref mut count) = *boot_state {
        *count += 1;
    } else {
        *boot_state = BootState::Ready(1);
    }
}

fn primary_init(boot_info: BootInfo) -> ! {
    jrinx_trap::init();
    jrinx_heap::init();
    jrinx_logging::init();

    let fdt = &boot_info.fdt();

    arch::cpus::init(fdt);

    jrinx_percpu::init(hal!().cpu().nproc());
    jrinx_percpu::set_local_pointer(hal!().cpu().id());

    jrinx_driver::probe_all(fdt);
    if let Some(bootargs) = fdt.chosen().bootargs() {
        bootargs::set(bootargs);
    }

    arch::secondary_boot(fdt);

    let arch = core::option_env!("ARCH").unwrap_or("unknown");
    let build_time = core::option_env!("BUILD_TIME").unwrap_or("unknown");
    let build_mode = core::option_env!("BUILD_MODE").unwrap_or("unknown");
    info!(
        "arch = {}, built at {} in {} mode",
        arch, build_time, build_mode,
    );

    jrinx_vmm::init();
    runtime::init(primary_task());
    jrinx_driver::irq::irq_dispatch::init_strategy();
    jrinx_driver::irq::irq_dispatch::min_count_strategy();
    //jrinx_driver::irq::irq_dispatch::tmp();
    boot_set_ready();

    Runtime::start();
}

fn secondary_init() -> ! {
    jrinx_trap::init();

    while let BootState::Bootstrap = *BOOT_STATE.lock() {
        core::hint::spin_loop();
    }

    jrinx_percpu::set_local_pointer(hal!().cpu().id());

    jrinx_vmm::init();
    runtime::init(secondary_task());
    boot_set_ready();

    Runtime::start();
}

async fn primary_task() {
    info!("primary task started");

    while let BootState::Ready(count) = *BOOT_STATE.lock() {
        if count == hal!().cpu().nproc_valid() {
            break;
        }
        core::hint::spin_loop();
    }

    //jrinx_net::init_network(VIRTIO_DEVICE.get().unwrap().clone());
    //jrinx_net::net_test();
    // const LOCAL_PORT: u16 = 5555;
    // let tcp_socket = TcpSocket::new();
    // tcp_socket
    //     .bind(SocketAddr::new(
    //         core::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
    //         LOCAL_PORT,
    //     ))
    //     .unwrap();
    // tcp_socket.listen().unwrap();
    // info!("listen on:http://{}/", tcp_socket.local_addr().unwrap());
    // info!("create {:?}", tcp_socket.local_addr());
    // spawn!(async { time_test() });
    // yield_now!();

    bootargs::execute().await;
    loop {}
}
pub fn time_test() {
    let start_time = hal!().cpu().get_time();
    let n = 100000000;
    let mut pi_estimate = 0.0;
    let mut sign = 1.0;
    for i in 0..n {
        pi_estimate += sign / (2.0 * i as f64 + 1.0);
        sign = -sign;
    }
    pi_estimate *= 4.0;
    let end_time = hal!().cpu().get_time();
    info!(
        "perform {} times,take {:?}, the result is {:?}",
        n,
        end_time - start_time,
        pi_estimate
    );
}
async fn secondary_task() {
    info!("secondary task started");
    // spawn!(async { time_test() });
    // yield_now!();

    loop {}
}
