use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_arch = "riscv32")] {
        #[path = "riscv32/mod.rs"]
        mod riscv32;
        pub use riscv32::*;
    } else if #[cfg(target_arch = "riscv64")] {
        #[path = "riscv64/mod.rs"]
        mod riscv64;
        pub use riscv64::*;
    }
}

use crate::error::HaltReason;

pub mod cpu;
pub mod cpus;
pub mod layout;
pub mod mm;

#[naked]
#[no_mangle]
#[link_section = ".boot"]
unsafe extern "C" fn _start() -> ! {
    extern "C" {
        fn _estack();
    }
    core::arch::asm!(
        "la sp, {INIT_STACK_TOP}",
        "mv tp, a0",
        "call {MAIN}",
        INIT_STACK_TOP = sym _estack,
        MAIN = sym crate::cold_init,
        options(noreturn),
    );
}

pub fn init() {
    unsafe {
        riscv::register::sstatus::set_sum();
    }
}

pub fn halt(reason: HaltReason) -> ! {
    let _ = sbi::system_reset::system_reset(
        sbi::system_reset::ResetType::WarmReboot,
        match reason {
            HaltReason::NormalExit => sbi::system_reset::ResetReason::NoReason,
            _ => sbi::system_reset::ResetReason::SystemFailure,
        },
    );
    unreachable!();
}
