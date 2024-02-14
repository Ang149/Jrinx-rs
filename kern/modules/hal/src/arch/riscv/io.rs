use crate::Io;
#[derive(Debug, Clone, Copy)]
pub struct IoImpl;

impl Io for IoImpl {
    fn read_fence(&self) {
        #[cfg(any(target_arch = "riscv32",target_arch = "riscv64"))]
        unsafe {
            core::arch::asm!("fence i,r");
        }
    }

    fn write_fence(&self) {
        #[cfg(any(target_arch = "riscv32",target_arch = "riscv64"))]
        unsafe {
            core::arch::asm!("fence w,o");
        }
    }
}


