pub fn riscv_mmio_read() {
    unsafe {
        core::arch::asm!("fence i,r");
    }
}

pub fn riscv_mmio_write() {
    unsafe {
        core::arch::asm!("fence w,o");
    }
}
